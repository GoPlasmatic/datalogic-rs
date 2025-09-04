//! Bump allocator for efficient arena-based memory management.
//!
//! This module provides a bump allocator that allows for efficient
//! allocation of memory with minimal overhead. All allocations are
//! freed at once when the arena is reset or dropped.
//!
//! The `DataArena` maintains shared references and context for evaluating
//! logic expressions.

use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use std::fmt;
use std::mem;

use crate::value::DataValue;

/// Default allocation size for vectors
const DEFAULT_VECTOR_CAPACITY: usize = 8;

/// An arena allocator for efficient data allocation.
///
/// The DataArena provides memory management for DataLogic values, with
/// optimized allocation strategies and context tracking for logic evaluation.
pub struct DataArena {
    /// The underlying bump allocator
    bump: Bump,

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
            chunk_size,
            null_value: &NULL_VALUE,
            true_value: &TRUE_VALUE,
            false_value: &FALSE_VALUE,
            empty_string: EMPTY_STRING,
            empty_string_value: &EMPTY_STRING_VALUE,
            empty_array: &EMPTY_ARRAY,
            empty_array_value: &EMPTY_ARRAY_VALUE,
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

    /// Gets a BumpVec for Token references with specified capacity
    pub fn get_token_vec<'a>(
        &'a self,
        capacity: usize,
    ) -> BumpVec<'a, &'a crate::logic::Token<'a>> {
        BumpVec::with_capacity_in(capacity, &self.bump)
    }

    /// Gets a BumpVec for structured object fields with specified capacity
    pub fn get_fields_vec<'a>(
        &'a self,
        capacity: usize,
    ) -> BumpVec<'a, (&'a str, &'a crate::logic::Token<'a>)> {
        BumpVec::with_capacity_in(capacity, &self.bump)
    }

    /// Gets an empty slice for object entries
    pub fn empty_object_entries(&self) -> &[(&str, DataValue<'_>)] {
        &[]
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

    /// Resets the arena, freeing all allocations.
    ///
    /// This clears all allocated memory.
    pub fn reset(&mut self) {
        self.bump.reset();
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
        // Use BumpVec to allocate in the arena instead of std::Vec
        let mut vec = BumpVec::with_capacity_in(vals.len(), &self.bump);
        vec.extend_from_slice(vals);
        self.bump_vec_into_slice(vec)
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

        // Use BumpVec to allocate in the arena instead of std::Vec
        let mut vec = BumpVec::with_capacity_in(entries.len(), &self.bump);
        vec.extend_from_slice(entries);
        self.bump_vec_into_slice(vec)
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
            let _ = arena.alloc_str(&format!("test string {i}"));
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
