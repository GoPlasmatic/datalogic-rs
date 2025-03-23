//! Variable operator implementation.
//!
//! This module provides the implementation of the variable operator.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
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
        
        // Extract the current component - we know the input is valid UTF-8
        // Use from_utf8_unchecked to avoid validation overhead
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

/// Evaluates an exists operation.
/// Checks whether the specified variable(s) exist in the data.
pub fn eval_exists<'a>(
    args: &'a [DataValue<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    let result = if args.len() == 1 {
        match &args[0] {
            // Case 1: Single string argument (simple key)
            DataValue::String(key) => data_has_property(data, key),
            
            // Case 2: Single array argument (array of path components)
            DataValue::Array(path_components) => {
                // Convert array elements to a slice of string references
                let string_components: Vec<&str> = collect_string_components(path_components);
                if string_components.len() != path_components.len() {
                    // Not all components were strings
                    false
                } else {
                    check_nested_path_exists(data, &string_components)
                }
            },
            
            // Invalid argument type
            _ => false
        }
    } else {
        // Case 3: Multiple arguments (each arg is a path component)
        // Convert arguments to a slice of string references
        let string_components: Vec<&str> = collect_string_components(args);
        if string_components.len() != args.len() {
            // Not all arguments were strings
            false
        } else {
            check_nested_path_exists(data, &string_components)
        }
    };

    Ok(arena.alloc(DataValue::Bool(result)))
}

/// Collects string components from DataValue array or slice
/// Returns a vector of string references, or empty vector if any non-string values are found
#[inline]
fn collect_string_components<'a>(values: &'a [DataValue<'a>]) -> Vec<&'a str> {
    let mut result = Vec::with_capacity(values.len());
    
    for value in values {
        if let DataValue::String(s) = value {
            result.push(*s);
        } else {
            return Vec::new(); // Return empty vec if any non-string value
        }
    }
    
    result
}

/// Check if a nested path exists in the data
/// Takes a slice of path components and verifies if the path exists
#[inline]
fn check_nested_path_exists<'a>(data: &'a DataValue<'a>, path_components: &[&str]) -> bool {
    // Empty path is always false
    if path_components.is_empty() {
        return false;
    }
    
    // Single component - simple property check
    if path_components.len() == 1 {
        return data_has_property(data, path_components[0]);
    }
    
    // Navigate through multiple components
    let mut current = data;
    
    // Process all but the last component
    for (i, &key) in path_components.iter().enumerate() {
        // For all but the last component, navigate through the object
        if i < path_components.len() - 1 {
            if let Some(next) = data_get_property(current, key) {
                if let DataValue::Object(_) = next {
                    current = next;
                    continue;
                }
            }
            // If any intermediate path component doesn't exist or isn't an object,
            // the full path doesn't exist
            return false;
        } else {
            // For the last component, just check if the key exists
            return data_has_property(current, key);
        }
    }
    
    // This should never be reached given the logic above
    false
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
        // If the object has more than 8 entries, use binary search
        // This assumes entries are sorted by key, which should be enforced elsewhere
        if entries.len() > 8 {
            // Binary search for the key
            match entries.binary_search_by_key(&key, |&(k, _)| k) {
                Ok(idx) => return Some(&entries[idx].1),
                Err(_) => return None,
            }
        }
        
        // For small objects, linear search is faster due to cache locality
        for &(k, ref v) in *entries {
            if k == key {
                return Some(v);
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

/// Helper function to check if a property exists in the data
#[inline]
fn data_has_property<'a>(data: &'a DataValue<'a>, key: &str) -> bool {
    match data {
        DataValue::Object(obj) => {
            // Check if the key exists in the object
            obj.iter().any(|(k, _v)| *k == key)
        },
        _ => false,
    }
}

/// Helper function to get a property from the data
#[inline]
fn data_get_property<'a>(data: &'a DataValue<'a>, key: &str) -> Option<&'a DataValue<'a>> {
    match data {
        DataValue::Object(obj) => {
            // Find the key in the object
            obj.iter().find_map(|(k, v)| if *k == key { Some(v) } else { None })
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::FromJson;
    use crate::logic::JsonLogic;
    use serde_json::json;

    #[test]
    fn test_evaluate_variable() {
        use serde_json::json;
        use crate::logic::JsonLogic;

        // Setup for both low-level and builder-based testing
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({
            "a": 1,
            "b": 2,
            "c": 3,
            "d": null,
        });

        // Test basic variable access
        // Using builder API
        let rule = builder.var("a").build();
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(1));
        
        // Test with missing variable
        let rule = builder.var("x").build();
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(null));

        // Test with default value for missing variable
        let rule = builder.var_with_default("x", builder.string_value("DEFAULT"));
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!("DEFAULT"));

        // Test with default value for null property
        // Note: In JSONLogic, if a property exists but its value is null,
        // the default value is NOT used, null is returned
        let rule = builder.var_with_default("d", builder.string_value("DEFAULT"));
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(null));

        // Test with default value for existing variable
        let rule = builder.var_with_default("a", builder.string_value("DEFAULT"));
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(1));
    }
    
    #[test]
    fn test_evaluate_variable_with_array_path() {
        use serde_json::json;

        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();

        let data_json = json!({
            "users": [
                {
                    "name": "Alice",
                    "role": "admin",
                    "details": {
                        "age": 30,
                        "active": true
                    }
                },
                {
                    "name": "Bob",
                    "role": "user",
                    "details": {
                        "age": 25,
                        "active": false
                    }
                }
            ]
        });

        // Test with dot notation using the builder
        let rule = builder.var("users.0.name").build();
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!("Alice"));
        
        // Test nested path
        let rule = builder.var("users.1.details.age").build();
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(25));
        
        // Test with default value for missing path
        let rule = builder.var_with_default("users.2.name", builder.string_value("Not Found"));
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!("Not Found"));
        
        // Test with boolean value
        let rule = builder.var("users.0.details.active").build();
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn test_eval_exists() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let arena = logic.arena();
        
        // Create test data with deeply nested structure
        let data_json = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "level4": 42,
                        "empty": {}
                    },
                    "alt3": true
                }
            },
            "sibling": {
                "path": "value"
            }
        });

        // The exists operation doesn't have a direct builder method
        // so we need to use the original implementation for the tests
        let data = DataValue::from_json(&data_json, &arena);
        
        // Test case: Key exists
        let args = vec![DataValue::String("level1")];
        let result = eval_exists(arena.alloc_slice_clone(&args), &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test case: Key doesn't exist
        let args = vec![DataValue::String("nonexistent")];
        let result = eval_exists(arena.alloc_slice_clone(&args), &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
        
        // Test case: Nested key exists
        let args = vec![
            DataValue::String("level1"),
            DataValue::String("level2"),
            DataValue::String("level3"),
            DataValue::String("level4")
        ];
        let result = eval_exists(arena.alloc_slice_clone(&args), &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test case: Nested key doesn't exist
        let args = vec![
            DataValue::String("level1"),
            DataValue::String("level2"),
            DataValue::String("level3"),
            DataValue::String("nonexistent")
        ];
        let result = eval_exists(arena.alloc_slice_clone(&args), &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
    }
} 