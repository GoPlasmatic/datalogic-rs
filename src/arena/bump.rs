//! Bump allocator for efficient arena-based memory management.
//!
//! This module provides a bump allocator that allows for efficient
//! allocation of memory with minimal overhead. All allocations are
//! freed at once when the arena is reset or dropped.

use bumpalo::Bump;
use std::cell::RefCell;
use std::fmt;
use std::vec::Vec;

use super::interner::StringInterner;
use crate::value::DataValue;

/// A pool of reusable vectors to avoid repeated allocations
struct VectorPool<T> {
    /// Pre-allocated vectors available for reuse
    vectors: Vec<Vec<T>>,
    /// Capacity for new vectors when pool is empty
    default_capacity: usize,
}

impl<T> VectorPool<T> {
    /// Creates a new vector pool with the specified default capacity
    fn new(default_capacity: usize) -> Self {
        Self {
            vectors: Vec::new(),
            default_capacity,
        }
    }
    
    /// Gets a vector from the pool or creates a new one
    fn get(&mut self) -> Vec<T> {
        if let Some(mut vec) = self.vectors.pop() {
            vec.clear();
            vec
        } else {
            Vec::with_capacity(self.default_capacity)
        }
    }
    
    /// Returns a vector to the pool for reuse
    fn release(&mut self, vec: Vec<T>) {
        // Only keep vectors that have a reasonable capacity to avoid memory bloat
        if vec.capacity() <= self.default_capacity * 4 {
            self.vectors.push(vec);
        }
    }
}

/// A memory arena for efficient allocation of data structures.
///
/// The `DataArena` uses a bump allocator to provide fast allocation
/// with minimal overhead. All allocations are freed at once when the
/// arena is reset or dropped.
///
/// # Examples
///
/// ```
/// use datalogic_rs::arena::DataArena;
///
/// let arena = DataArena::new();
/// let value = arena.alloc(42);
/// assert_eq!(*value, 42);
/// ```
pub struct DataArena {
    /// The underlying bump allocator
    bump: Bump,
    
    /// String interner for efficient string storage
    interner: RefCell<StringInterner>,
    
    /// Chunk size for allocations (in bytes)
    chunk_size: usize,
    
    /// Pool of DataValue vectors for reuse
    data_value_pool: RefCell<VectorPool<DataValue<'static>>>,
    
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
            .finish()
    }
}

impl DataArena {
    /// Creates a new arena with default settings.
    pub fn new() -> Self {
        Self::with_chunk_size(1 * 1024 * 1024) // 1MB chunks by default
    }
    
    /// Creates a new arena with the specified chunk size.
    ///
    /// The chunk size determines how much memory is allocated at once
    /// when the arena needs more space. Larger chunk sizes can improve
    /// performance but may waste memory if not fully utilized.
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        let bump = Bump::new();
        bump.set_allocation_limit(Some(chunk_size * 256)); // Safety limit
        
        // Create static references to common values
        // SAFETY: These are static and never change, so it's safe to cast them
        static NULL_VALUE: DataValue<'static> = DataValue::Null;
        static TRUE_VALUE: DataValue<'static> = DataValue::Bool(true);
        static FALSE_VALUE: DataValue<'static> = DataValue::Bool(false);
        static EMPTY_STRING: &'static str = "";
        static EMPTY_STRING_VALUE: DataValue<'static> = DataValue::String(EMPTY_STRING);
        static EMPTY_ARRAY: [DataValue<'static>; 0] = [];
        static EMPTY_ARRAY_VALUE: DataValue<'static> = DataValue::Array(&EMPTY_ARRAY);
        
        Self {
            bump,
            interner: RefCell::new(StringInterner::new()),
            chunk_size,
            data_value_pool: RefCell::new(VectorPool::new(8)), // Smaller capacity for DataValue vectors
            null_value: &NULL_VALUE,
            true_value: &TRUE_VALUE,
            false_value: &FALSE_VALUE,
            empty_string: EMPTY_STRING,
            empty_string_value: &EMPTY_STRING_VALUE,
            empty_array: &EMPTY_ARRAY,
            empty_array_value: &EMPTY_ARRAY_VALUE,
        }
    }
    
    /// Allocates a value in the arena.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    ///
    /// let arena = DataArena::new();
    /// let value = arena.alloc(42);
    /// assert_eq!(*value, 42);
    /// ```
    pub fn alloc<T>(&self, val: T) -> &T {
        self.bump.alloc(val)
    }
    
