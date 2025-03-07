//! String interning for efficient string storage and deduplication.
//!
//! This module provides a string interner that deduplicates strings,
//! reducing memory usage for repeated strings such as object keys.

use bumpalo::Bump;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::fmt;

/// A string reference with efficient equality comparison.
///
/// `StringRef` stores a reference to a string along with its hash,
/// allowing for efficient equality comparison without recomputing
/// the hash.
#[derive(Clone, Copy)]
struct StringRef<'a> {
    /// Reference to the string data
    data: &'a str,
    
    /// Precomputed hash of the string
    hash: u64,
}

impl<'a> StringRef<'a> {
    /// Creates a new `StringRef` from a string.
    fn new(s: &'a str) -> Self {
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        let hash = hasher.finish();
        
        Self {
            data: s,
            hash,
        }
    }
}

impl<'a> PartialEq for StringRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        // Fast path: check hash first
        if self.hash != other.hash {
            return false;
        }
        
        // Slow path: compare strings
        self.data == other.data
    }
}

impl<'a> Eq for StringRef<'a> {}

impl<'a> Hash for StringRef<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl<'a> fmt::Debug for StringRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StringRef({:?}, hash={})", self.data, self.hash)
    }
}

/// A string interner for efficient string storage and deduplication.
///
/// The `StringInterner` deduplicates strings, reducing memory usage
/// for repeated strings such as object keys.
///
/// # Examples
///
/// ```
/// use datalogic_rs::arena::{DataArena, StringInterner};
/// use bumpalo::Bump;
///
/// let bump = Bump::new();
/// let mut interner = StringInterner::new();
///
/// let s1 = interner.intern("hello", &bump);
/// let s2 = interner.intern("hello", &bump);
///
/// // Both references point to the same string
/// assert_eq!(s1, s2);
/// ```
#[derive(Default)]
pub struct StringInterner {
    /// Map of interned strings
    strings: HashMap<StringRef<'static>, ()>,
}

impl fmt::Debug for StringInterner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StringInterner")
            .field("interned_count", &self.strings.len())
            .finish()
    }
}

impl StringInterner {
    /// Creates a new string interner.
    pub fn new() -> Self {
        Self {
            strings: HashMap::new(),
        }
    }
    
    /// Interns a string, returning a reference to a unique instance.
    ///
    /// If the string has been interned before, returns a reference to
    /// the existing instance. Otherwise, allocates the string in the
    /// provided arena and returns a reference to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::arena::{DataArena, StringInterner};
    /// use bumpalo::Bump;
    ///
    /// let bump = Bump::new();
    /// let mut interner = StringInterner::new();
    ///
    /// let s1 = interner.intern("hello", &bump);
    /// let s2 = interner.intern("hello", &bump);
    ///
    /// // Both references point to the same string
    /// assert_eq!(s1, s2);
    /// ```
    pub fn intern<'a>(&mut self, s: &str, arena: &'a Bump) -> &'a str {
        // Create a temporary StringRef for lookup
        let temp_ref = StringRef::new(s);
        
        // Convert to 'static lifetime for HashMap lookup
        // This is safe because we only use the hash and string content for lookup
        let static_ref: StringRef<'static> = unsafe {
            std::mem::transmute(temp_ref)
        };
        
        // Check if the string is already interned
        if self.strings.contains_key(&static_ref) {
            // Find the existing string reference
            let existing = self.strings.keys()
                .find(|k| k.data == s)
                .unwrap();
            
            // Convert back to the arena's lifetime
            // This is safe because the string is allocated in the arena
            let existing_str: &'a str = unsafe {
                std::mem::transmute(existing.data)
            };
            
            return existing_str;
        }
        
        // Allocate the string in the arena
        let new_str = arena.alloc_str(s);
        
        // Create a new StringRef for the interned string
        let new_ref = StringRef::new(new_str);
        
        // Convert to 'static lifetime for HashMap storage
        // This is safe because the HashMap doesn't outlive the arena
        let static_new_ref: StringRef<'static> = unsafe {
            std::mem::transmute(new_ref)
        };
        
        // Store the interned string
        self.strings.insert(static_new_ref, ());
        
        new_str
    }
    
    /// Returns the number of interned strings.
    pub fn len(&self) -> usize {
        self.strings.len()
    }
    
    /// Returns true if no strings have been interned.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
    
    /// Clears the interner, removing all interned strings.
    pub fn clear(&mut self) {
        self.strings.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn test_intern() {
        let bump = Bump::new();
        let mut interner = StringInterner::new();
        
        let s1 = interner.intern("hello", &bump);
        let s2 = interner.intern("hello", &bump);
        let s3 = interner.intern("world", &bump);
        
        assert_eq!(s1, "hello");
        assert_eq!(s2, "hello");
        assert_eq!(s3, "world");
        
        // Same strings should yield same references
        assert_eq!(s1.as_ptr(), s2.as_ptr());
        
        // Different strings should yield different references
        assert_ne!(s1.as_ptr(), s3.as_ptr());
        
        // Interner should have 2 strings
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn test_clear() {
        let bump = Bump::new();
        let mut interner = StringInterner::new();
        
        interner.intern("hello", &bump);
        interner.intern("world", &bump);
        
        assert_eq!(interner.len(), 2);
        
        interner.clear();
        
        assert_eq!(interner.len(), 0);
        assert!(interner.is_empty());
    }

    #[test]
    fn test_string_ref() {
        let s1 = "hello";
        let s2 = "hello";
        let s3 = "world";
        
        let ref1 = StringRef::new(s1);
        let ref2 = StringRef::new(s2);
        let ref3 = StringRef::new(s3);
        
        assert_eq!(ref1, ref2);
        assert_ne!(ref1, ref3);
        
        // Same strings should have same hash
        assert_eq!(ref1.hash, ref2.hash);
        
        // Different strings should have different hash (with high probability)
        assert_ne!(ref1.hash, ref3.hash);
    }
}
