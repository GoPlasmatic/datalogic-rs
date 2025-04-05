//! Bump allocator for efficient arena-based memory management.
//!
//! This module provides a bump allocator that allows for efficient
//! allocation of memory with minimal overhead. All allocations are
//! freed at once when the arena is reset or dropped.
//!
//! The `DataArena` maintains shared references and context for evaluating
//! logic expressions.

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::mem;

use super::interner::StringInterner;
use crate::logic::Result;
use crate::value::{DataValue, FromJson, NumberValue, ToJson};

/// Trait for custom JSONLogic operators
pub trait CustomOperator: fmt::Debug + Send + Sync {
    /// Evaluate the custom operator with the given arguments
    ///
    /// This function takes owned DataValue arguments and returns an owned DataValue.
    /// The actual allocation in the arena is handled internally.
    fn evaluate(&self, args: &[DataValue]) -> Result<DataValue>;
}

/// Registry for custom operator functions
#[derive(Default)]
pub struct CustomOperatorRegistry {
    operators: HashMap<String, Box<dyn CustomOperator>>,
}

impl CustomOperatorRegistry {
    /// Creates a new empty custom operator registry
    pub fn new() -> Self {
        Self {
            operators: HashMap::new(),
        }
    }

    /// Registers a custom operator function
    pub fn register(&mut self, name: &str, operator: Box<dyn CustomOperator>) {
        self.operators.insert(name.to_string(), operator);
    }

    /// Returns a reference to a custom operator by name
    pub fn get(&self, name: &str) -> Option<&dyn CustomOperator> {
        self.operators.get(name).map(|op| op.as_ref())
    }
}

/// Maximum number of path components in the fixed-size array
const PATH_CHAIN_CAPACITY: usize = 16;

/// Default allocation size for vectors
const DEFAULT_VECTOR_CAPACITY: usize = 8;

/// A wrapper for path chain that maintains safety around lifetimes
///
/// This struct helps track the path from root to current position
/// in a data structure, while handling lifetimes safely.
struct PathChainVec {
    /// The inner vector, using 'static lifetimes to avoid borrow checker issues
    vec: Vec<&'static DataValue<'static>>,
    /// Capacity reserved for the vector to avoid reallocations
    capacity: usize,
}

impl PathChainVec {
    /// Create a new path chain with default capacity
    fn new() -> Self {
        Self {
            vec: Vec::with_capacity(PATH_CHAIN_CAPACITY),
            capacity: PATH_CHAIN_CAPACITY,
        }
    }

    /// Push a new element to the path chain
    fn push(&mut self, value: &'static DataValue<'static>) {
        self.vec.push(value);
    }

    /// Pop the last element from the path chain
    fn pop(&mut self) -> Option<&'static DataValue<'static>> {
        self.vec.pop()
    }

    /// Clear the path chain
    fn clear(&mut self) {
        self.vec.clear();
    }

    /// Get the length of the path chain
    fn len(&self) -> usize {
        self.vec.len()
    }

    /// Check if the path chain is empty
    fn _is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    /// Get an element at the specified index
    fn _get(&self, index: usize) -> Option<&'static DataValue<'static>> {
        self.vec.get(index).copied()
    }

    /// Get the last element in the path chain
    fn last(&self) -> Option<&'static DataValue<'static>> {
        self.vec.last().copied()
    }

    /// Get a slice of the path chain
    fn as_slice(&self) -> &[&'static DataValue<'static>] {
        &self.vec
    }
}

impl fmt::Debug for PathChainVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PathChainVec")
            .field("len", &self.len())
            .field("capacity", &self.capacity)
            .field("elements", &self.vec)
            .finish()
    }
}

/// An arena allocator for efficient data allocation.
///
/// The DataArena provides memory management for DataLogic values, with
/// optimized allocation strategies and context tracking for logic evaluation.
pub struct DataArena {
    /// The underlying bump allocator
    bump: Bump,

