//! String interner for efficient string deduplication.
//!
//! This module provides a string interner that allows for efficient storage of strings,
//! ensuring that identical strings are only stored once in memory. This is particularly
//! useful for JSON processing where the same string keys might appear many times.
//!
//! This implementation is based on the idea of "string interning" which is a technique
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::mem;

use bumpalo::Bump;

/// Computes a hash for the given string.
///
/// This function uses the DefaultHasher from the standard library.
#[inline]
fn compute_hash(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// String interner for efficient deduplication of strings.
///
/// The interner stores unique instances of strings and provides
/// references to them, ensuring that identical strings are only
/// stored once in memory. This reduces memory usage when processing
/// data with many repeated strings, such as JSON objects with
/// identical keys across many objects.
pub struct StringInterner {
    /// Map from string hash to interned string references
    map: HashMap<u64, Vec<&'static str>>,

    /// Counter for tracking the number of interned strings
    count: usize,
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl StringInterner {
    /// Creates a new empty string interner.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            count: 0,
        }
    }

    /// Creates a new string interner with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - The initial capacity for the interner
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            count: 0,
        }
    }

    /// Interns a string, returning a reference to the unique instance.
    ///
    /// If the string has already been interned, returns a reference to
    /// the existing instance. Otherwise, allocates the string in the
    /// provided bump allocator and stores a reference to it.
    ///
    /// # Arguments
    ///
    /// * `s` - The string to intern
    /// * `bump` - The bump allocator for allocation if needed
    ///
    /// # Returns
    ///
    /// A reference to the interned string, valid for the lifetime of the bump allocator
    pub fn intern<'a>(&mut self, s: &str, bump: &'a Bump) -> &'a str {
        // Compute hash of the string
        let hash = compute_hash(s);

        // Check if we have a bucket for this hash
        if let Some(bucket) = self.map.get(&hash) {
            // Check if the string is already interned
            for &stored_str in bucket {
                if stored_str == s {
                    // Found existing interned string, return it
                    // SAFETY: The static lifetime can be safely narrowed to match the bump's lifetime
                    return unsafe { mem::transmute::<&'static str, &'a str>(stored_str) };
                }
            }
        }

        // String not found, allocate and store it
        let allocated = bump.alloc_str(s);

        // SAFETY: We widen the lifetime from 'a to 'static for storage
        // This is safe because:
        // 1. The bump allocator owns the memory and won't deallocate it during its lifetime
        // 2. We'll only return references with lifetimes bound to the bump
        // 3. When transmuting back to 'a, we ensure we don't outlive the bump
        let static_str: &'static str =
            unsafe { mem::transmute::<&'a str, &'static str>(allocated) };

        // Store the string in the appropriate bucket
        self.map.entry(hash).or_default().push(static_str);

        self.count += 1;

        // Return the allocated reference with the correct lifetime
        allocated
    }

    /// Returns the number of unique strings interned.
    #[inline]
    pub fn _len(&self) -> usize {
        self.count
    }

    /// Checks if the interner is empty.
    #[inline]
    pub fn _is_empty(&self) -> bool {
        self.count == 0
    }

    /// Clears the interner, removing all interned strings.
    ///
    /// Note that this does not deallocate the strings, as they
    /// are managed by the bump allocators.
    #[inline]
    pub fn _clear(&mut self) {
        self.map.clear();
        self.count = 0;
    }

    /// Reserves capacity for at least the specified number of additional elements.
    ///
    /// # Arguments
    ///
    /// * `additional` - The number of additional elements to reserve capacity for
    pub fn _reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
    }

    /// Shrinks the capacity of the interner as much as possible.
    ///
    /// This may be useful after interning a large number of strings
    /// to reduce memory usage.
    pub fn _shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();

        // Shrink each bucket as well
        for bucket in self.map.values_mut() {
            bucket.shrink_to_fit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_interner_basic() {
        let bump = Bump::new();
        let mut interner = StringInterner::new();

        // Intern some strings
        let s1 = interner.intern("hello", &bump);
        let s2 = interner.intern("hello", &bump);
        let s3 = interner.intern("world", &bump);

        // Verify that identical strings return the same reference
        assert!(ptr::eq(s1, s2));

        // Verify that different strings return different references
        assert!(!ptr::eq(s1, s3));

        // Check length
        assert_eq!(interner._len(), 2);
    }

    #[test]
    fn test_interner_with_capacity() {
        let bump = Bump::new();
        let mut interner = StringInterner::with_capacity(100);

        // Intern a large number of strings
        for i in 0..50 {
            let s = format!("string{i}");
            interner.intern(&s, &bump);
        }

        // Check length
        assert_eq!(interner._len(), 50);
    }

    #[test]
    fn test_interner_clear() {
        let bump = Bump::new();
        let mut interner = StringInterner::new();

        // Intern some strings
        interner.intern("hello", &bump);
        interner.intern("world", &bump);

        // Clear the interner
        interner._clear();

        // Check that the interner is empty
        assert!(interner._is_empty());
        assert_eq!(interner._len(), 0);
    }

    #[test]
    fn test_hash_collisions() {
        let bump = Bump::new();
        let mut interner = StringInterner::new();

        // Create strings that may hash to the same value
        // (This is a simplistic test; real hash collisions are rare but possible)
        let strings = ["a", "b", "c", "d", "e", "f", "g", "h"];

        // Intern all strings
        let refs: Vec<_> = strings.iter().map(|&s| interner.intern(s, &bump)).collect();

        // Verify that all strings are correctly interned
        for (i, &s) in strings.iter().enumerate() {
            assert_eq!(refs[i], s);
        }

        // Verify that reinterning returns the same references
        for (i, &s) in strings.iter().enumerate() {
            assert!(ptr::eq(refs[i], interner.intern(s, &bump)));
        }
    }
}
