//! Variable operator implementation.
//!
//! This module provides the implementation of the variable operator.

use crate::arena::DataArena;
use crate::context::EvalContext;
use crate::logic::error::Result;
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Evaluates a variable reference.
#[inline]
pub fn evaluate_variable<'a>(
    path: &str,
    default: &Option<&'a Token<'a>>,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let current_context = context.current();

    // Handle empty path as a reference to the data itself
    if path.is_empty() {
        return Ok(current_context);
    }

    // Fast path for direct property access (no dots)
    if !path.contains('.') {
        return evaluate_simple_path(path, default, current_context, context, arena);
    }

    // For paths with dots, process nested path
    process_nested_path(path, default, current_context, context, arena)
}

/// Process a nested path (with dots)
#[inline]
fn process_nested_path<'a>(
    path: &str,
    default: &Option<&'a Token<'a>>,
    current_context: &'a DataValue<'a>,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let mut current = current_context;
    let mut start = 0;
    let path_bytes = path.as_bytes();

    // Iterate through path components without allocating a Vec
    while start < path_bytes.len() {
        // Find the next dot or end of string
        let end = find_next_component_boundary(path_bytes, start);

        // Extract the current component - we know the input is valid UTF-8
        let component = extract_path_component(path_bytes, start, end);

        // Process this component based on current value type
        match current {
            DataValue::Object(_) => {
                current = match process_object_component(current, component) {
                    Some(value) => value,
                    None => return use_default_or_null(default, context, arena),
                }
            }
            DataValue::Array(_) => {
                current = match process_array_component(current, component) {
                    Some(value) => value,
                    None => return use_default_or_null(default, context, arena),
                }
            }
            _ => {
                // Not an object or array, use default
                return use_default_or_null(default, context, arena);
            }
        }

        // Move to the next component
        start = end + 1;
    }

    // Successfully traversed the entire path
    Ok(current)
}

/// Find the boundary index for the next path component
#[inline]
fn find_next_component_boundary(path_bytes: &[u8], start: usize) -> usize {
    path_bytes[start..]
        .iter()
        .position(|&b| b == b'.')
        .map(|pos| start + pos)
        .unwrap_or(path_bytes.len())
}

/// Extract a path component from the path bytes
#[inline]
fn extract_path_component(path_bytes: &[u8], start: usize, end: usize) -> &str {
    // Safe because we know the input is valid UTF-8
    unsafe { std::str::from_utf8_unchecked(&path_bytes[start..end]) }
}

/// Process a component when the current value is an object
#[inline]
fn process_object_component<'a>(
    obj: &'a DataValue<'a>,
    component: &str,
) -> Option<&'a DataValue<'a>> {
    find_in_object(obj, component)
}

/// Process a component when the current value is an array
#[inline]
fn process_array_component<'a>(
    arr: &'a DataValue<'a>,
    component: &str,
) -> Option<&'a DataValue<'a>> {
    // Try to parse the component as an index
    if let Ok(index) = component.parse::<usize>() {
        get_array_index(arr, index)
    } else {
        // Not a valid index
        None
    }
}

/// Helper function to evaluate a simple path (no dots)
#[inline]
fn evaluate_simple_path<'a>(
    path: &str,
    default: &Option<&'a Token<'a>>,
    data: &'a DataValue<'a>,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Special case for numeric indices - direct array access
    if let Ok(index) = path.parse::<usize>() {
        return handle_array_index_access(data, index, default, context, arena);
    }

    // Otherwise, look for a matching property in the object
    if let Some(value) = find_in_object(data, path) {
        return Ok(value);
    }

    // Not found, use default
    use_default_or_null(default, context, arena)
}

/// Handle direct array index access for simple paths
#[inline]
fn handle_array_index_access<'a>(
    data: &'a DataValue<'a>,
    index: usize,
    default: &Option<&'a Token<'a>>,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if let Some(value) = get_array_index(data, index) {
        return Ok(value);
    }

    // Not found or not an array, use default
    use_default_or_null(default, context, arena)
}