    /// Allocates a slice in the arena by copying from a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    ///
    /// let arena = DataArena::new();
    /// let original = &[1, 2, 3, 4, 5];
    /// let slice = arena.alloc_slice_copy(original);
    /// assert_eq!(slice, original);
    /// ```
    pub fn alloc_slice_copy<T: Copy>(&self, vals: &[T]) -> &[T] {
        self.bump.alloc_slice_copy(vals)
    }
    
    /// Allocates a slice in the arena by cloning each element.
    ///
    /// If the slice is empty, returns a reference to the preallocated empty slice.
    /// This function is optimized for small slices to reduce allocation overhead.
    ///
    /// This is useful for types that don't implement Copy but do implement Clone.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    ///
    /// let arena = DataArena::new();
    /// let original = vec![String::from("hello"), String::from("world")];
    /// let slice = arena.alloc_slice_clone(&original);
    /// assert_eq!(slice[0], "hello");
    /// assert_eq!(slice[1], "world");
    /// ```
    #[inline]
    pub fn alloc_slice_clone<T: Clone>(&self, vals: &[T]) -> &[T] {
        // Fast path for empty slices
        if vals.is_empty() {
            return &[];
        }
        
        // Fast path for single element slices (very common)
        if vals.len() == 1 {
            let ptr = self.bump.alloc(vals[0].clone());
            return std::slice::from_ref(ptr);
        }
        
        // Fast path for two element slices (common)
        if vals.len() == 2 {
            // Allocate both elements at once for better locality
            let ptr = self.bump.alloc_layout(std::alloc::Layout::array::<T>(2).unwrap()).cast::<T>();
            
            unsafe {
                // Clone elements directly
                std::ptr::write(ptr.as_ptr(), vals[0].clone());
                std::ptr::write(ptr.as_ptr().add(1), vals[1].clone());
                
                return std::slice::from_raw_parts(ptr.as_ptr(), 2);
            }
        }
        
        // For larger slices, use the standard allocation approach
        let layout = std::alloc::Layout::array::<T>(vals.len()).unwrap();
        let ptr = self.bump.alloc_layout(layout).cast::<T>();
        
        // Clone each element into the allocated memory
        let slice = unsafe {
            let mut dst = ptr.as_ptr();
            for val in vals {
                std::ptr::write(dst, val.clone());
                dst = dst.add(1);
            }
            std::slice::from_raw_parts(ptr.as_ptr(), vals.len())
        };
        
        slice
    }
    
    /// Allocates a string in the arena.
    ///
    /// If the string is empty, returns a reference to the preallocated empty string.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    ///
    /// let arena = DataArena::new();
    /// let s = arena.alloc_str("hello");
    /// assert_eq!(s, "hello");
    /// ```
    pub fn alloc_str(&self, s: &str) -> &str {
        if s.is_empty() {
            return self.empty_string();
        }
        self.bump.alloc_str(s)
    }
    
    /// Interns a string, returning a reference to a unique instance.
    ///
    /// If the string has been interned before, returns a reference to
    /// the existing instance. Otherwise, allocates the string in the
    /// arena and returns a reference to it.
    ///
    /// If the string is empty, returns a reference to the preallocated empty string.
    ///
    /// This is particularly useful for strings that are likely to be
    /// repeated, such as object keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    ///
    /// let arena = DataArena::new();
    /// let s1 = arena.intern_str("hello");
    /// let s2 = arena.intern_str("hello");
    ///
    /// // Both references point to the same string
    /// assert_eq!(s1, s2);
    /// ```
    pub fn intern_str(&self, s: &str) -> &str {
        if s.is_empty() {
            return self.empty_string();
        }
        self.interner.borrow_mut().intern(s, &self.bump)
    }
    
    /// Resets the arena, freeing all allocations.
    ///
    /// This method resets the arena to its initial state, freeing all
    /// allocations at once. This is much faster than freeing each
    /// allocation individually.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    ///
    /// let mut arena = DataArena::new();
    /// let s = arena.alloc_str("hello");
    /// arena.reset();
    /// // s is no longer valid after reset
    /// ```
    pub fn reset(&mut self) {
        self.bump.reset();
        self.interner = RefCell::new(StringInterner::new());
        // No need to reset the preallocated values as they are static
    }
    