    /// String interner for efficient string storage
    interner: RefCell<StringInterner>,

    /// Custom operator registry for evaluating custom operators
    custom_operators: RefCell<CustomOperatorRegistry>,

    /// Chunk size for allocations (in bytes)
    chunk_size: usize,

    /// Preallocated null value
    null_value: &'static DataValue<'static>,

    /// Preallocated true value
    true_value: &'static DataValue<'static>,

    /// Preallocated false value
    false_value: &'static DataValue<'static>,

    /// Preallocated empty string
    empty_string: &'static str,

    /// Preallocated empty string value
    empty_string_value: &'static DataValue<'static>,

    /// Preallocated empty array
    empty_array: &'static [DataValue<'static>],

    /// Preallocated empty array value
    empty_array_value: &'static DataValue<'static>,

    /// Current context (root data)
    current_context: RefCell<Option<&'static DataValue<'static>>>,

    /// Preallocated root context
    root_context: RefCell<Option<&'static DataValue<'static>>>,

    /// Current path chain - represents the path from root to current position
    path_chain: RefCell<PathChainVec>,
}

impl Default for DataArena {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for DataArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataArena")
            .field("chunk_size", &self.chunk_size)
            .field("path_chain", &self.path_chain)
            .finish()
    }
}

impl DataArena {
    /// Creates a new empty arena.
    ///
    /// The arena starts with default capacity and will grow as needed.
    pub fn new() -> Self {
        Self::with_chunk_size(0) // No limit
    }

    /// Creates a new arena with the specified chunk size.
    ///
    /// The chunk size determines how much memory is allocated at once
    /// when the arena needs more space. Larger chunk sizes can improve
    /// performance but may waste memory if not fully utilized.
    ///
    /// # Arguments
    ///
    /// * `chunk_size` - The size in bytes of each chunk allocation
    ///   (0 means no limit)
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        let bump = Bump::new();
        if chunk_size > 0 {
            bump.set_allocation_limit(Some(chunk_size)); // Safety limit
        }

        // Create static references to common values
        // These are static and never change, so they're safe to use
        static NULL_VALUE: DataValue<'static> = DataValue::Null;
        static TRUE_VALUE: DataValue<'static> = DataValue::Bool(true);
        static FALSE_VALUE: DataValue<'static> = DataValue::Bool(false);
        static EMPTY_STRING: &str = "";
        static EMPTY_STRING_VALUE: DataValue<'static> = DataValue::String(EMPTY_STRING);
        static EMPTY_ARRAY: [DataValue<'static>; 0] = [];
        static EMPTY_ARRAY_VALUE: DataValue<'static> = DataValue::Array(&EMPTY_ARRAY);

        Self {
            bump,
            interner: RefCell::new(StringInterner::with_capacity(64)), // Start with reasonable capacity
            custom_operators: RefCell::new(CustomOperatorRegistry::new()),
            chunk_size,
            null_value: &NULL_VALUE,
            true_value: &TRUE_VALUE,
            false_value: &FALSE_VALUE,
            empty_string: EMPTY_STRING,
            empty_string_value: &EMPTY_STRING_VALUE,
            empty_array: &EMPTY_ARRAY,
            empty_array_value: &EMPTY_ARRAY_VALUE,
            current_context: RefCell::new(None),
            root_context: RefCell::new(None),
            path_chain: RefCell::new(PathChainVec::new()),
        }
    }

    //
    // Vector allocation helpers
    //

    /// Gets a new BumpVec for DataValues with default capacity.
    #[inline]
    pub fn get_data_value_vec(&self) -> BumpVec<'_, DataValue<'_>> {
        BumpVec::with_capacity_in(DEFAULT_VECTOR_CAPACITY, &self.bump)
    }