/// Helper function to find a key in an object
#[inline]
fn find_in_object<'a>(obj: &'a DataValue<'a>, key: &str) -> Option<&'a DataValue<'a>> {
    if let DataValue::Object(entries) = obj {
        // If the object has more than 8 entries, use binary search
        // This assumes entries are sorted by key, which should be enforced elsewhere
        if entries.len() > 8 {
            return find_in_large_object(entries, key);
        }

        // For small objects, linear search is faster due to cache locality
        return find_in_small_object(entries, key);
    }
    None
}

/// Find a key in a large object using binary search
#[inline]
fn find_in_large_object<'a>(
    entries: &'a [(&'a str, DataValue<'a>)],
    key: &str,
) -> Option<&'a DataValue<'a>> {
    // Binary search for the key
    match entries.binary_search_by_key(&key, |&(k, _)| k) {
        Ok(idx) => Some(&entries[idx].1),
        Err(_) => None,
    }
}

/// Find a key in a small object using linear search
#[inline]
fn find_in_small_object<'a>(
    entries: &'a [(&'a str, DataValue<'a>)],
    key: &str,
) -> Option<&'a DataValue<'a>> {
    for &(k, ref v) in entries {
        if k == key {
            return Some(v);
        }
    }
    None
}

/// Helper function to get an index from an array
#[inline]
fn get_array_index<'a>(arr: &'a DataValue<'a>, index: usize) -> Option<&'a DataValue<'a>> {
    if let DataValue::Array(items) = arr
        && index < items.len()
    {
        return Some(&items[index]);
    }
    None
}

