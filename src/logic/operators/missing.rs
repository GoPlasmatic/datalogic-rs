//! Missing operators for logic expressions.
//!
//! This module provides implementations for missing operators
//! such as missing and missing_some.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;

/// Direct property check without creating intermediate DataValues
/// This is more efficient than using evaluate_variable for simple existence checks
#[inline]
fn has_property<'a>(data: &'a DataValue<'a>, key: &str, _arena: &'a DataArena) -> bool {
    // Fast path for empty key
    if key.is_empty() {
        return false;
    }

    // Fast path for direct property access (no dots)
    if !key.contains('.') {
        return has_simple_property(data, key);
    }
    
    // For paths with dots, traverse the object tree without creating a Vec
    let mut current = data;
    let mut start = 0;
    let key_bytes = key.as_bytes();
    
    // Iterate through path components without allocating a Vec
    while start < key_bytes.len() {
        // Find the next dot or end of string
        let end = key_bytes[start..].iter()
            .position(|&b| b == b'.')
            .map(|pos| start + pos)
            .unwrap_or(key_bytes.len());
        
        // Extract the current component
        let component = unsafe { std::str::from_utf8_unchecked(&key_bytes[start..end]) };
        
        // Process this component
        match current {
            DataValue::Object(_) => {
                if let Some(next) = find_in_object(current, component) {
                    current = next;
                } else {
                    return false;
                }
            },
            DataValue::Array(_) => {
                if let Ok(index) = component.parse::<usize>() {
                    if let Some(next) = get_array_index(current, index) {
                        current = next;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            },
            _ => {
                return false;
            }
        }
        
        // Move to the next component
        start = end + 1;
    }
    
    true
}

/// Helper function to check for a simple property (no dots)
#[inline]
fn has_simple_property<'a>(data: &'a DataValue<'a>, key: &str) -> bool {
    match data {
        DataValue::Object(obj) => {
            // For small objects, linear search is faster
            for (k, _) in *obj {
                // First check if the pointers are the same (interned strings)
                if std::ptr::eq(*k as *const str, key as *const str) {
                    return true;
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
                        return true;
                    }
                } else {
                    // For longer keys, use the standard string comparison
                    if *k == key {
                        return true;
                    }
                }
            }
            false
        },
        DataValue::Array(arr) => {
            if let Ok(idx) = key.parse::<usize>() {
                idx < arr.len()
            } else {
                false
            }
        },
        _ => false,
    }
}

