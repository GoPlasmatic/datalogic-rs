//! Val operator implementation.
//!
//! This module provides the implementation of the val operator,
//! which is a replacement for the var operator.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};

/// Validates arguments for val operator
fn validate_val_args(args: &[&Token]) -> Result<()> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }
    Ok(())
}

/// The val operator is used to access properties from the data context
/// Examples: {"val": "a"}, {"val": ["a", "b", "c"]}, {"val": 0}
#[inline]
pub fn eval_val<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    validate_val_args(args)?;

    // Evaluate the first argument to get the path
    let first_arg = evaluate(args[0], arena)?;

    // If we have a second argument, it's a property access on the first argument
    if args.len() > 1 {
        return handle_property_access(first_arg, args, arena);
    }

    // Regular val operator behavior (accessing data context)
    let current_context = arena
        .current_context(0)
        .unwrap_or_else(|| arena.null_value());

    // Fast path: String path access without scope jump (most common case)
    if let DataValue::String(path_str) = first_arg {
        return handle_string_path(path_str, current_context, arena);
    }

    // Process other path types (slower paths)
    process_complex_path(first_arg, current_context, arena)
}

/// Handles property access when we have a second argument
#[inline]
fn handle_property_access<'a>(
    first_arg: &'a DataValue<'a>,
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let property = evaluate(args[1], arena)?;

    if let DataValue::String(prop_name) = property {
        // Handle special object types first (datetime, duration)
        let special_access = handle_special_object_types(first_arg, prop_name, arena);
        if let Some(item) = special_access {
            return item;
        }

        // Handle direct datetime or duration values
        match first_arg {
            DataValue::DateTime(dt) => return access_datetime_property(dt, prop_name, arena),
            DataValue::Duration(dur) => return access_duration_property(dur, prop_name, arena),
            _ => {}
        }

        // Fall back to regular property access
        return access_property(first_arg, prop_name, arena);
    }

    // If property is not a string, return null
    Ok(arena.null_value())
}

/// Handle special object types like datetime and duration objects
#[inline]
fn handle_special_object_types<'a>(
    value: &'a DataValue<'a>,
    prop_name: &str,
    arena: &'a DataArena,
) -> Option<Result<&'a DataValue<'a>>> {
    if let DataValue::Object(entries) = value {
        // Handle datetime objects with {"datetime": dt} structure
        if let Some((_, DataValue::DateTime(dt))) = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("datetime"))
        {
            return Some(access_datetime_property(dt, prop_name, arena));
        }

        // Handle duration objects with {"timestamp": dur} structure
        if let Some((_, DataValue::Duration(dur))) = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("timestamp"))
        {
            return Some(access_duration_property(dur, prop_name, arena));
        }
    }

    None
}

/// Handle a simple string path
#[inline]
fn handle_string_path<'a>(
    path_str: &str,
    current_context: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Handle empty string as a reference to the property with empty key
    if path_str.is_empty() {
        return access_property(current_context, "", arena);
    }

    // Direct property access (most common case)
    access_property(current_context, path_str, arena)
}

/// Get the current index in an array operation context
#[inline]
fn get_current_index(arena: &DataArena) -> Result<&DataValue<'_>> {
    // Check if we're in a scope with an array index
    if let Some(DataValue::Number(n)) = arena.last_path_component() {
        if let Some(idx) = n.as_i64() {
            // Return the array index as a DataValue
            return Ok(arena.alloc(DataValue::integer(idx)));
        }
    }
    // If we can't find a valid index, return 0
    Ok(arena.alloc(DataValue::integer(0)))
}

/// Get the current key in an array operation context
#[inline]
fn get_current_key(arena: &DataArena) -> Result<&DataValue<'_>> {
    if let Some(DataValue::String(key)) = arena.last_path_component() {
        return Ok(arena.alloc(DataValue::string(arena, key)));
    }
    Ok(arena.null_value())
}

