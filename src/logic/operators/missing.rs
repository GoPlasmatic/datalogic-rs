//! Missing operators for logic expressions.
//!
//! This module provides implementations for missing operators
//! such as missing and missing_some.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::operators::variable::evaluate_variable;

/// Direct property check without creating intermediate DataValues
/// This is more efficient than using evaluate_variable for simple existence checks
#[inline]
fn has_property<'a>(data: &'a DataValue<'a>, key: &str, arena: &'a DataArena) -> bool {
    // Fast path for empty key
    if key.is_empty() {
        return false;
    }

    // Fast path for direct property access (no dots)
    if !key.contains('.') {
        match data {
            DataValue::Object(obj) => {
                // For small objects, linear search is faster
                if obj.len() <= 8 {
                    for (k, _) in *obj {
                        if *k == key {
                            return true;
                        }
                    }
                    return false;
                }
                
                // For larger objects, binary search might be faster if the keys are sorted
                // Otherwise, consider using a hash lookup
                for (k, _) in *obj {
                    if *k == key {
                        return true;
                    }
                }
                return false;
            },
            DataValue::Array(arr) => {
                if let Ok(idx) = key.parse::<usize>() {
                    return idx < arr.len();
                }
                return false;
            },
            _ => return false,
        }
    }
    
    // Fast path for dot-notation - avoid the contains check for the common case
    // This branch prediction optimization helps when most keys don't have dots
    if let Some(dot_pos) = key.find('.') {
        // For simple one-level dot notation, handle directly without recursion
        if key[dot_pos+1..].find('.').is_none() {
            let parent_key = &key[..dot_pos];
            let child_key = &key[dot_pos+1..];
            
            // Get parent object directly
            match data {
                DataValue::Object(obj) => {
                    // Find parent property
                    for (k, v) in *obj {
                        if *k == parent_key {
                            // Check child property in parent
                            return has_property(v, child_key, arena);
                        }
                    }
                    return false;
                },
                DataValue::Array(arr) => {
                    // Try to parse parent as array index
                    if let Ok(idx) = parent_key.parse::<usize>() {
                        if idx < arr.len() {
                            return has_property(&arr[idx], child_key, arena);
                        }
                    }
                    return false;
                },
                _ => return false,
            }
        }
        
        // For multi-level paths, use the evaluate_variable with default = None
        // This is a fallback for complex paths
        let result = evaluate_variable(key, &None, data, arena).unwrap_or(DataValue::null());
        return !result.is_null();
    }

    // Fast path for direct property access
    match data {
        DataValue::Object(obj) => {
            // For small objects, linear search is faster than creating a HashMap
            // Direct comparison avoids string interning overhead
            for (k, _) in *obj {
                if *k == key {
                    return true;
                }
            }
            false
        },
        DataValue::Array(arr) => {
            // Fast path for numeric indices - most common case in array access
            let index = match key.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => return false // Early return for non-numeric keys
            };
            
            // Direct length check is faster than bounds check + access
            index < arr.len()
        },
        _ => false, // All other types have no properties
    }
}