/// Helper function to find a key in an object
#[inline]
fn find_in_object<'a>(obj: &'a DataValue<'a>, key: &str) -> Option<&'a DataValue<'a>> {
    if let DataValue::Object(entries) = obj {
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

/// Evaluates a missing operation.
#[inline]
pub fn eval_missing<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // If there are no arguments, return an empty array
    if args.is_empty() {
        return Ok(arena.alloc(DataValue::Array(&[])));
    }
    
    // Check if the first argument is an array or if we have multiple arguments
    let first_arg = &args[0];
    let is_array = match first_arg {
        Token::Literal(DataValue::Array(_)) => true,
        _ => false,
    };
    
    // If we have multiple arguments, treat them as individual keys to check
    if args.len() > 1 || !is_array {
        // Get a pre-allocated vector from the pool
        let mut missing = arena.get_data_value_vec();
        
        // Check each argument
        for arg in args {
            let value = evaluate(arg, data, arena)?;
            
            // Handle the case where the value is an array (e.g., from merge operator)
            if let DataValue::Array(items) = value {
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        if !has_property(data, key, arena) {
                            // Reuse the string reference directly
                            if let DataValue::String(s) = item {
                                missing.push(DataValue::String(s));
                            } else {
                                missing.push(DataValue::String(arena.intern_str(key)));
                            }
                        }
                    } else {
                        // Release the vector back to the pool before returning an error
                        arena.release_data_value_vec(missing);
                        return Err(LogicError::OperatorError {
                            operator: "missing".to_string(),
                            reason: format!("Expected string key, got {:?}", item),
                        });
                    }
                }
            } else if let Some(key) = value.as_str() {
                if !has_property(data, key, arena) {
                    // Reuse the string reference directly
                    if let DataValue::String(s) = value {
                        missing.push(DataValue::String(s));
                    } else {
                        missing.push(DataValue::String(arena.intern_str(key)));
                    }
                }
            } else {
                // Release the vector back to the pool before returning an error
                arena.release_data_value_vec(missing);
                return Err(LogicError::OperatorError {
                    operator: "missing".to_string(),
                    reason: format!("Expected string key, got {:?}", value),
                });
            }
        }
        
        // Create the result array
        let result = DataValue::Array(arena.alloc_slice_clone(&missing));
        arena.release_data_value_vec(missing);
        
        return Ok(arena.alloc(result));
    }
    
    // If the first argument is an array, check each key in the array
    if let Token::Literal(DataValue::Array(keys)) = first_arg {
        // Get a pre-allocated vector from the pool
        let mut missing = arena.get_data_value_vec();
        
        // Check each key in the array
        for key_value in keys.iter() {
            if let Some(key) = key_value.as_str() {
                if !has_property(data, key, arena) {
                    // Reuse the string reference directly
                    if let DataValue::String(s) = key_value {
                        missing.push(DataValue::String(s));
                    } else {
                        missing.push(DataValue::String(arena.intern_str(key)));
                    }
                }
            } else {
                // Release the vector back to the pool before returning an error
                arena.release_data_value_vec(missing);
                return Err(LogicError::OperatorError {
                    operator: "missing".to_string(),
                    reason: format!("Expected string key, got {:?}", key_value),
                });
            }
        }
        
        // Create the result array
        let result = DataValue::Array(arena.alloc_slice_clone(&missing));
        arena.release_data_value_vec(missing);
        
        return Ok(arena.alloc(result));
    }
    
    // If we get here, the first argument is not an array
    // Evaluate it and check if it's a string
    let value = evaluate(first_arg, data, arena)?;
    
    if let Some(key) = value.as_str() {
        if !has_property(data, key, arena) {
            // Return an array with the single missing key
            return Ok(arena.alloc(DataValue::Array(arena.alloc_slice_fill_with(1, |_| DataValue::String(arena.intern_str(key))))));
        } else {
            // Return an empty array
            return Ok(arena.alloc(DataValue::Array(&[])));
        }
    } else if let DataValue::Array(keys) = value {
        // Get a pre-allocated vector from the pool
        let mut missing = arena.get_data_value_vec();
        
        // Check each key in the array
        for key_value in keys.iter() {
            if let Some(key) = key_value.as_str() {
                if !has_property(data, key, arena) {
                    // Reuse the string reference directly
                    if let DataValue::String(s) = key_value {
                        missing.push(DataValue::String(s));
                    } else {
                        missing.push(DataValue::String(arena.intern_str(key)));
                    }
                }
            } else {
                // Release the vector back to the pool before returning an error
                arena.release_data_value_vec(missing);
                return Err(LogicError::OperatorError {
                    operator: "missing".to_string(),
                    reason: format!("Expected string key, got {:?}", key_value),
                });
            }
        }
        
        // Create the result array
        let result = DataValue::Array(arena.alloc_slice_clone(&missing));
        arena.release_data_value_vec(missing);
        
        return Ok(arena.alloc(result));
    } else {
        return Err(LogicError::OperatorError {
            operator: "missing".to_string(),
            reason: format!("Expected string key or array of keys, got {:?}", value),
        });
    }
}