/// Process complex path expressions that may involve scope jumps or nested access
#[cold]
#[inline(never)]
fn process_complex_path<'a>(
    path_value: &'a DataValue<'a>,
    current_context: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match path_value {
        // Case 1: Empty array means return the entire data context
        DataValue::Array([]) => Ok(current_context),

        // Case 2: String path for direct property access
        // (Already handled in the fast path)
        DataValue::String(_) => unreachable!(),

        // Case 3: Array path for nested access
        DataValue::Array(path_components) => {
            if let DataValue::Array(jumps) = path_components[0] {
                if jumps.len() == 1 {
                    return handle_scope_jump(jumps, path_components, arena);
                }
            }

            navigate_nested_path(current_context, path_components, arena)
        }

        // Case 4: Number path for array index access
        DataValue::Number(n) => handle_numeric_path(n, current_context, arena),

        // Any other type is not supported
        _ => Ok(arena.null_value()),
    }
}

/// Handle a numeric path (array index access)
#[inline]
fn handle_numeric_path<'a>(
    n: &crate::value::NumberValue,
    current_context: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if let Some(idx) = n.as_i64() {
        if idx >= 0 {
            access_array_index(current_context, idx as usize, arena)
        } else {
            // Negative index
            Ok(arena.null_value())
        }
    } else {
        // Not a valid index
        Ok(arena.null_value())
    }
}

/// Handle scope jumps in path expressions
#[inline]
fn handle_scope_jump<'a>(
    jumps: &'a [DataValue<'a>],
    path_components: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let jump = jumps[0].as_i64().unwrap_or(0);

    // Get the context after jumping up the scope chain
    let jumped_context = arena
        .current_context(jump.unsigned_abs() as usize)
        .unwrap_or_else(|| arena.null_value());

    // If there are additional path components beyond the jump, navigate them
    if path_components.len() > 1 {
        // Special case for accessing the index after a scope jump
        if path_components.len() == 2 {
            if matches!(path_components[1], DataValue::String(key) if key == "index") {
                return handle_index_with_jump(jump, arena);
            } else if matches!(path_components[1], DataValue::String(key) if key == "key") {
                return get_current_key(arena);
            }
        }

        return navigate_nested_path(jumped_context, &path_components[1..], arena);
    }

    Ok(jumped_context)
}

/// Handle the special case of accessing "index" with a scope jump
#[cold]
#[inline(never)]
fn handle_index_with_jump(jump: i64, arena: &DataArena) -> Result<&DataValue<'_>> {
    let path_len = arena.path_chain_len();

    // For positive jump, we need to get the current array index
    // A jump of +1 means look at the current array index
    if jump > 0 && jump <= path_len as i64 {
        return get_current_index(arena);
    }

    // For negative jump, check if we're looking at the current array index
    let jump_level = jump.unsigned_abs() as usize;
    if jump == -1 {
        // For -1 jump, prioritize checking the current array index
        return get_current_index(arena);
    }

    // For other negative jumps, use path chain navigation
    if jump_level < path_len {
        return arena.with_path_chain(|path_components| {
            let idx_position = path_len - jump_level;

            if idx_position > 0 && idx_position <= path_components.len() {
                if let DataValue::Number(n) = path_components[idx_position - 1] {
                    if let Some(idx) = n.as_i64() {
                        return Ok(arena.alloc(DataValue::integer(idx)));
                    }
                }
            }

            // If we can't find a valid index, return 0
            Ok(arena.alloc(DataValue::integer(0)))
        });
    }

    // If we can't find a valid index, return 0
    Ok(arena.alloc(DataValue::integer(0)))
}

/// Access a property from an object or an array using a string key
#[inline]
fn access_property<'a>(
    data: &'a DataValue<'a>,
    key: &str,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match data {
        DataValue::Object(entries) => {
            // Look for the key in the object
            for &(k, ref v) in *entries {
                if k == key {
                    return Ok(v);
                }
            }

            // Check for special object types
            let special_access = handle_special_object_types(data, key, arena);
            if let Some(item) = special_access {
                return item;
            }

            // Key not found
            Ok(arena.null_value())
        }
        DataValue::Array(items) => {
            // Try to parse the key as an array index
            if let Ok(index) = key.parse::<usize>() {
                if index < items.len() {
                    return Ok(&items[index]);
                }
            }
            // Invalid index or out of bounds
            Ok(arena.null_value())
        }
        DataValue::DateTime(dt) => {
            // Direct access to datetime properties
            access_datetime_property(dt, key, arena)
        }
        DataValue::Duration(dur) => {
            // Direct access to duration properties
            access_duration_property(dur, key, arena)
        }
        // Not an object or array
        _ => Ok(arena.null_value()),
    }
}