/// Evaluates a missing operation.
pub fn eval_missing<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // If there are no arguments, return an empty array
    if args.is_empty() {
        return Ok(DataValue::Array(&[]));
    }
    
    // Check if the first argument is an array or if we have multiple arguments
    let first_arg = &args[0];
    let is_array = matches!(first_arg, Token::Literal(DataValue::Array(_)));
    //  match first_arg {
    //     Token::Literal(DataValue::Array(_)) => true,
    //     _ => false,
    // };
    
    // If we have multiple arguments, treat them as individual keys to check
    if args.len() > 1 || !is_array {
        // Get a pre-allocated vector from the pool
        let mut missing = arena.get_data_value_vec();
        
        // Check each argument
        for arg in args {
            let value = evaluate(arg, data, arena)?;
            
            // Handle the case where the value is an array (e.g., from merge operator)
            if let DataValue::Array(items) = &value {
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
                        return Err(LogicError::operator_error(
                            "missing", 
                            format!("Expected string key, got {:?}", item)
                        ));
                    }
                }
            } else if let Some(key) = value.as_str() {
                if !has_property(data, key, arena) {
                    // Reuse the string reference directly
                    if let DataValue::String(s) = &value {
                        missing.push(DataValue::String(s));
                    } else {
                        missing.push(DataValue::String(arena.intern_str(key)));
                    }
                }
            } else {
                // Release the vector back to the pool before returning an error
                arena.release_data_value_vec(missing);
                return Err(LogicError::operator_error(
                    "missing", 
                    format!("Expected string key or array of string keys, got {:?}", value)
                ));
            }
        }
        
        // Optimize the empty case - return a static empty array
        if missing.is_empty() {
            // Release the vector back to the pool
            arena.release_data_value_vec(missing);
            return Ok(DataValue::Array(&[]));
        }
        
        // Allocate the result array only once at the end
        let result = DataValue::Array(arena.alloc_slice_clone(&missing));
        
        // Release the vector back to the pool
        arena.release_data_value_vec(missing);
        
        return Ok(result);
    }
    
    // Get the keys to check from the first argument
    let value = evaluate(&args[0], data, arena)?;
    
    // Null data is a common special case
    let is_data_null = matches!(data, DataValue::Null);
    
    // Fast path for single string key with null data (common case)
    if is_data_null {
        if let DataValue::String(key) = &value {
            // Single key case - direct reuse of string reference
            return Ok(DataValue::Array(arena.alloc_slice_fill_with(1, |_| DataValue::String(key))));
        }
    }
    
    // Fast path for single string key with non-null data (common case)
    if let DataValue::String(key) = &value {
        // Check if the key is missing
        if !has_property(data, key, arena) {
            // Key is missing - return array with single key
            return Ok(DataValue::Array(arena.alloc_slice_fill_with(1, |_| DataValue::String(key))));
        } else {
            // Key is present - return empty array
            return Ok(DataValue::Array(&[]));
        }
    }
    
    // Get a pre-allocated vector from the pool
    let mut missing = arena.get_data_value_vec();
    
    // Handle the case where the value is an array
    if let DataValue::Array(items) = &value {
        // Check if we need to flatten nested arrays
        let mut has_array_items = false;
        for item in items.iter() {
            if let DataValue::Array(_) = item {
                has_array_items = true;
                break;
            }
        }
        
        if has_array_items {
            // We have nested arrays, need to flatten
            for item in items.iter() {
                if let DataValue::Array(nested_items) = item {
                    // Process each item in the nested array
                    for nested_item in nested_items.iter() {
                        if let Some(key) = nested_item.as_str() {
                            if is_data_null || !has_property(data, key, arena) {
                                // Reuse the string reference directly
                                if let DataValue::String(s) = nested_item {
                                    missing.push(DataValue::String(s));
                                } else {
                                    missing.push(DataValue::String(arena.intern_str(key)));
                                }
                            }
                        } else {
                            // Release the vector back to the pool before returning an error
                            arena.release_data_value_vec(missing);
                            return Err(LogicError::operator_error(
                                "missing", 
                                format!("Expected string key, got {:?}", nested_item)
                            ));
                        }
                    }
                } else if let Some(key) = item.as_str() {
                    if is_data_null || !has_property(data, key, arena) {
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
                    return Err(LogicError::operator_error(
                        "missing", 
                        format!("Expected string key or array of string keys, got {:?}", item)
                    ));
                }
            }
        } else {
            // Regular array case - no nested arrays
            if is_data_null {
                // Everything is missing when data is null
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        // Reuse the string reference directly
                        if let DataValue::String(s) = item {
                            missing.push(DataValue::String(s));
                        } else {
                            missing.push(DataValue::String(arena.intern_str(key)));
                        }
                    } else {
                        // Release the vector back to the pool before returning an error
                        arena.release_data_value_vec(missing);
                        return Err(LogicError::operator_error(
                            "missing", 
                            format!("Expected string key, got {:?}", item)
                        ));
                    }
                }
            } else {
                // Regular data case - need to check each key
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        // Direct property check without creating intermediate strings
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
                        return Err(LogicError::operator_error(
                            "missing", 
                            format!("Expected string key, got {:?}", item)
                        ));
                    }
                }
            }
        }
    } else {
        // Release the vector back to the pool before returning an error
        arena.release_data_value_vec(missing);
        return Err(LogicError::operator_error(
            "missing", 
            format!("Expected string or array, got {:?}", value)
        ));
    }
    
    // Optimize the empty case - return a static empty array
    if missing.is_empty() {
        // Release the vector back to the pool
        arena.release_data_value_vec(missing);
        return Ok(DataValue::Array(&[]));
    }
    
    // Allocate the result array only once at the end
    let result = DataValue::Array(arena.alloc_slice_clone(&missing));
    
    // Release the vector back to the pool
    arena.release_data_value_vec(missing);
    
    Ok(result)
}