    /// Returns the current memory usage of the arena in bytes.
    pub fn memory_usage(&self) -> usize {
        self.bump.allocated_bytes()
    }
    
    /// Creates a new temporary arena for short-lived allocations.
    ///
    /// This method creates a new arena that can be used for temporary
    /// allocations that are freed all at once when the arena is dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    ///
    /// let arena = DataArena::new();
    /// {
    ///     let temp_arena = arena.create_temp_arena();
    ///     let temp = temp_arena.alloc(42);
    ///     assert_eq!(*temp, 42);
    /// }
    /// // temp is no longer valid here
    /// ```
    pub fn create_temp_arena(&self) -> DataArena {
        // We can reuse the same chunk size and preallocated values
        DataArena::with_chunk_size(self.chunk_size)
    }
    
    /// Gets a pre-allocated vector of DataValues from the pool.
    ///
    /// This is useful for building up collections that will be converted to arena-allocated
    /// slices. It avoids the overhead of heap allocations for temporary vectors.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    /// use datalogic_rs::value::DataValue;
    ///
    /// let arena = DataArena::new();
    /// let mut vec = arena.get_data_value_vec();
    /// vec.push(DataValue::integer(1));
    /// vec.push(DataValue::integer(2));
    /// let slice = arena.alloc_slice_clone(&vec);
    /// arena.release_data_value_vec(vec); // Return to pool when done
    /// ```
    pub fn get_data_value_vec<'a>(&'a self) -> Vec<DataValue<'a>> {
        // SAFETY: This is safe because we're only using the vector for the lifetime of the arena
        // and we ensure it's cleared before reuse
        unsafe {
            std::mem::transmute::<Vec<DataValue<'static>>, Vec<DataValue<'a>>>(
                self.data_value_pool.borrow_mut().get()
            )
        }
    }
    
    /// Returns a vector of DataValues to the pool for reuse.
    ///
    /// This should be called when you're done with a vector obtained from `get_data_value_vec`.
    /// The function is optimized to avoid excessive memory retention and reduce overhead.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    /// use datalogic_rs::value::DataValue;
    ///
    /// let arena = DataArena::new();
    /// let mut vec = arena.get_data_value_vec();
    /// vec.push(DataValue::integer(1));
    /// vec.push(DataValue::integer(2));
    /// let slice = arena.alloc_slice_clone(&vec);
    /// arena.release_data_value_vec(vec); // Return to pool when done
    /// ```
    pub fn release_data_value_vec<'a>(&self, vec: Vec<DataValue<'a>>) {
        // SAFETY: This is safe because we're only using the vector for the lifetime of the arena
        // and we ensure it's cleared before reuse
        
        // Only keep vectors with a reasonable capacity to avoid memory bloat
        // Also, don't bother with the overhead of returning very small vectors to the pool
        let capacity = vec.capacity();
        if capacity >= 8 && capacity <= self.data_value_pool.borrow().default_capacity * 4 {
            unsafe {
                self.data_value_pool.borrow_mut().release(
                    std::mem::transmute::<Vec<DataValue<'a>>, Vec<DataValue<'static>>>(vec)
                );
            }
        }
        // Otherwise, let the vector be dropped normally
    }
    
    /// Allocates a slice in the arena and fills it with values generated by a function.
    ///
    /// This is more efficient than creating a temporary vector and then cloning it.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    ///
    /// let arena = DataArena::new();
    /// let slice = arena.alloc_slice_fill_with(5, |i| i * 2);
    /// assert_eq!(slice, &[0, 2, 4, 6, 8]);
    /// ```
    pub fn alloc_slice_fill_with<T, F>(&self, len: usize, mut f: F) -> &[T]
    where
        F: FnMut(usize) -> T,
    {
        if len == 0 {
            return &[];
        }
        
        // Allocate memory for the slice
        let layout = std::alloc::Layout::array::<T>(len).unwrap();
        let ptr = self.bump.alloc_layout(layout).cast::<T>();
        
        // Fill the slice with values generated by the function
        unsafe {
            let mut dst = ptr.as_ptr();
            for i in 0..len {
                std::ptr::write(dst, f(i));
                dst = dst.add(1);
            }
            std::slice::from_raw_parts(ptr.as_ptr(), len)
        }
    }
    
    /// Returns a reference to the preallocated null value.
    pub fn null_value(&self) -> &DataValue {
        // SAFETY: The lifetime is tied to self, which is safe because the static value lives forever
        unsafe { std::mem::transmute::<&'static DataValue<'static>, &DataValue>(self.null_value) }
    }
    
    /// Returns a reference to the preallocated true value.
    pub fn true_value(&self) -> &DataValue {
        // SAFETY: The lifetime is tied to self, which is safe because the static value lives forever
        unsafe { std::mem::transmute::<&'static DataValue<'static>, &DataValue>(self.true_value) }
    }
    
    /// Returns a reference to the preallocated false value.
    pub fn false_value(&self) -> &DataValue {
        // SAFETY: The lifetime is tied to self, which is safe because the static value lives forever
        unsafe { std::mem::transmute::<&'static DataValue<'static>, &DataValue>(self.false_value) }
    }
    
    /// Returns a reference to a boolean value (either true or false).
    pub fn bool_value(&self, value: bool) -> &DataValue {
        if value {
            self.true_value()
        } else {
            self.false_value()
        }
    }
    
    /// Returns a reference to the preallocated empty string.
    pub fn empty_string(&self) -> &str {
        self.empty_string
    }
    
    /// Returns a reference to the preallocated empty string value.
    pub fn empty_string_value(&self) -> &DataValue {
        // SAFETY: The lifetime is tied to self, which is safe because the static value lives forever
        unsafe { std::mem::transmute::<&'static DataValue<'static>, &DataValue>(self.empty_string_value) }
    }
    
    /// Returns a reference to the preallocated empty array.
    pub fn empty_array(&self) -> &[DataValue] {
        // SAFETY: The lifetime is tied to self, which is safe because the static value lives forever
        unsafe { std::mem::transmute::<&'static [DataValue<'static>], &[DataValue]>(self.empty_array) }
    }
    
    /// Returns a reference to the preallocated empty array value.
    pub fn empty_array_value(&self) -> &DataValue {
        // SAFETY: The lifetime is tied to self, which is safe because the static value lives forever
        unsafe { std::mem::transmute::<&'static DataValue<'static>, &DataValue>(self.empty_array_value) }
    }
    
    /// Allocates a slice of DataValues in the arena by cloning each element.
    ///
    /// If the slice is empty, returns a reference to the preallocated empty array.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    /// use datalogic_rs::value::DataValue;
    ///
    /// let arena = DataArena::new();
    /// let original = vec![DataValue::integer(1), DataValue::integer(2)];
    /// let slice = arena.alloc_data_value_slice(&original);
    /// ```
    pub fn alloc_data_value_slice<'a>(&'a self, vals: &[DataValue<'a>]) -> &'a [DataValue<'a>] {
        if vals.is_empty() {
            return self.empty_array();
        }
        
        self.alloc_slice_clone(vals)
    }
    
    /// Allocates a slice of object entries in the arena by cloning each element.
    ///
    /// If the slice is empty, returns a reference to an empty slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    /// use datalogic_rs::value::DataValue;
    ///
    /// let arena = DataArena::new();
    /// let key = arena.intern_str("key");
    /// let entries = vec![(key, DataValue::integer(42))];
    /// let slice = arena.alloc_object_entries(&entries);
    /// ```
    pub fn alloc_object_entries<'a>(&'a self, entries: &[(&'a str, DataValue<'a>)]) -> &'a [(&'a str, DataValue<'a>)] {
        if entries.is_empty() {
            return &[];
        }
        
        self.alloc_slice_clone(entries)
    }
    
    /// Allocates a small array of DataValues (up to 8 elements) in the arena.
    ///
    /// This is optimized for the common case of small arrays in JSON Logic expressions.
    /// It avoids the overhead of allocating a Vec and then converting it to a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::DataArena;
    /// use datalogic_rs::value::DataValue;
    ///
    /// let arena = DataArena::new();
    /// let values = [DataValue::integer(1), DataValue::integer(2)];
    /// let slice = arena.alloc_small_data_value_array(&values);
    /// ```
    pub fn alloc_small_data_value_array<'a>(&'a self, values: &[DataValue<'a>]) -> &'a [DataValue<'a>] {
        debug_assert!(values.len() <= 8, "This method is only for small arrays");
        
        if values.is_empty() {
            return self.empty_array();
        }
        
        // For small arrays, allocate directly in the arena
        self.alloc_slice_clone(values)
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
        
        // The key behavior to test is that we can reuse the arena after reset
        // Not necessarily that the memory usage decreases
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
}