/// Access properties of a DateTime value
#[inline]
fn access_datetime_property<'a>(
    dt: &DateTime<Utc>,
    key: &str,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match key {
        "year" => Ok(arena.alloc(DataValue::integer(dt.year() as i64))),
        "month" => Ok(arena.alloc(DataValue::integer(dt.month() as i64))),
        "day" => Ok(arena.alloc(DataValue::integer(dt.day() as i64))),
        "hour" => Ok(arena.alloc(DataValue::integer(dt.hour() as i64))),
        "minute" => Ok(arena.alloc(DataValue::integer(dt.minute() as i64))),
        "second" => Ok(arena.alloc(DataValue::integer(dt.second() as i64))),
        "timestamp" => Ok(arena.alloc(DataValue::integer(dt.timestamp()))),
        "iso" => Ok(arena.alloc(DataValue::datetime(*dt))),
        _ => Ok(arena.null_value()),
    }
}

/// Access properties of a Duration value
#[inline]
fn access_duration_property<'a>(
    dur: &Duration,
    key: &str,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match key {
        "days" => Ok(arena.alloc(DataValue::integer(dur.num_days()))),
        "hours" => Ok(arena.alloc(DataValue::integer(dur.num_hours() % 24))),
        "minutes" => Ok(arena.alloc(DataValue::integer(dur.num_minutes() % 60))),
        "seconds" => Ok(arena.alloc(DataValue::integer(dur.num_seconds() % 60))),
        "total_seconds" => Ok(arena.alloc(DataValue::integer(dur.num_seconds()))),
        _ => Ok(arena.null_value()),
    }
}

/// Access an array element by index
#[inline]
fn access_array_index<'a>(
    data: &'a DataValue<'a>,
    index: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if let DataValue::Array(items) = data {
        if index < items.len() {
            return Ok(&items[index]);
        }
    }
    // Not an array or index out of bounds
    Ok(arena.null_value())
}

/// Navigate through a nested path represented as an array of components
#[inline]
fn navigate_nested_path<'a>(
    data: &'a DataValue<'a>,
    path_components: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Start with the current data
    let mut current = data;

    // Navigate through each path component
    for component in path_components {
        match component {
            DataValue::String(key) => {
                // Handle string component
                current = match handle_string_component(current, key)? {
                    Some(value) => value,
                    None => return Ok(arena.null_value()),
                };
            }
            DataValue::Number(n) => {
                // Handle number component
                current = match handle_number_component(current, n)? {
                    Some(value) => value,
                    None => return Ok(arena.null_value()),
                };
            }
            _ => {
                // Unsupported path component type
                return Ok(arena.null_value());
            }
        }
    }

    // Successfully navigated through all components
    Ok(current)
}

/// Handle navigation through a string component in a path
#[inline]
fn handle_string_component<'a>(
    current: &'a DataValue<'a>,
    key: &str,
) -> Result<Option<&'a DataValue<'a>>> {
    match current {
        DataValue::Object(entries) => {
            // Look for the key in the object
            for &(k, ref v) in *entries {
                if k == key {
                    return Ok(Some(v));
                }
            }
            // Property not found
            Ok(None)
        }
        DataValue::Array(items) => {
            // Try to parse the key as an array index
            if let Ok(index) = key.parse::<usize>() {
                if index < items.len() {
                    return Ok(Some(&items[index]));
                }
            }
            // Invalid index or out of bounds
            Ok(None)
        }
        _ => {
            // Not an object or array
            Ok(None)
        }
    }
}

/// Handle navigation through a number component in a path
#[inline]
fn handle_number_component<'a>(
    current: &'a DataValue<'a>,
    n: &crate::value::NumberValue,
) -> Result<Option<&'a DataValue<'a>>> {
    if let Some(idx) = n.as_i64() {
        if idx >= 0 {
            let index = idx as usize;
            if let DataValue::Array(items) = current {
                if index < items.len() {
                    return Ok(Some(&items[index]));
                }
            }
        }
    }
    // Not an array, index out of bounds, or invalid index
    Ok(None)
}

