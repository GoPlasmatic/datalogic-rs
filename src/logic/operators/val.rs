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
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check if we have the right number of arguments
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the path
    let path_value = evaluate(args[0], data, arena)?;
    
    // Handle different path types
    match path_value {
        // Case 1: Empty array means return the entire data context
        DataValue::Array(items) if items.is_empty() => {
            Ok(data)
        },
        
        // Case 2: String path for direct property access
        DataValue::String(path_str) => {
            // Handle empty string as a reference to the property with empty key
            if path_str.is_empty() {
                // For empty path, access property with empty key
                return access_property(data, "", arena);
            }
            
            // Access the property from the data
            access_property(data, *path_str, arena)
        },
        
        // Case 3: Array path for nested access
        DataValue::Array(path_components) => {
            navigate_nested_path(data, path_components, arena)
        },
        
        // Case 4: Number path for array index access
        DataValue::Number(n) => {
            if let Some(idx) = n.as_i64() {
                if idx >= 0 {
                    access_array_index(data, idx as usize, arena)
                } else {
                    // Negative index
                    Ok(arena.null_value())
                }
            } else {
                // Not a valid index
                Ok(arena.null_value())
            }
        },
        
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
        },
        DataValue::Array(items) => {
            // Try to parse the key as an array index
            if let Ok(index) = key.parse::<usize>() {
                if index < items.len() {
                    return Ok(&items[index]);
                }
            }
            // Invalid index or out of bounds
            Ok(arena.null_value())
        },
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
                // String component - access a property by name
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
                    },
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
                    },
                    _ => {
                        // Not an object or array
                        return Ok(arena.null_value());
                    }
                }
            },
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
            },
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

    #[test]
    fn test_eval_val_simple() {
        let arena = DataArena::new();
        
        // Create test data: { "hello": 0 }
        let entries = arena.alloc_slice_clone(&[
            (arena.intern_str("hello"), DataValue::integer(0))
        ]);
        let data = DataValue::Object(entries);
        
        // Create "hello" token
        let hello_token = Token::literal(DataValue::string(&arena, "hello"));
        let args = arena.alloc_slice_copy(&[arena.alloc(hello_token)]);
        
        // Evaluate the val operator
        let result = eval_val(args, &data, &arena).unwrap();
        
        // Check the result
        assert_eq!(*result, DataValue::integer(0));
    }

    #[test]
    fn test_eval_val_nested() {
        let arena = DataArena::new();
        
        // Create test data: { "hello": { "world": 1 } }
        let world_entries = arena.alloc_slice_clone(&[
            (arena.intern_str("world"), DataValue::integer(1))
        ]);
        let world_obj = DataValue::Object(world_entries);
        
        let hello_entries = arena.alloc_slice_clone(&[
            (arena.intern_str("hello"), world_obj)
        ]);
        let data = DataValue::Object(hello_entries);
        
        // Create ["hello", "world"] token
        let path_tokens = arena.alloc_slice_clone(&[
            DataValue::string(&arena, "hello"),
            DataValue::string(&arena, "world"),
        ]);
        let path_array = DataValue::Array(path_tokens);
        let path_token = Token::literal(path_array);
        let args = arena.alloc_slice_copy(&[arena.alloc(path_token)]);
        
        // Evaluate the val operator
        let result = eval_val(args, &data, &arena).unwrap();
        
        // Check the result
        assert_eq!(*result, DataValue::integer(1));
    }

    #[test]
    fn test_eval_val_array_index() {
        let arena = DataArena::new();
        
        // Create test data: [1, 2]
        let array_items = arena.alloc_slice_clone(&[
            DataValue::integer(1),
            DataValue::integer(2),
        ]);
        let data = DataValue::Array(array_items);
        
        // Create [1] token (to access index 1)
        let index_token = Token::literal(DataValue::integer(1));
        let args = arena.alloc_slice_copy(&[arena.alloc(index_token)]);
        
        // Evaluate the val operator
        let result = eval_val(args, &data, &arena).unwrap();
        
        // Check the result
        assert_eq!(*result, DataValue::integer(2));
    }
    
    #[test]
    fn test_eval_val_empty_key() {
        let arena = DataArena::new();
        
        // Create test data: { "": 1 }
        let entries = arena.alloc_slice_clone(&[
            (arena.intern_str(""), DataValue::integer(1))
        ]);
        let data = DataValue::Object(entries);
        
        // Create "" token
        let empty_token = Token::literal(DataValue::string(&arena, ""));
        let args = arena.alloc_slice_copy(&[arena.alloc(empty_token)]);
        
        // Evaluate the val operator
        let result = eval_val(args, &data, &arena).unwrap();
        
        // Check the result - should be the value at the empty key (1)
        assert_eq!(*result, DataValue::integer(1));
    }
    
    #[test]
    fn test_eval_val_entire_context() {
        let arena = DataArena::new();
        
        // Create test data: { "": 21 }
        let entries = arena.alloc_slice_clone(&[
            (arena.intern_str(""), DataValue::integer(21))
        ]);
        let data = DataValue::Object(entries);
        
        // Create [] token
        let empty_array = DataValue::Array(arena.empty_array());
        let empty_token = Token::literal(empty_array);
        let args = arena.alloc_slice_copy(&[arena.alloc(empty_token)]);
        
        // Evaluate the val operator
        let result = eval_val(args, &data, &arena).unwrap();
        
        // Check the result
        assert_eq!(*result, data);
    }
} 