/// Helper function to use the default value or return null
#[inline]
fn use_default_or_null<'a>(
    default: &Option<&'a Token<'a>>,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if let Some(default_token) = default {
        evaluate(default_token, context, arena)
    } else {
        Ok(arena.null_value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::{CustomOperatorRegistry, DataArena};
    use crate::logic::operators::val::eval_exists;
    use crate::logic::{DataLogicCore, Logic, OperatorType};
    use crate::value::{DataValue, FromJson};
    use serde_json::json;
    use std::sync::LazyLock;

    // Static empty operator registry for tests
    static EMPTY_OPERATORS: LazyLock<CustomOperatorRegistry> =
        LazyLock::new(CustomOperatorRegistry::new);

    #[test]
    fn test_variable_lookup() {
        let arena = DataArena::new();
        let core = DataLogicCore::new();

        // Create test data object: { "a": 1, "b": { "c": 2 } }
        let data_json = json!({
            "a": 1,
            "b": {
                "c": 2
            }
        });

        // For low-level testing, convert to DataValue
        let data = DataValue::from_json(&data_json, &arena);
        let data_ref = arena.alloc(data.clone());
        let context = EvalContext::new(data_ref, &EMPTY_OPERATORS);

        // Test simple variable access
        let path = "a";
        let result = evaluate_variable(path, &None, &context, &arena).unwrap();
        assert_eq!(result.as_i64(), Some(1));

        // Test nested variable access
        let path = "b.c";
        let result = evaluate_variable(path, &None, &context, &arena).unwrap();
        assert_eq!(result.as_i64(), Some(2));

        // Test missing variable with default
        let path = "d";
        let default_value = Token::literal(DataValue::string(&arena, "default"));
        let default_token = arena.alloc(default_value);
        let result = evaluate_variable(path, &Some(default_token), &context, &arena).unwrap();
        assert_eq!(result.as_str(), Some("default"));

        // Test using direct token creation
        // Simple variable access - create {"var": "a"}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let rule = Logic::new(a_var_ref, &arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(1));

        // Nested variable access - create {"var": "b.c"}
        let bc_var_token = Token::variable("b.c", None);
        let bc_var_ref = arena.alloc(bc_var_token);

        let rule = Logic::new(bc_var_ref, &arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(2));

        // Missing variable with default - create {"var": ["d", "default"]}
        let default_str_token = Token::literal(DataValue::string(&arena, "default"));
        let default_str_ref = arena.alloc(default_str_token);

        let d_var_token = Token::variable("d", Some(default_str_ref));
        let d_var_ref = arena.alloc(d_var_token);

        let rule = Logic::new(d_var_ref, &arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("default"));
    }

    #[test]
    fn test_variable_with_array_path() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Test data with arrays
        let data_json = json!({
            "users": [
                {"name": "Alice", "age": 25},
                {"name": "Bob", "age": 30},
                {"name": "Charlie", "age": 35}
            ]
        });

        // Test accessing array elements - create {"var": "users.0.name"}
        let users_0_name_var_token = Token::variable("users.0.name", None);
        let users_0_name_var_ref = arena.alloc(users_0_name_var_token);

        let rule = Logic::new(users_0_name_var_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("Alice"));

        // Test accessing multiple array elements with map function
        // Create {"map": [{"var": "users"}, {"var": "name"}]}
        let users_var_token = Token::variable("users", None);
        let users_var_ref = arena.alloc(users_var_token);

        let name_var_token = Token::variable("name", None);
        let name_var_ref = arena.alloc(name_var_token);

        let map_args = vec![users_var_ref, name_var_ref];
        let map_array_token = Token::ArrayLiteral(map_args);
        let map_array_ref = arena.alloc(map_array_token);

        let map_token = Token::operator(
            OperatorType::Array(crate::logic::operators::array::ArrayOp::Map),
            map_array_ref,
        );
        let map_ref = arena.alloc(map_token);

        let rule = Logic::new(map_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(["Alice", "Bob", "Charlie"]));
    }

    #[test]
    fn test_variable_with_missing_data() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Empty data
        let data_json = json!({});

        // Test with default value - create {"var": ["missing", "default"]}
        let default_str_token = Token::literal(DataValue::string(arena, "default"));
        let default_str_ref = arena.alloc(default_str_token);

        let missing_var_token = Token::variable("missing", Some(default_str_ref));
        let missing_var_ref = arena.alloc(missing_var_token);

        let rule = Logic::new(missing_var_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("default"));

        // Test without default (should return null) - create {"var": "missing"}
        let missing_var_token = Token::variable("missing", None);
        let missing_var_ref = arena.alloc(missing_var_token);

        let rule = Logic::new(missing_var_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(null));

        // Test deeply nested missing path - create {"var": "a.b.c.d"}
        let nested_var_token = Token::variable("a.b.c.d", None);
        let nested_var_ref = arena.alloc(nested_var_token);

        let rule = Logic::new(nested_var_ref, arena);

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
        let data_ref = arena.alloc(data.clone());
        let context = EvalContext::new(data_ref, &EMPTY_OPERATORS);

        // Test single path exists
        let path = DataValue::string(&arena, "a");
        let path_slice = arena.vec_into_slice(vec![path]);
        let result = eval_exists(path_slice, &context, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));

        // Test nested path exists
        let nested_path = DataValue::Array(arena.vec_into_slice(vec![
            DataValue::string(&arena, "b"),
            DataValue::string(&arena, "c"),
        ]));
        let nested_path_slice = arena.vec_into_slice(vec![nested_path]);
        let result = eval_exists(nested_path_slice, &context, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));

        // Test path doesn't exist
        let nonexistent_path = DataValue::string(&arena, "nonexistent");
        let nonexistent_path_slice = arena.vec_into_slice(vec![nonexistent_path]);
        let result = eval_exists(nonexistent_path_slice, &context, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));

        // Test nested path doesn't exist
        let nonexistent_nested_path = DataValue::Array(arena.vec_into_slice(vec![
            DataValue::string(&arena, "b"),
            DataValue::string(&arena, "nonexistent"),
        ]));
        let nonexistent_nested_path_slice = arena.vec_into_slice(vec![nonexistent_nested_path]);
        let result = eval_exists(nonexistent_nested_path_slice, &context, &arena).unwrap();
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
