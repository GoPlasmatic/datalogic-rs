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
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let current_context = arena.current_context(0).unwrap();
    // Handle empty path as a reference to the data itself
    if path.is_empty() {
        return Ok(current_context);
    }

    // Fast path for direct property access (no dots)
    if !path.contains('.') {
        return evaluate_simple_path(path, default, current_context, arena);
    }

    // For paths with dots, traverse the object tree without creating a Vec
    let mut current = current_context;
    let mut start = 0;
    let path_bytes = path.as_bytes();

    // Iterate through path components without allocating a Vec
    while start < path_bytes.len() {
        // Find the next dot or end of string
        let end = path_bytes[start..]
            .iter()
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
                    return use_default_or_null(default, arena);
                }
            }
            DataValue::Array(_) => {
                // Try to parse the component as an index
                if let Ok(index) = component.parse::<usize>() {
                    if let Some(value) = get_array_index(current, index) {
                        current = value;
                    } else {
                        // Index out of bounds, use default
                        return use_default_or_null(default, arena);
                    }
                } else {
                    // Not a valid index, use default
                    return use_default_or_null(default, arena);
                }
            }
            _ => {
                // Not an object or array, use default
                return use_default_or_null(default, arena);
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
        return use_default_or_null(default, arena);
    }

    if let DataValue::Object(obj) = data {
        for (k, v) in *obj {
            if *k == path {
                return Ok(v);
            }
        }
    }

    // Not found, use default
    use_default_or_null(default, arena)
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
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if let Some(default_token) = default {
        evaluate(default_token, arena)
    } else {
        Ok(arena.null_value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;
    use crate::logic::{DataLogicCore, Logic, OperatorType};
    use crate::logic::operators::val::eval_exists;
    use crate::value::{DataValue, FromJson};

    use serde_json::json;

    #[test]
    fn test_variable_lookup() {
        let arena = DataArena::new();
        let core = DataLogicCore::new();
        let builder = core.builder();

        // Create test data object: { "a": 1, "b": { "c": 2 } }
        let data_json = json!({
            "a": 1,
            "b": {
                "c": 2
            }
        });

        // For low-level testing, convert to DataValue
        let data = DataValue::from_json(&data_json, &arena);
        let key = DataValue::String("$");
        arena.set_current_context(&data, &key);

        // Test simple variable access
        let path = "a";
        let result = evaluate_variable(path, &None, &arena).unwrap();
        assert_eq!(result.as_i64(), Some(1));

        // Test nested variable access
        let path = "b.c";
        let result = evaluate_variable(path, &None, &arena).unwrap();
        assert_eq!(result.as_i64(), Some(2));

        // Test missing variable with default
        let path = "d";
        let default_value = Token::literal(DataValue::string(&arena, "default"));
        let default_token = arena.alloc(default_value);
        let result = evaluate_variable(path, &Some(default_token), &arena).unwrap();
        assert_eq!(result.as_str(), Some("default"));

        // Test using builder API
        // Simple variable access
        let rule = builder.var("a").build();
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(1));

        // Nested variable access
        let rule = builder.var("b.c").build();
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(2));

        // Missing variable with default
        let rule = builder.var_with_default("d", builder.string_value("default"));
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("default"));
    }

    #[test]
    fn test_variable_with_array_path() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        // Test data with arrays
        let data_json = json!({
            "users": [
                {"name": "Alice", "age": 25},
                {"name": "Bob", "age": 30},
                {"name": "Charlie", "age": 35}
            ]
        });

        // Test accessing array elements
        let rule = builder.var("users.0.name").build();
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("Alice"));

        // Test accessing multiple array elements with map function
        let map_rule = builder
            .array()
            .map_op()
            .array_var("users")
            .mapper_var("name")
            .build();

        let result = core.apply(&map_rule, &data_json).unwrap();
        assert_eq!(result, json!(["Alice", "Bob", "Charlie"]));
    }

    #[test]
    fn test_variable_with_missing_data() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        // Empty data
        let data_json = json!({});

        // Test with default value
        let rule = builder.var_with_default("missing", builder.string_value("default"));
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("default"));

        // Test without default (should return null)
        let rule = builder.var("missing").build();
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(null));

        // Test deeply nested missing path
        let rule = builder.var("a.b.c.d").build();
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(null));
    }

    #[test]
    fn test_exists() {
        let arena = DataArena::new();
        let core = DataLogicCore::new();

        // Setup test data
        let data_json = json!({
            "a": 1,
            "b": {
                "c": 2
            }
        });

        let data = DataValue::from_json(&data_json, &arena);
        let key = DataValue::String("$");
        arena.set_current_context(&data, &key);

        // Test single path exists
        let path = DataValue::string(&arena, "a");
        let path_slice = arena.vec_into_slice(vec![path]);
        let result = eval_exists(path_slice, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test nested path exists
        let nested_path = DataValue::Array(arena.vec_into_slice(vec![
            DataValue::string(&arena, "b"),
            DataValue::string(&arena, "c")
        ]));
        let nested_path_slice = arena.vec_into_slice(vec![nested_path]);
        let result = eval_exists(nested_path_slice, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test path doesn't exist
        let nonexistent_path = DataValue::string(&arena, "nonexistent");
        let nonexistent_path_slice = arena.vec_into_slice(vec![nonexistent_path]);
        let result = eval_exists(nonexistent_path_slice, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
        
        // Test nested path doesn't exist
        let nonexistent_nested_path = DataValue::Array(arena.vec_into_slice(vec![
            DataValue::string(&arena, "b"),
            DataValue::string(&arena, "nonexistent")
        ]));
        let nonexistent_nested_path_slice = arena.vec_into_slice(vec![nonexistent_nested_path]);
        let result = eval_exists(nonexistent_nested_path_slice, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));

        // Test using direct operator creation
        let exists_rule = Logic::operator(
            OperatorType::Exists,
            vec![Logic::literal(DataValue::string(&arena, "a"), &arena)],
            &arena,
        );

        let result = core.apply(&exists_rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test with a non-existent path
        let exists_rule = Logic::operator(
            OperatorType::Exists,
            vec![Logic::literal(
                DataValue::string(&arena, "nonexistent"),
                &arena,
            )],
            &arena,
        );

        let result = core.apply(&exists_rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }
}