/// Evaluates if a path exists in the input data.
pub fn eval_exists<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Validate arguments
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    let current_context = arena.current_context(0).unwrap();

    // Single string key case
    if args.len() == 1 {
        if let DataValue::String(key) = &args[0] {
            // Check if the key exists in the object
            let exists = match current_context {
                DataValue::Object(obj) => obj.iter().any(|(k, _)| *k == *key),
                _ => false,
            };
            return Ok(arena.alloc(DataValue::Bool(exists)));
        }

        // Handle array case for a nested path
        if let DataValue::Array(components) = &args[0] {
            if components.is_empty() {
                return Ok(arena.alloc(DataValue::Bool(true)));
            }

            // For array of strings, treat it as a nested path
            return check_nested_path_exists(components, current_context, arena);
        }
    }

    // Multiple arguments case - treat as a nested path
    check_nested_path_exists(args, current_context, arena)
}

/// Checks if a nested path exists in the data
fn check_nested_path_exists<'a>(
    components: &'a [DataValue<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let mut current = data;

    for (i, component) in components.iter().enumerate() {
        let next_data = match component {
            DataValue::String(key) => check_string_component_exists(current, key),
            DataValue::Number(n) => check_number_component_exists(current, n),
            _ => {
                // Unsupported component type
                return Ok(arena.alloc(DataValue::Bool(false)));
            }
        };

        if let Some(next) = next_data {
            current = next;
        } else {
            // Path component doesn't exist
            return Ok(arena.alloc(DataValue::Bool(false)));
        }

        // If this is the last component, we've successfully traversed the path
        if i == components.len() - 1 {
            return Ok(arena.alloc(DataValue::Bool(true)));
        }
    }

    // Completed traversal, path exists
    Ok(arena.alloc(DataValue::Bool(true)))
}

/// Check if a string component exists in the current data
#[inline]
fn check_string_component_exists<'a>(
    data: &'a DataValue<'a>,
    key: &str,
) -> Option<&'a DataValue<'a>> {
    match data {
        DataValue::Object(obj) => {
            for &(k, ref v) in *obj {
                if k == key {
                    return Some(v);
                }
            }
            None
        }
        _ => None,
    }
}

/// Check if a number component exists in the current data
#[inline]
fn check_number_component_exists<'a>(
    data: &'a DataValue<'a>,
    n: &crate::value::NumberValue,
) -> Option<&'a DataValue<'a>> {
    if let Some(idx) = n.as_i64() {
        if idx >= 0 {
            let idx_usize = idx as usize;
            if let DataValue::Array(arr) = data {
                if idx_usize < arr.len() {
                    return Some(&arr[idx_usize]);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::Logic;
    use crate::logic::OperatorType;
    use crate::logic::datalogic_core::DataLogicCore;
    use serde_json::json;

    #[test]
    fn test_eval_val_with_path_components() {
        let arena = DataArena::new();
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({
            "users": [
                {
                    "name": "Alice",
                    "details": {
                        "age": 30,
                        "active": true
                    }
                },
                {
                    "name": "Bob",
                    "details": {
                        "age": 25,
                        "active": false
                    }
                }
            ]
        });

        // Use a mix of strings and numbers for the path components
        let components: Vec<DataValue> = vec![
            DataValue::string(&arena, "users"),
            DataValue::integer(0),
            DataValue::string(&arena, "name"),
        ];
        let rule = builder.val_path(components);
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("Alice"));

        // Second test with different path
        let components: Vec<DataValue> = vec![
            DataValue::string(&arena, "users"),
            DataValue::integer(1),
            DataValue::string(&arena, "details"),
            DataValue::string(&arena, "age"),
        ];
        let rule = builder.val_path(components);
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(25));

        // Third test accessing boolean value
        let components: Vec<DataValue> = vec![
            DataValue::string(&arena, "users"),
            DataValue::integer(0),
            DataValue::string(&arena, "details"),
            DataValue::string(&arena, "active"),
        ];
        let rule = builder.val_path(components);
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
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

        let data = <DataValue as crate::value::FromJson>::from_json(&data_json, &arena);
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
            DataValue::string(&arena, "c"),
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
            DataValue::string(&arena, "nonexistent"),
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
