//! Variable operator implementation.
//!
//! This module provides the implementation of the variable operator.

use crate::arena::DataArena;
use crate::logic::error::Result;
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Evaluates a variable reference.
#[inline]
pub fn evaluate_variable<'a>(
    path: &str,
    default: &Option<&'a Token<'a>>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Handle empty path as a reference to the data itself
    if path.is_empty() {
        return Ok(data);
    }
    
    // Fast path for direct property access (no dots)
    if !path.contains('.') {
        return evaluate_simple_path(path, default, data, arena);
    }
    
    // For paths with dots, traverse the object tree without creating a Vec
    let mut current = data;
    let mut start = 0;
    let path_bytes = path.as_bytes();
    
    // Iterate through path components without allocating a Vec
    while start < path_bytes.len() {
        // Find the next dot or end of string
        let end = path_bytes[start..].iter()
            .position(|&b| b == b'.')
            .map(|pos| start + pos)
            .unwrap_or(path_bytes.len());
        
        // Extract the current component
        let component = unsafe { std::str::from_utf8_unchecked(&path_bytes[start..end]) };
        
        // Process this component
        match current {
            DataValue::Object(_) => {
                // Try to find the component in the object
                if let Some(value) = find_in_object(current, component) {
                    current = value;
                } else {
                    // Component not found, use default
                    return use_default_or_null(default, data, arena);
                }
            },
            DataValue::Array(_) => {
                // Try to parse the component as an index
                if let Ok(index) = component.parse::<usize>() {
                    if let Some(value) = get_array_index(current, index) {
                        current = value;
                    } else {
                        // Index out of bounds, use default
                        return use_default_or_null(default, data, arena);
                    }
                } else {
                    // Not a valid index, use default
                    return use_default_or_null(default, data, arena);
                }
            },
            _ => {
                // Not an object or array, use default
                return use_default_or_null(default, data, arena);
            }
        }
        
        // Move to the next component
        start = end + 1;
    }
    
    // Successfully traversed the entire path
    Ok(current)
}

/// Helper function to evaluate a simple path (no dots)
#[inline]
fn evaluate_simple_path<'a>(
    path: &str,
    default: &Option<&'a Token<'a>>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Special case for numeric indices - direct array access
    if let Ok(index) = path.parse::<usize>() {
        if let DataValue::Array(items) = data {
            if index < items.len() {
                return Ok(&items[index]);
            }
        }
        
        // Not found, use default
        return use_default_or_null(default, data, arena);
    }

    if let DataValue::Object(obj) = data {
        for (k, v) in *obj {
            if *k == path {
                return Ok(v);
            }
        }
    }
    
    // Not found, use default
    use_default_or_null(default, data, arena)
}

/// Helper function to find a key in an object
#[inline]
fn find_in_object<'a>(obj: &'a DataValue<'a>, key: &str) -> Option<&'a DataValue<'a>> {
    if let DataValue::Object(entries) = obj {
        // Fast path for small objects (common case)
        if entries.len() <= 8 {
            for (k, v) in *entries {
                // First check if the pointers are the same (interned strings)
                if std::ptr::eq(*k as *const str, key as *const str) {
                    return Some(v);
                }
                
                // Then check if the lengths are different (quick rejection)
                if k.len() != key.len() {
                    continue;
                }
                
                // For short keys, compare bytes directly instead of using string comparison
                if k.len() <= 16 {
                    let k_bytes = k.as_bytes();
                    let key_bytes = key.as_bytes();
                    let mut equal = true;
                    
                    // Manual byte-by-byte comparison
                    for i in 0..k.len() {
                        if k_bytes[i] != key_bytes[i] {
                            equal = false;
                            break;
                        }
                    }
                    
                    if equal {
                        return Some(v);
                    }
                } else {
                    // For longer keys, use the standard string comparison
                    if *k == key {
                        return Some(v);
                    }
                }
            }
        } else {
            // For larger objects, use the standard approach
            for (k, v) in *entries {
                // First check if the pointers are the same (interned strings)
                if std::ptr::eq(*k as *const str, key as *const str) {
                    return Some(v);
                }
                
                // Then check if the lengths are different (quick rejection)
                if k.len() != key.len() {
                    continue;
                }
                
                // Finally, compare the actual strings
                if *k == key {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// Helper function to get an index from an array
#[inline]
fn get_array_index<'a>(arr: &'a DataValue<'a>, index: usize) -> Option<&'a DataValue<'a>> {
    if let DataValue::Array(items) = arr {
        if index < items.len() {
            return Some(&items[index]);
        }
    }
    None
}

/// Helper function to use the default value or return null
#[inline]
fn use_default_or_null<'a>(
    default: &Option<&'a Token<'a>>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if let Some(default_token) = default {
        evaluate(default_token, data, arena)
    } else {
        Ok(arena.null_value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_evaluate_variable() {
        let arena = DataArena::new();
        let data_json = json!({
            "user": {
                "name": "Alice",
                "age": 30
            }
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Parse and evaluate a variable
        let token = parse_str(r#"{"var": "user.name"}"#, &arena).unwrap();
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("Alice"));
        
        // Test with default value (not used)
        let token = parse_str(r#"{"var": ["user.name", "Bob"]}"#, &arena).unwrap();
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("Alice"));
        
        // Test with default value (used)
        let token = parse_str(r#"{"var": ["user.email", "bob@example.com"]}"#, &arena).unwrap();
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("bob@example.com"));
    }
} 