/// Evaluates a missing_some operation.
#[inline]
pub fn eval_missing_some<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check that we have exactly 2 arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "missing_some".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument (minimum number of required keys)
    let min_required = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is a number
    let min = match min_required {
        DataValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                i as usize
            } else {
                n.as_f64() as usize
            }
        },
        _ => return Err(LogicError::OperatorError {
            operator: "missing_some".to_string(),
            reason: format!("First argument must be a number, got {:?}", min_required),
        }),
    };
    
    // Evaluate the second argument (array of keys)
    let keys = evaluate(&args[1], data, arena)?;
    
    // Check that the second argument is an array
    let keys_array = match keys {
        DataValue::Array(arr) => arr,
        _ => return Err(LogicError::OperatorError {
            operator: "missing_some".to_string(),
            reason: format!("Second argument must be an array, got {:?}", keys),
        }),
    };
    
    // If the array is empty, return an empty array
    if keys_array.is_empty() {
        return Ok(arena.alloc(DataValue::Array(&[])));
    }
    
    // Get a pre-allocated vector from the pool for missing keys
    let mut missing = arena.get_data_value_vec();
    
    // Get a pre-allocated vector from the pool for found keys
    let mut found = arena.get_data_value_vec();
    
    // Check each key in the array
    for key_value in keys_array.iter() {
        if let Some(key) = key_value.as_str() {
            if has_property(data, key, arena) {
                found.push(key_value.clone());
            } else {
                // Reuse the string reference directly
                if let DataValue::String(s) = key_value {
                    missing.push(DataValue::String(s));
                } else {
                    missing.push(DataValue::String(arena.intern_str(key)));
                }
            }
        } else {
            // Release the vectors back to the pool before returning an error
            arena.release_data_value_vec(missing);
            arena.release_data_value_vec(found);
            return Err(LogicError::OperatorError {
                operator: "missing_some".to_string(),
                reason: format!("Expected string key, got {:?}", key_value),
            });
        }
    }
    
    // If we have enough keys, return an empty array
    if found.len() >= min {
        arena.release_data_value_vec(missing);
        arena.release_data_value_vec(found);
        return Ok(arena.alloc(DataValue::Array(&[])));
    }
    
    // Otherwise, return the array of missing keys
    let result = DataValue::Array(arena.alloc_slice_clone(&missing));
    arena.release_data_value_vec(missing);
    arena.release_data_value_vec(found);
    
    Ok(arena.alloc(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_missing() {
        let arena = DataArena::new();
        let data_json = json!({
            "a": 1,
            "c": 3
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Test with a literal array
        let token = parse_str(r#"{"missing": ["a", "b", "c"]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str(), Some("b"));
        
        // Test with a single key
        let token = parse_str(r#"{"missing": "b"}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str(), Some("b"));
        
        // Test with multiple arguments
        let token = parse_str(r#"{"missing": ["a", "b", "c", "d"]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("b"));
        assert_eq!(arr[1].as_str(), Some("d"));
        
        // Test with null data
        let null_data = DataValue::null();
        let token = parse_str(r#"{"missing": ["a", "b"]}"#, &arena).unwrap();
        let result = evaluate(token, &null_data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("a"));
        assert_eq!(arr[1].as_str(), Some("b"));
        
        // Test with null data and single key
        let token = parse_str(r#"{"missing": "a"}"#, &arena).unwrap();
        let result = evaluate(token, &null_data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str(), Some("a"));
        
        // Test with dot notation
        let token = parse_str(r#"{"missing": ["a.b"]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str(), Some("a.b"));
        
        // Test with merge operator
        let data_json = json!({
            "vin": "123",
            "financing": true
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Test with merge operator that returns an array of strings
        let token = parse_str(r#"{"missing": {"merge": ["vin", {"if": [{"var": "financing"}, ["apr"], []]}]}}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str(), Some("apr"));
        
        // Test with merge operator that returns nested arrays
        let token = parse_str(r#"{"missing": {"merge": [["vin"], {"if": [{"var": "financing"}, ["apr"], []]}]}}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str(), Some("apr"));
    }
    
    #[test]
    fn test_missing_some() {
        let arena = DataArena::new();
        let data_json = json!({
            "a": 1,
            "c": 3
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Test with min_required = 1, should return empty array since we have 2 present
        let token = parse_str(r#"{"missing_some": [1, ["a", "b", "c", "d"]]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 0);
        
        // Test with min_required = 3, should return ["b", "d"] since we only have 2 present
        let token = parse_str(r#"{"missing_some": [3, ["a", "b", "c", "d"]]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("b"));
        assert_eq!(arr[1].as_str(), Some("d"));
        
        // Test with null data
        let null_data = DataValue::null();
        let token = parse_str(r#"{"missing_some": [1, ["a", "b", "c"]]}"#, &arena).unwrap();
        let result = evaluate(token, &null_data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_str(), Some("a"));
        assert_eq!(arr[1].as_str(), Some("b"));
        assert_eq!(arr[2].as_str(), Some("c"));
        
        // Test with min_required = 0, should always return empty array
        let token = parse_str(r#"{"missing_some": [0, ["a", "b", "c"]]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 0);
    }
} 