/// Evaluates a missing_some operation.
pub fn eval_missing_some<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Check that we have exactly 2 arguments
    if args.len() != 2 {
        return Err(LogicError::operator_error(
            "missing_some", 
            format!("Expected 2 arguments, got {}", args.len())
        ));
    }
    
    // Evaluate the first argument to get the minimum required
    let min_required = evaluate(&args[0], data, arena)?;
    let min_required = match min_required.as_i64() {
        Some(n) => n as usize,
        None => return Err(LogicError::operator_error(
            "missing_some", 
            format!("Expected integer for minimum required, got {:?}", min_required)
        )),
    };
    
    // Early optimization for min_required = 0 - always return empty array
    if min_required == 0 {
        return Ok(DataValue::Array(&[]));
    }
    
    // Evaluate the second argument to get the keys
    let keys_value = evaluate(&args[1], data, arena)?;
    
    // Null data is a common special case
    let is_data_null = matches!(data, DataValue::Null);
    
    // Early return for null data with min_required > 0
    if is_data_null {
        match &keys_value {
            DataValue::Array(items) if !items.is_empty() => {
                // Fast path for small arrays with all string keys (common case)
                if items.len() <= 8 {
                    let mut all_strings = true;
                    // Get a pre-allocated vector from the pool for string references
                    let mut string_refs = arena.get_data_value_vec();
                    
                    for item in items.iter() {
                        if let Some(key) = item.as_str() {
                            if let DataValue::String(s) = item {
                                string_refs.push(DataValue::String(s));
                            } else {
                                string_refs.push(DataValue::String(arena.intern_str(key)));
                            }
                        } else {
                            all_strings = false;
                            break;
                        }
                    }
                    
                    if all_strings {
                        // All items are strings, create array directly
                        let result = DataValue::Array(arena.alloc_slice_clone(&string_refs));
                        
                        // Release the vector back to the pool
                        arena.release_data_value_vec(string_refs);
                        
                        return Ok(result);
                    }
                    
                    // Release the vector back to the pool if we didn't return
                    arena.release_data_value_vec(string_refs);
                }
                
                // Fallback for mixed types or larger arrays
                // With null data, all keys are missing
                // Get a pre-allocated vector from the pool
                let mut missing = arena.get_data_value_vec();
                
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        // Reuse the string reference directly
                        if let DataValue::String(s) = item {
                            missing.push(DataValue::String(s));
                        } else {
                            missing.push(DataValue::String(arena.intern_str(key)));
                        }
                    } else {
                        // Release the vector back to the pool before returning an error
                        arena.release_data_value_vec(missing);
                        return Err(LogicError::operator_error(
                            "missing_some", 
                            format!("Expected string keys, got {:?}", item)
                        ));
                    }
                }
                
                // Allocate the result array
                let result = DataValue::Array(arena.alloc_slice_clone(&missing));
                
                // Release the vector back to the pool
                arena.release_data_value_vec(missing);
                
                // Return the array of missing keys
                return Ok(result);
            },
            _ => return Ok(DataValue::Array(&[])),
        }
    }
    
    match keys_value {
        DataValue::Array(items) => {
            // Empty array case - nothing can be missing
            if items.is_empty() {
                return Ok(DataValue::Array(&[]));
            }
            
            // For small arrays, use a specialized fast path
            if items.len() <= 16 {
                // Get a pre-allocated vector from the pool
                let mut missing = arena.get_data_value_vec();
                let mut present_count = 0;
                
                // First pass: count present keys and collect missing keys
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        if has_property(data, key, arena) {
                            present_count += 1;
                            // Early exit if we have enough present keys
                            if present_count >= min_required {
                                // Release the vector back to the pool
                                arena.release_data_value_vec(missing);
                                return Ok(DataValue::Array(&[]));
                            }
                        } else {
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
                        return Err(LogicError::operator_error(
                            "missing_some", 
                            format!("Expected string keys, got {:?}", item)
                        ));
                    }
                }
                
                // Allocate the result array
                let result = DataValue::Array(arena.alloc_slice_clone(&missing));
                
                // Release the vector back to the pool
                arena.release_data_value_vec(missing);
                
                // Return the array of missing keys
                return Ok(result);
            }
            
            // For larger arrays, use a direct approach with early exit
            // Get a pre-allocated vector from the pool
            let mut missing = arena.get_data_value_vec();
            let mut present_count = 0;
            
            // Check which keys are missing, stop early if we have enough present keys
            for item in items.iter() {
                if let Some(key) = item.as_str() {
                    if has_property(data, key, arena) {
                        present_count += 1;
                        // Early exit if we have enough present keys
                        if present_count >= min_required {
                            // Release the vector back to the pool
                            arena.release_data_value_vec(missing);
                            return Ok(DataValue::Array(&[]));
                        }
                    } else {
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
                    return Err(LogicError::operator_error(
                        "missing_some", 
                        format!("Expected string keys, got {:?}", item)
                    ));
                }
            }
            
            // Allocate the result array
            let result = DataValue::Array(arena.alloc_slice_clone(&missing));
            
            // Release the vector back to the pool
            arena.release_data_value_vec(missing);
            
            // Return the array of missing keys
            Ok(result)
        },
        _ => Err(LogicError::operator_error(
            "missing_some", 
            format!("Expected array of keys, got {:?}", keys_value)
        )),
    }
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