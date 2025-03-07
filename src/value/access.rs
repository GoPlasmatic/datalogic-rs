//! Value access utilities for path-based value retrieval.
//!
//! This module provides utilities for accessing values in nested data structures
//! using path expressions.

use crate::arena::DataArena;
use super::data_value::DataValue;

/// A segment in a path expression.
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment<'a> {
    /// A key in an object.
    Key(&'a str),
    
    /// An index in an array.
    Index(usize),
}

impl<'a> PathSegment<'a> {
    /// Creates a new key segment.
    pub fn key(arena: &'a DataArena, key: &str) -> Self {
        PathSegment::Key(arena.intern_str(key))
    }
    
    /// Creates a new index segment.
    pub fn index(index: usize) -> Self {
        PathSegment::Index(index)
    }
    
    /// Parses a path segment from a string.
    pub fn parse(arena: &'a DataArena, segment: &str) -> Self {
        if let Ok(index) = segment.parse::<usize>() {
            PathSegment::Index(index)
        } else {
            PathSegment::Key(arena.intern_str(segment))
        }
    }
}

/// A trait for accessing values using path expressions.
pub trait ValueAccess<'a> {
    /// Gets a value using a path expression.
    fn get_path(&self, path: &[PathSegment<'a>]) -> Option<&DataValue<'a>>;
    
    /// Gets a value using a dot-separated path string.
    fn get_path_str(&self, arena: &'a DataArena, path: &str) -> Option<&DataValue<'a>>;
}

impl<'a> ValueAccess<'a> for DataValue<'a> {
    fn get_path(&self, path: &[PathSegment<'a>]) -> Option<&DataValue<'a>> {
        if path.is_empty() {
            return Some(self);
        }
        
        let (segment, rest) = path.split_first().unwrap();
        
        match segment {
            PathSegment::Key(key) => {
                match self {
                    DataValue::Object(entries) => {
                        for (k, v) in *entries {
                            if *k == *key {
                                if rest.is_empty() {
                                    return Some(v);
                                } else {
                                    return v.get_path(rest);
                                }
                            }
                        }
                        None
                    },
                    _ => None,
                }
            },
            PathSegment::Index(index) => {
                match self {
                    DataValue::Array(elements) => {
                        if let Some(value) = elements.get(*index) {
                            if rest.is_empty() {
                                Some(value)
                            } else {
                                value.get_path(rest)
                            }
                        } else {
                            None
                        }
                    },
                    _ => None,
                }
            },
        }
    }
    
    fn get_path_str(&self, arena: &'a DataArena, path: &str) -> Option<&DataValue<'a>> {
        if path.is_empty() {
            return Some(self);
        }
        
        // Use the parse_path function to get arena-allocated path segments
        let segments = parse_path(arena, path);
        self.get_path(segments)
    }
}

/// Parses a path string into a vector of path segments.
pub fn parse_path<'a>(arena: &'a DataArena, path: &str) -> &'a [PathSegment<'a>] {
    // Calculate the number of segments
    let segment_count = path.chars().filter(|&c| c == '.').count() + 1;
    
    // Pre-allocate a buffer for segments
    let mut segments = Vec::with_capacity(segment_count);
    
    // Fill the buffer
    for segment in path.split('.') {
        segments.push(PathSegment::parse(arena, segment));
    }
    
    // Return a slice from the arena
    arena.alloc_slice_clone(&segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;

    #[test]
    fn test_path_segment_parsing() {
        let arena = DataArena::new();
        
        let key = PathSegment::parse(&arena, "name");
        let index = PathSegment::parse(&arena, "42");
        
        assert_eq!(key, PathSegment::Key(arena.intern_str("name")));
        assert_eq!(index, PathSegment::Index(42));
    }

    #[test]
    fn test_value_access() {
        let arena = DataArena::new();
        
        // Create a nested object
        let user = DataValue::object(&arena, &[
            (arena.intern_str("name"), DataValue::string(&arena, "John")),
            (arena.intern_str("age"), DataValue::integer(30)),
            (arena.intern_str("address"), DataValue::object(&arena, &[
                (arena.intern_str("city"), DataValue::string(&arena, "New York")),
                (arena.intern_str("zip"), DataValue::string(&arena, "10001")),
            ])),
            (arena.intern_str("scores"), DataValue::array(&arena, &[
                DataValue::integer(85),
                DataValue::integer(90),
                DataValue::integer(95),
            ])),
        ]);
        
        // Test path access
        assert_eq!(user.get_path_str(&arena, "name").unwrap().as_str(), Some("John"));
        assert_eq!(user.get_path_str(&arena, "age").unwrap().as_i64(), Some(30));
        assert_eq!(user.get_path_str(&arena, "address.city").unwrap().as_str(), Some("New York"));
        assert_eq!(user.get_path_str(&arena, "scores.1").unwrap().as_i64(), Some(90));
        
        // Test with explicit path segments
        let path = vec![
            PathSegment::key(&arena, "address"),
            PathSegment::key(&arena, "zip"),
        ];
        assert_eq!(user.get_path(&path).unwrap().as_str(), Some("10001"));
        
        // Test non-existent paths
        assert_eq!(user.get_path_str(&arena, "email"), None);
        assert_eq!(user.get_path_str(&arena, "address.country"), None);
        assert_eq!(user.get_path_str(&arena, "scores.5"), None);
    }
}