    /// Gets a new BumpVec with specified capacity.
    #[inline]
    pub fn get_data_value_vec_with_capacity(&self, capacity: usize) -> BumpVec<'_, DataValue<'_>> {
        BumpVec::with_capacity_in(capacity, &self.bump)
    }

    /// Gets a new BumpVec for object entries with specified capacity.
    #[inline]
    pub fn get_object_entries_vec(&self, capacity: usize) -> BumpVec<'_, (&str, DataValue<'_>)> {
        BumpVec::with_capacity_in(capacity, &self.bump)
    }

    /// Converts a BumpVec into a slice allocated in the arena.
    ///
    /// This efficiently transfers ownership of the vector's memory to the arena.
    ///
    /// # Safety
    ///
    /// This function takes ownership of the vector's memory and transfers it to the arena.
    /// The vector is forgotten to prevent double-free, as the arena will reclaim the memory
    /// when it is reset or dropped.
    #[inline]
    pub fn bump_vec_into_slice<'a, T>(&'a self, vec: BumpVec<'a, T>) -> &'a [T] {
        if vec.is_empty() {
            return &[];
        }

        let ptr = vec.as_ptr();
        let len = vec.len();

        // Forget the vector to prevent double-free (memory is owned by the arena)
        std::mem::forget(vec);

        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    /// Converts a Vec into a slice allocated in the arena.
    ///
    /// # Safety
    ///
    /// This function takes ownership of the vector's memory and transfers it to the arena.
    /// The vector is forgotten to prevent double-free, as the arena will reclaim the memory
    /// when it is reset or dropped.
    #[inline]
    pub fn vec_into_slice<T>(&self, vec: Vec<T>) -> &[T] {
        if vec.is_empty() {
            return &[];
        }

        let ptr = vec.as_ptr();
        let len = vec.len();

        // Forget the vector to prevent double-free (memory will be reclaimed when arena is dropped)
        std::mem::forget(vec);

        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    //
    // Basic allocation methods
    //

    /// Allocates a value in the arena.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to allocate
    ///
    /// # Returns
    ///
    /// A reference to the allocated value, valid for the lifetime of the arena
    #[inline]
    pub fn alloc<T>(&self, value: T) -> &T {
        self.bump.alloc(value)
    }

    /// Allocates a slice in the arena by copying from a slice.
    ///
    /// # Arguments
    ///
    /// * `slice` - The slice to copy
    ///
    /// # Returns
    ///
    /// A reference to the allocated slice, valid for the lifetime of the arena
    #[inline]
    pub fn alloc_slice_copy<'a, T: Copy>(&'a self, slice: &[T]) -> &'a [T] {
        self.bump.alloc_slice_copy(slice)
    }

    /// Allocates a string in the arena.
    ///
    /// # Arguments
    ///
    /// * `s` - The string to allocate
    ///
    /// # Returns
    ///
    /// A reference to the allocated string, valid for the lifetime of the arena
    #[inline]
    pub fn alloc_str<'a>(&'a self, s: &str) -> &'a str {
        if s.is_empty() {
            return self.empty_string();
        }
        self.bump.alloc_str(s)
    }

    /// Interns a string, returning a reference to a unique instance.
    ///
    /// This uses the string interner to deduplicate strings, reducing memory usage.
    ///
    /// # Arguments
    ///
    /// * `s` - The string to intern
    ///
    /// # Returns
    ///
    /// A reference to the interned string, valid for the lifetime of the arena
    #[inline]
    pub fn intern_str<'a>(&'a self, s: &str) -> &'a str {
        if s.is_empty() {
            return self.empty_string();
        }
        self.interner.borrow_mut().intern(s, &self.bump)
    }

    /// Resets the arena, freeing all allocations.
    ///
    /// This clears all allocated memory, contexts, and path chains.
    pub fn reset(&mut self) {
        self.bump.reset();
        self.interner = RefCell::new(StringInterner::with_capacity(64));
        self.clear_contexts_and_paths();
    }

    /// Clears all contexts and path information.
    #[inline]
    fn clear_contexts_and_paths(&mut self) {
        self.current_context.replace(None);
        self.root_context.replace(None);
        self.path_chain.replace(PathChainVec::new());
    }

    /// Returns the current memory usage of the arena in bytes.
    #[inline]
    pub fn memory_usage(&self) -> usize {
        self.bump.allocated_bytes()
    }

    /// Creates a new temporary arena for short-lived allocations.
    ///
    /// This is useful for operations that need temporary allocations
    /// that should be discarded after use.
    #[inline]
    pub fn create_temp_arena(&self) -> DataArena {
        DataArena::with_chunk_size(self.chunk_size)
    }

    /// Allocates a slice in the arena and fills it with values generated by a function.
    ///
    /// # Arguments
    ///
    /// * `len` - The length of the slice to allocate
    /// * `f` - A function that produces a value for each index
    ///
    /// # Returns
    ///
    /// A reference to the allocated slice, valid for the lifetime of the arena
    pub fn alloc_slice_fill_with<T, F>(&self, len: usize, mut f: F) -> &[T]
    where
        F: FnMut(usize) -> T,
    {
        if len == 0 {
            return &[];
        }

        let mut vec = BumpVec::with_capacity_in(len, &self.bump);
        for i in 0..len {
            vec.push(f(i));
        }
        self.bump_vec_into_slice(vec)
    }

    //
    // Preallocated value accessors
    //

    /// Returns a reference to the preallocated null value.
    #[inline]
    pub fn null_value(&self) -> &DataValue<'_> {
        // SAFETY: The static lifetime can be safely narrowed to match the arena's lifetime
        self.transmute_lifetime(self.null_value)
    }

    /// Returns a reference to the preallocated true value.
    #[inline]
    pub fn true_value(&self) -> &DataValue<'_> {
        // SAFETY: The static lifetime can be safely narrowed to match the arena's lifetime
        self.transmute_lifetime(self.true_value)
    }

    /// Returns a reference to the preallocated false value.
    #[inline]
    pub fn false_value(&self) -> &DataValue<'_> {
        // SAFETY: The static lifetime can be safely narrowed to match the arena's lifetime
        self.transmute_lifetime(self.false_value)
    }

    /// Returns a reference to the preallocated empty string.
    #[inline]
    pub fn empty_string<'a>(&'a self) -> &'a str {
        // SAFETY: The static lifetime can be safely narrowed to match the arena's lifetime
        unsafe { mem::transmute::<&'static str, &'a str>(self.empty_string) }
    }

    /// Returns a reference to the preallocated empty string value.
    #[inline]
    pub fn empty_string_value(&self) -> &DataValue<'_> {
        // SAFETY: The static lifetime can be safely narrowed to match the arena's lifetime
        self.transmute_lifetime(self.empty_string_value)
    }

    /// Returns a reference to the preallocated empty array.
    #[inline]
    pub fn empty_array<'a>(&'a self) -> &'a [DataValue<'a>] {
        // SAFETY: The static lifetime can be safely narrowed to match the arena's lifetime
        unsafe {
            mem::transmute::<&'static [DataValue<'static>], &'a [DataValue<'a>]>(self.empty_array)
        }
    }

    /// Returns a reference to the preallocated empty array value.
    #[inline]
    pub fn empty_array_value(&self) -> &DataValue<'_> {
        // SAFETY: The static lifetime can be safely narrowed to match the arena's lifetime
        self.transmute_lifetime(self.empty_array_value)
    }

    /// Safely narrows 'static lifetime to arena lifetime.
    ///
    /// This helper centralizes the transmute pattern used throughout the code.
    ///
    /// # Safety
    ///
    /// This assumes that the static reference will live at least as long
    /// as the arena, which is guaranteed for preallocated static values.
    #[inline]
    fn transmute_lifetime<'a, T>(&'a self, value: &'static T) -> &'a T {
        // SAFETY: We're narrowing a 'static lifetime to 'a, which is always safe
        unsafe { mem::transmute::<&'static T, &'a T>(value) }
    }

    /// Allocates a slice of DataValues in the arena.
    ///
    /// # Arguments
    ///
    /// * `vals` - The values to allocate
    ///
    /// # Returns
    ///
    /// A reference to the allocated slice, valid for the lifetime of the arena
    #[inline]
    pub fn alloc_data_value_slice<'a>(&'a self, vals: &[DataValue<'a>]) -> &'a [DataValue<'a>] {
        if vals.is_empty() {
            return self.empty_array();
        }
        self.vec_into_slice(vals.to_vec())
    }

    /// Allocates a slice of object entries in the arena.
    ///
    /// # Arguments
    ///
    /// * `entries` - The object entries to allocate
    ///
    /// # Returns
    ///
    /// A reference to the allocated slice, valid for the lifetime of the arena
    #[inline]
    pub fn alloc_object_entries<'a>(
        &'a self,
        entries: &[(&'a str, DataValue<'a>)],
    ) -> &'a [(&'a str, DataValue<'a>)] {
        if entries.is_empty() {
            return &[];
        }

        self.vec_into_slice(entries.to_vec())
    }

    /// Allocates a small array of DataValues (up to 8 elements) in the arena.
    ///
    /// This method is optimized for small arrays.
    ///
    /// # Arguments
    ///
    /// * `values` - The values to allocate (must be at most 8 elements)
    ///
    /// # Returns
    ///
    /// A reference to the allocated slice, valid for the lifetime of the arena
    #[inline]
    pub fn alloc_small_data_value_array<'a>(
        &'a self,
        values: &[DataValue<'a>],
    ) -> &'a [DataValue<'a>] {
        debug_assert!(values.len() <= 8, "This method is only for small arrays");

        if values.is_empty() {
            return self.empty_array();
        }

        // For very small arrays, use specialized methods
        match values.len() {
            1 => {
                let ptr = self.bump.alloc(values[0].clone());
                std::slice::from_ref(ptr)
            }
            2..=8 => self.vec_into_slice(values.to_vec()),
            _ => unreachable!("This method is only for arrays up to 8 elements"),
        }
    }

    //
    // Context management methods
    //

    /// Sets the current context for the arena.
    ///
    /// This establishes a new current context and records the path component.
    ///
    /// # Arguments
    ///
    /// * `context` - The context data value
    /// * `key` - The key for this context in the path chain
    #[inline]
    pub fn set_current_context<'a>(&self, context: &'a DataValue<'a>, key: &'a DataValue<'a>) {
        // SAFETY: Widening the lifetime is safe because the arena manages the memory
        let static_context =
            unsafe { mem::transmute::<&'a DataValue<'a>, &'static DataValue<'static>>(context) };

        self.current_context.replace(Some(static_context));
        self.push_path_key(key);
    }

    /// Returns the current context for the arena.
    ///
    /// # Arguments
    ///
    /// * `scope_jump` - How many levels to jump up the scope chain (0 means current context)
    ///
    /// # Returns
    ///
    /// The context data value, or None if no context is set
    #[inline]
    pub fn current_context(&self, scope_jump: usize) -> Option<&DataValue> {
        // Fast path for the common case (no scope jump)
        if scope_jump == 0 {
            return *self.current_context.borrow();
        } else {
            // Cold path for scope jumps
            self.root_context_with_jump(scope_jump)
        }
    }

    /// Returns the root context for the arena.
    ///
    /// This also resets the path chain.
    ///
    /// # Returns
    ///
    /// The root context data value, or None if no root context is set
    #[inline]
    pub fn root_context(&self) -> Option<&DataValue> {
        // Reset the path chain when getting root context
        self.path_chain.borrow_mut().clear();
        *self.root_context.borrow()
    }

    /// Sets the root context for the arena.
    ///
    /// # Arguments
    ///
    /// * `context` - The root context data value
    #[inline]
    pub fn set_root_context<'a>(&self, context: &'a DataValue<'a>) {
        // SAFETY: Widening the lifetime is safe because the arena manages the memory
        let static_context =
            unsafe { mem::transmute::<&'a DataValue<'a>, &'static DataValue<'static>>(context) };

        self.root_context.replace(Some(static_context));
    }

    /// Get a context after jumping up the scope chain.
    ///
    /// This is a cold path for handling scope jumps.
    ///
    /// # Arguments
    ///
    /// * `scope_jump` - How many levels to jump up the scope chain
    ///
    /// # Returns
    ///
    /// The context data value after jumping up the scope chain
    #[cold]
    #[inline(never)]
    fn root_context_with_jump(&self, scope_jump: usize) -> Option<&DataValue> {
        if scope_jump == 0 {
            return *self.current_context.borrow();
        }

        // Get the current path chain
        let chain_len = self.path_chain_len();

        if scope_jump >= chain_len {
            // If trying to jump beyond the root, return the root context
            // We must always return a valid context, never None
            return match *self.root_context.borrow() {
                Some(ctx) => Some(ctx),
                None => Some(self.null_value()), // Return null if no root context
            };
        }

        // Get the root context, never returning None
        let root = match *self.root_context.borrow() {
            Some(ctx) => ctx,
            None => return Some(self.null_value()), // Return null if no root context
        };

        // Use an optimization to avoid allocating a new vector when possible
        let path_chain = self.path_chain.borrow();
        let path_slice = path_chain.as_slice();

        // Navigate to the correct context without creating intermediate vectors
        self.navigate_to_context(root, path_slice, chain_len - scope_jump)
    }

    /// Helper function to navigate through a context without allocating
    ///
    /// # Arguments
    ///
    /// * `root` - The root context to start from
    /// * `path_components` - The path components to navigate through
    /// * `depth` - How many components to include in the navigation
    ///
    /// # Returns
    ///
    /// The context data value after navigating to the specified depth
    #[inline(never)]
    fn navigate_to_context<'a>(
        &'a self,
        root: &'a DataValue<'a>,
        path_components: &[&'a DataValue<'a>],
        depth: usize,
    ) -> Option<&'a DataValue<'a>> {
        let mut current = root;

        // Only navigate to the specified depth
        for component in path_components.iter().take(depth) {
            match component {
                DataValue::String(key) => {
                    if !self.navigate_by_string_key(&mut current, key) {
                        return Some(self.null_value());
                    }
                }
                DataValue::Number(n) => {
                    if !self.navigate_by_array_index(&mut current, n) {
                        return Some(self.null_value());
                    }
                }
                _ => return Some(self.null_value()),
            }
        }

        Some(current)
    }

    /// Navigate an object by string key
    ///
    /// # Returns
    ///
    /// `true` if navigation succeeded, `false` if key not found or not an object
    #[inline]
    fn navigate_by_string_key<'a>(&'a self, current: &mut &'a DataValue<'a>, key: &str) -> bool {
        if let DataValue::Object(entries) = *current {
            for &(k, ref v) in *entries {
                if k == key {
                    *current = v;
                    return true;
                }
            }
            false // Key not found
        } else {
            false // Not an object
        }
    }

    /// Navigate an array by index
    ///
    /// # Returns
    ///
    /// `true` if navigation succeeded, `false` if invalid index or not an array
    #[inline]
    fn navigate_by_array_index<'a>(
        &'a self,
        current: &mut &'a DataValue<'a>,
        n: &NumberValue,
    ) -> bool {
        if let Some(idx) = n.as_i64() {
            if idx >= 0 {
                let index = idx as usize;
                if let DataValue::Array(items) = *current {
                    if index < items.len() {
                        *current = &items[index];
                        return true;
                    }
                }
            }
        }
        false
    }

    //
    // Path chain management methods
    //

    /// Appends a key component to the current path chain.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to append to the path chain
    #[inline]
    pub fn push_path_key<'a>(&self, key: &'a DataValue<'a>) {
        // SAFETY: Widening the lifetime is safe because the arena manages the memory
        let static_key =
            unsafe { mem::transmute::<&'a DataValue<'a>, &'static DataValue<'static>>(key) };

        self.path_chain.borrow_mut().push(static_key);
    }

    /// Removes the last component from the path chain.
    ///
    /// # Returns
    ///
    /// The removed path component, or None if the path chain is empty
    #[inline]
    pub fn pop_path_component(&self) -> Option<&DataValue> {
        // SAFETY: The static lifetime can be safely narrowed
        self.path_chain
            .borrow_mut()
            .pop()
            .map(|v| self.transmute_lifetime(v))
    }

    /// Clears the path chain.
    #[inline]
    pub fn clear_path_chain(&self) {
        self.path_chain.borrow_mut().clear();
    }

    /// Returns the length of the path chain.
    #[inline]
    pub fn path_chain_len(&self) -> usize {
        self.path_chain.borrow().len()
    }

    /// Returns the current path chain as a slice.
    ///
    /// This allocates a new vector.
    #[inline]
    pub fn path_chain_as_slice(&self) -> Vec<&DataValue> {
        let chain = self.path_chain.borrow();
        chain
            .as_slice()
            .iter()
            .map(|&v| self.transmute_lifetime(v))
            .collect()
    }

    /// Efficiently access the path chain without allocating a new vector.
    ///
    /// # Arguments
    ///
    /// * `f` - A function that takes a slice of the path chain and returns a result
    ///
    /// # Returns
    ///
    /// The result of calling the function with the path chain slice
    #[inline]
    pub fn with_path_chain<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[&DataValue]) -> R,
    {
        let chain = self.path_chain.borrow();
        // SAFETY: The static lifetime can be safely narrowed
        let path_slice: &[&DataValue] = unsafe {
            mem::transmute::<&[&'static DataValue<'static>], &[&DataValue]>(chain.as_slice())
        };
        f(path_slice)
    }

    /// Returns the last path component.
    ///
    /// # Returns
    ///
    /// The last path component, or None if the path chain is empty
    #[inline]
    pub fn last_path_component(&self) -> Option<&DataValue> {
        // SAFETY: The static lifetime can be safely narrowed
        self.path_chain
            .borrow()
            .last()
            .map(|v| self.transmute_lifetime(v))
    }

    /// Batch appends multiple path components in one operation.
    ///
    /// # Arguments
    ///
    /// * `keys` - The keys to append to the path chain
    #[inline]
    pub fn push_path_components<'a, 'b>(&'a self, keys: &'b [&'b DataValue<'b>])
    where
        'b: 'a,
    {
        if keys.is_empty() {
            return;
        }

        let mut path_chain = self.path_chain.borrow_mut();
        for &key in keys {
            // SAFETY: Widening the lifetime is safe because the arena manages the memory
            let static_key =
                unsafe { mem::transmute::<&'b DataValue<'b>, &'static DataValue<'static>>(key) };
            path_chain.push(static_key);
        }
    }

    /// Register a custom operator
    pub fn register_custom_operator(&self, name: &str, operator: Box<dyn CustomOperator>) {
        self.custom_operators.borrow_mut().register(name, operator);
    }

    /// Check if a custom operator exists
    pub fn has_custom_operator(&self, name: &str) -> bool {
        self.custom_operators.borrow().get(name).is_some()
    }

    /// Evaluate a custom operator with the given name and arguments
    pub fn evaluate_custom_operator<'a>(
        &'a self,
        name: &str,
        args: &[&'a DataValue<'a>],
    ) -> Result<&'a DataValue<'a>> {
        // Get the custom operator
        if let Some(op) = self.custom_operators.borrow().get(name) {
            // Convert arena references to owned DataValues by going through JSON
            let owned_args: Vec<DataValue> = args
                .iter()
                .map(|&arg| {
                    // Convert to JSON and back to create owned values
                    let json = arg.to_json();
                    DataValue::from_json(&json, self)
                })
                .collect();

            // Call the custom operator with owned values
            let result = op.evaluate(&owned_args)?;

            // Allocate the result back into the arena
            let json_result = result.to_json();
            let result_value = DataValue::from_json(&json_result, self);
            Ok(self.alloc(result_value))
        } else {
            Err(crate::logic::LogicError::OperatorNotFoundError {
                operator: name.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc() {
        let arena = DataArena::new();
        let value = arena.alloc(42);
        assert_eq!(*value, 42);
    }

    #[test]
    fn test_alloc_slice_copy() {
        let arena = DataArena::new();
        let original = &[1, 2, 3, 4, 5];
        let slice = arena.alloc_slice_copy(original);
        assert_eq!(slice, original);
    }

    #[test]
    fn test_alloc_str() {
        let arena = DataArena::new();
        let s = arena.alloc_str("hello");
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_reset() {
        let mut arena = DataArena::new();

        // Allocate a significant amount of data
        for i in 0..1000 {
            let _ = arena.alloc_str(&format!("test string {}", i));
        }

        arena.reset();

        // After reset, the memory is still allocated to the arena but marked as free
        // So we need to check that we can reuse it without increasing usage significantly

        // Allocate some data again
        let s = arena.alloc_str("hello");
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_temp_arena() {
        let arena = DataArena::new();
        let value;
        {
            let temp_arena = arena.create_temp_arena();
            let temp = temp_arena.alloc(42);
            assert_eq!(*temp, 42);
            value = *temp;
        }
        // We can still use the value, but the memory is freed
        assert_eq!(value, 42);
    }

    #[test]
    fn test_bump_vec() {
        let arena = DataArena::new();
        let mut vec = arena.get_data_value_vec();
        vec.push(DataValue::integer(1));
        vec.push(DataValue::integer(2));
        let slice = arena.bump_vec_into_slice(vec);
        assert_eq!(slice.len(), 2);
    }

    #[test]
    fn test_path_chain() {
        let arena = DataArena::new();

        // Test pushing and popping
        arena.push_path_key(&DataValue::string(&arena, "key1"));
        arena.push_path_key(&DataValue::string(&arena, "key2"));

        assert_eq!(arena.path_chain_len(), 2);

        let last = arena.last_path_component().unwrap();
        assert_eq!(last.as_str(), Some("key2"));

        let popped = arena.pop_path_component().unwrap();
        assert_eq!(popped.as_str(), Some("key2"));

        assert_eq!(arena.path_chain_len(), 1);

        // Test clearing
        arena.clear_path_chain();
        assert_eq!(arena.path_chain_len(), 0);
    }

    #[test]
    fn test_context_management() {
        let arena = DataArena::new();

        // Test setting and getting contexts
        let root = arena.alloc(DataValue::object(
            &arena,
            &[(arena.intern_str("root"), DataValue::integer(1))],
        ));

        arena.set_root_context(root);
        let retrieved_root = arena.root_context().unwrap();

        assert!(matches!(retrieved_root, DataValue::Object(_)));

        // Test current context
        let current = arena.alloc(DataValue::object(
            &arena,
            &[(arena.intern_str("current"), DataValue::integer(2))],
        ));

        let key = arena.alloc(DataValue::string(&arena, "key"));
        arena.set_current_context(current, key);

        let retrieved_current = arena.current_context(0).unwrap();
        assert!(matches!(retrieved_current, DataValue::Object(_)));
    }
}
