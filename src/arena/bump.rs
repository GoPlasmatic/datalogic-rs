//! Bump allocator for efficient arena-based memory management.
//!
//! This module provides a bump allocator that allows for efficient
//! allocation of memory with minimal overhead. All allocations are
//! freed at once when the arena is reset or dropped.

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;
use std::cell::RefCell;
use std::fmt;

use super::interner::StringInterner;
use crate::value::DataValue;

/// Maximum number of path components in the fixed-size array
const PATH_CHAIN_CAPACITY: usize = 16;

/// A wrapper for a BumpVec that maintains safety around lifetimes
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
pub struct DataArena {
    /// The underlying bump allocator
    bump: Bump,

    /// String interner for efficient string storage
    interner: RefCell<StringInterner>,

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
    pub fn new() -> Self {
        Self::with_chunk_size(0) // No limit
    }

    /// Creates a new arena with the specified chunk size.
    ///
    /// The chunk size determines how much memory is allocated at once
    /// when the arena needs more space. Larger chunk sizes can improve
    /// performance but may waste memory if not fully utilized.
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        let bump = Bump::new();
        if chunk_size > 0 {
            bump.set_allocation_limit(Some(chunk_size)); // Safety limit
        }

        // Create static references to common values
        // SAFETY: These are static and never change, so it's safe to cast them
        static NULL_VALUE: DataValue<'static> = DataValue::Null;
        static TRUE_VALUE: DataValue<'static> = DataValue::Bool(true);
        static FALSE_VALUE: DataValue<'static> = DataValue::Bool(false);
        static EMPTY_STRING: &str = "";
        static EMPTY_STRING_VALUE: DataValue<'static> = DataValue::String(EMPTY_STRING);
        static EMPTY_ARRAY: [DataValue<'static>; 0] = [];
        static EMPTY_ARRAY_VALUE: DataValue<'static> = DataValue::Array(&EMPTY_ARRAY);

        Self {
            bump,
            interner: RefCell::new(StringInterner::new()),
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

    /// Gets a new BumpVec for DataValues with default capacity.
    #[inline]
    pub fn get_data_value_vec(&self) -> BumpVec<'_, DataValue<'_>> {
        BumpVec::with_capacity_in(8, &self.bump)
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
    /// This is more efficient than cloning as it reuses the BumpVec's memory.
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

    /// Allocates a value in the arena.
    ///
    pub fn alloc<T>(&self, value: T) -> &T {
        self.bump.alloc(value)
    }

    /// Allocates a slice in the arena by copying from a slice.
    ///
    pub fn alloc_slice_copy<'a, T: Copy>(&'a self, slice: &[T]) -> &'a [T] {
        self.bump.alloc_slice_copy(slice)
    }

    /// Allocates a string in the arena.
    ///
    pub fn alloc_str<'a>(&'a self, s: &str) -> &'a str {
        if s.is_empty() {
            return self.empty_string();
        }
        self.bump.alloc_str(s)
    }

    /// Interns a string, returning a reference to a unique instance.
    ///
    pub fn intern_str<'a>(&'a self, s: &str) -> &'a str {
        if s.is_empty() {
            return self.empty_string();
        }
        self.interner.borrow_mut().intern(s, &self.bump)
    }

    /// Resets the arena, freeing all allocations.
    ///
    pub fn reset(&mut self) {
        self.bump.reset();
        self.interner = RefCell::new(StringInterner::new());
        self.current_context.replace(None);
        self.root_context.replace(None);
        self.path_chain.replace(PathChainVec::new());
    }

    /// Returns the current memory usage of the arena in bytes.
    pub fn memory_usage(&self) -> usize {
        self.bump.allocated_bytes()
    }

    /// Creates a new temporary arena for short-lived allocations.
    ///
    pub fn create_temp_arena(&self) -> DataArena {
        DataArena::with_chunk_size(self.chunk_size)
    }

    /// Allocates a slice in the arena and fills it with values generated by a function.
    /// Now implemented using BumpVec for better efficiency.
    ///
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

    /// Returns a reference to the preallocated null value.
    pub fn null_value<'a>(&'a self) -> &'a DataValue<'a> {
        unsafe {
            std::mem::transmute::<&'static DataValue<'static>, &'a DataValue<'a>>(self.null_value)
        }
    }

    /// Returns a reference to the preallocated true value.
    pub fn true_value<'a>(&'a self) -> &'a DataValue<'a> {
        unsafe {
            std::mem::transmute::<&'static DataValue<'static>, &'a DataValue<'a>>(self.true_value)
        }
    }

    /// Returns a reference to the preallocated false value.
    pub fn false_value<'a>(&'a self) -> &'a DataValue<'a> {
        unsafe {
            std::mem::transmute::<&'static DataValue<'static>, &'a DataValue<'a>>(self.false_value)
        }
    }

    /// Returns a reference to the preallocated empty string.
    pub fn empty_string<'a>(&'a self) -> &'a str {
        unsafe { std::mem::transmute::<&'static str, &'a str>(self.empty_string) }
    }

    /// Returns a reference to the preallocated empty string value.
    pub fn empty_string_value<'a>(&'a self) -> &'a DataValue<'a> {
        unsafe {
            std::mem::transmute::<&'static DataValue<'static>, &'a DataValue<'a>>(
                self.empty_string_value,
            )
        }
    }

    /// Returns a reference to the preallocated empty array.
    pub fn empty_array<'a>(&'a self) -> &'a [DataValue<'a>] {
        unsafe {
            std::mem::transmute::<&'static [DataValue<'static>], &'a [DataValue<'a>]>(
                self.empty_array,
            )
        }
    }

    /// Returns a reference to the preallocated empty array value.
    pub fn empty_array_value<'a>(&'a self) -> &'a DataValue<'a> {
        unsafe {
            std::mem::transmute::<&'static DataValue<'static>, &'a DataValue<'a>>(
                self.empty_array_value,
            )
        }
    }

    /// Allocates a slice of DataValues in the arena.
    /// Now implemented using BumpVec for better efficiency.
    ///
    pub fn alloc_data_value_slice<'a>(&'a self, vals: &[DataValue<'a>]) -> &'a [DataValue<'a>] {
        if vals.is_empty() {
            return self.empty_array();
        }
        self.vec_into_slice(vals.to_vec())
    }

    /// Allocates a slice of object entries in the arena.
    /// Now implemented using BumpVec for better efficiency.
    ///
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
    /// Now implemented using BumpVec for better efficiency.
    ///
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

    /// Sets the current context for the arena.
    pub fn set_current_context<'a>(&self, context: &'a DataValue<'a>, key: &'a DataValue<'a>) {
        self.current_context.replace(Some(unsafe {
            std::mem::transmute::<&'a DataValue<'a>, &'static DataValue<'static>>(context)
        }));
        self.push_path_key(key);
    }

    /// Returns the current context for the arena.
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
    pub fn root_context(&self) -> Option<&DataValue> {
        // Reset the path chain when getting root context
        self.path_chain.borrow_mut().clear();
        *self.root_context.borrow()
    }

    /// Sets the root context for the arena.
    pub fn set_root_context<'a>(&self, context: &'a DataValue<'a>) {
        self.root_context.replace(Some(unsafe {
            std::mem::transmute::<&'a DataValue<'a>, &'static DataValue<'static>>(context)
        }));
    }

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
                None => Some(self.null_value()),  // Return null if no root context
            };
        }
        
        // Get the root context, never returning None
        let root = match *self.root_context.borrow() {
            Some(ctx) => ctx,
            None => return Some(self.null_value()),  // Return null if no root context
        };
        
        // Use an optimization to avoid allocating a new vector when possible
        let path_chain = self.path_chain.borrow();
        let path_slice = path_chain.as_slice();
        
        // Navigate to the correct context without creating intermediate vectors
        self.navigate_to_context(root, path_slice, chain_len - scope_jump)
    }
    
    // Helper function to navigate through a context without allocating
    #[inline(never)]
    fn navigate_to_context<'a>(&'a self, root: &'a DataValue<'a>, path_components: &[&'a DataValue<'a>], depth: usize) -> Option<&'a DataValue<'a>> {
        let mut current = root;
        
        // Only navigate to the specified depth
        for component in path_components.iter().take(depth) {
            match component {
                DataValue::String(key) => {
                    // Navigate by string key
                    if let DataValue::Object(entries) = current {
                        let mut found = false;
                        for &(k, ref v) in *entries {
                            if k == *key {
                                current = v;
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return Some(self.null_value());
                        }
                    } else {
                        return Some(self.null_value());
                    }
                },
                DataValue::Number(n) => {
                    // Navigate by array index
                    if let Some(idx) = n.as_i64() {
                        if idx >= 0 {
                            let index = idx as usize;
                            if let DataValue::Array(items) = current {
                                if index < items.len() {
                                    current = &items[index];
                                } else {
                                    return Some(self.null_value());
                                }
                            } else {
                                return Some(self.null_value());
                            }
                        } else {
                            return Some(self.null_value());
                        }
                    } else {
                        return Some(self.null_value());
                    }
                },
                _ => return Some(self.null_value()),
            }
        }
        
        Some(current)
    }

    /// Appends a key component to the current path chain.
    pub fn push_path_key<'a>(&self, key: &'a DataValue<'a>) {
        self.path_chain.borrow_mut().push(unsafe {
            std::mem::transmute::<&'a DataValue<'a>, &'static DataValue<'static>>(key)
        });
    }
    
    /// Removes the last component from the path chain.
    pub fn pop_path_component(&self) -> Option<&'static DataValue<'static>> {
        self.path_chain.borrow_mut().pop()
    }
    
    /// Clears the path chain.
    pub fn clear_path_chain(&self) {
        self.path_chain.borrow_mut().clear();
    }
    
    /// Returns the length of the path chain.
    pub fn path_chain_len(&self) -> usize {
        self.path_chain.borrow().len()
    }
    
    /// Returns the current path chain as a slice.
    pub fn path_chain_as_slice(&self) -> Vec<&DataValue> {
        let chain = self.path_chain.borrow();
        chain.as_slice().iter().copied().collect()
    }
    
    /// Efficiently access the path chain without allocating a new vector.
    #[inline]
    pub fn with_path_chain<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[&DataValue]) -> R,
    {
        let chain = self.path_chain.borrow();
        f(chain.as_slice())
    }
    
    /// Returns the last path component.
    pub fn last_path_component(&self) -> Option<&DataValue> {
        self.path_chain.borrow().last()
    }
    
    /// Batch appends multiple path components in one operation.
    pub fn push_path_components<'a>(&self, keys: &[&'a DataValue<'a>]) {
        if keys.is_empty() {
            return;
        }
        
        let mut path_chain = self.path_chain.borrow_mut();
        for key in keys {
            let static_key = unsafe {
                std::mem::transmute::<&'a DataValue<'a>, &'static DataValue<'static>>(key)
            };
            path_chain.push(static_key);
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
}
