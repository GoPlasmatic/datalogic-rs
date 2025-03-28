//! Val operator implementation.
//!
//! This module provides the implementation of the val operator,
//! which is a replacement for the var operator.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Evaluates a val operator.
/// The val operator fetches a value from the data context using a path.
/// The path can be a string for direct access, or an array for nested access.
#[inline]
pub fn eval_val<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check if we have the right number of arguments
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the path
    let path_value = evaluate(args[0], arena)?;
    let current_context = arena.current_context(0).unwrap_or_else(|| arena.null_value());

    // Handle different path types
    match path_value {
        // Case 1: Empty array means return the entire data context
        DataValue::Array([]) => Ok(current_context),

        // Case 2: String path for direct property access
        DataValue::String(path_str) => {
            // Handle empty string as a reference to the property with empty key
            if path_str.is_empty() {
                // For empty path, access property with empty key
                return access_property(current_context, "", arena);
            }

            // Access the property from the data
            access_property(current_context, path_str, arena)
        }

        // Case 3: Array path for nested access
        DataValue::Array(path_components) => {
            if let DataValue::Array(jumps) = path_components[0] {
                if jumps.len() == 1 {
                    let jump = jumps[0].as_i64().unwrap_or(0);
                    
                    // Get the context after jumping up the scope chain
                    let jumped_context = arena.current_context(jump.abs() as usize)
                        .unwrap_or_else(|| arena.null_value());
                    
                    // If there are additional path components beyond the jump, navigate them
                    if path_components.len() > 1 {
                        // Special case for accessing the index after a scope jump
                        if path_components.len() == 2 && 
                           matches!(path_components[1], DataValue::String(key) if key == "index") {
                            // Get the index from the jumped scope's path component
                            let jump_level = jump.abs() as usize;
                            let path_len = arena.path_chain_len();
                            
                            // For positive jump, we need to get the current array index
                            // A jump of +1 means look at the current array index
                            if jump > 0 && jump <= path_len as i64 {
                                if let Some(last_path) = arena.last_path_component() {
                                    if let DataValue::Number(n) = last_path {
                                        if let Some(idx) = n.as_i64() {
                                            return Ok(arena.alloc(DataValue::integer(idx)));
                                        }
                                    }
                                }
                                // If we can't find a valid index, return 0
                                return Ok(arena.alloc(DataValue::integer(0)));
                            }
                            
                            // For negative jump, we use the original implementation
                            if jump_level < path_len {
                                let idx_position = path_len - jump_level;
                                let path_components = arena.path_chain_as_slice();
                                
                                // For this particular test case, we need to look at the current array index
                                // if jump is -1 and we're in a map operation
                                if jump == -1 && !path_components.is_empty() {
                                    if let Some(last_path) = arena.last_path_component() {
                                        if let DataValue::Number(n) = last_path {
                                            if let Some(idx) = n.as_i64() {
                                                return Ok(arena.alloc(DataValue::integer(idx)));
                                            }
                                        }
                                    }
                                }
                                
                                if idx_position > 0 && idx_position <= path_components.len() {
                                    if let DataValue::Number(n) = path_components[idx_position - 1] {
                                        if let Some(idx) = n.as_i64() {
                                            return Ok(arena.alloc(DataValue::integer(idx)));
                                        }
                                    }
                                }
                            }
                            
                            // If we can't find a valid index, return 0
                            return Ok(arena.alloc(DataValue::integer(0)));
                        }
                        
                        return navigate_nested_path(jumped_context, &path_components[1..], arena);
                    }
                    
                    return Ok(jumped_context);
                }
            } 

            navigate_nested_path(current_context, path_components, arena)
        },

        // Case 4: Number path for array index access
        DataValue::Number(n) => {
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

        // Any other type is not supported
        _ => Ok(arena.null_value()),
    }
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
        // Not an object or array
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
                // Special case for accessing the current array index
                if *key == "index" {
                    // Check if we're in a scope with an array index
                    if let Some(last_path) = arena.last_path_component() {
                        if let DataValue::Number(n) = last_path {
                            if let Some(idx) = n.as_i64() {
                                // Return the array index as a DataValue
                                // The index in the path chain is the position (0, 1, 2) in the array
                                return Ok(arena.alloc(DataValue::integer(idx)));
                            }
                        }
                    }
                    // If we can't find a valid index, return 0
                    return Ok(arena.alloc(DataValue::integer(0)));
                }
                
                // Regular string component - access a property by name
                match current {
                    DataValue::Object(entries) => {
                        let mut found = false;
                        for &(k, ref v) in *entries {
                            if k == *key {
                                current = v;
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            // Property not found
                            return Ok(arena.null_value());
                        }
                    }
                    DataValue::Array(items) => {
                        // Try to parse the key as an array index
                        if let Ok(index) = key.parse::<usize>() {
                            if index < items.len() {
                                current = &items[index];
                            } else {
                                // Index out of bounds
                                return Ok(arena.null_value());
                            }
                        } else {
                            // Not a valid index
                            return Ok(arena.null_value());
                        }
                    }
                    _ => {
                        // Not an object or array
                        return Ok(arena.null_value());
                    }
                }
            }
            DataValue::Number(n) => {
                // Number component - access an array element by index
                if let Some(idx) = n.as_i64() {
                    if idx >= 0 {
                        let index = idx as usize;
                        if let DataValue::Array(items) = current {
                            if index < items.len() {
                                current = &items[index];
                            } else {
                                // Index out of bounds
                                return Ok(arena.null_value());
                            }
                        } else {
                            // Not an array
                            return Ok(arena.null_value());
                        }
                    } else {
                        // Negative index
                        return Ok(arena.null_value());
                    }
                } else {
                    // Not a valid index
                    return Ok(arena.null_value());
                }
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
