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
                // Fast path for numeric indices using a lookup table for common indices
                match key {
                    "0" => return arr.len() > 0,
                    "1" => return arr.len() > 1,
                    "2" => return arr.len() > 2,
                    "3" => return arr.len() > 3,
                    "4" => return arr.len() > 4,
                    "5" => return arr.len() > 5,
                    "6" => return arr.len() > 6,
                    "7" => return arr.len() > 7,
                    "8" => return arr.len() > 8,
                    "9" => return arr.len() > 9,
                    _ => {
                        if let Ok(idx) = key.parse::<usize>() {
                            return idx < arr.len();
                        }
                        return false;
                    }
                }
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
            // Avoid parsing for common small indices
            let index = match key {
                "0" => 0,
                "1" => 1,
                "2" => 2,
                "3" => 3,
                "4" => 4,
                "5" => 5,
                "6" => 6,
                "7" => 7,
                "8" => 8,
                "9" => 9,
                _ => {
                    // Try to parse as number for larger indices
                    match key.parse::<usize>() {
                        Ok(idx) => idx,
                        Err(_) => return false // Early return for non-numeric keys
                    }
                }
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
    
    // Get the keys to check
    let value = evaluate(&args[0], data, arena)?;
    
    // Null data is a common special case - everything is missing
    let is_data_null = matches!(data, DataValue::Null);
    
    // Create a single buffer for all cases to minimize allocations
    // We'll reuse this buffer for all operations
    let mut missing = Vec::new();
    
    // Early optimization for null data - everything is missing
    if is_data_null {
        match &value {
            DataValue::Array(items) => {
                // Reserve capacity to avoid reallocations
                missing.reserve(items.len() + args.len() - 1);
                
                // Everything is missing when data is null
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        // Reuse the string reference if possible
                        if let DataValue::String(s) = item {
                            missing.push(DataValue::String(*s));
                        } else {
                            missing.push(DataValue::String(arena.intern_str(key)));
                        }
                    } else {
                        return Err(LogicError::operator_error(
                            "missing", 
                            format!("Expected string keys, got {:?}", item)
                        ));
                    }
                }
            },
            DataValue::String(key) => {
                // Reserve for single key plus any additional args
                missing.reserve(args.len());
                
                // Single key case
                missing.push(DataValue::String(*key)); // Reuse the existing string reference
            },
            _ => {
                return Err(LogicError::operator_error(
                    "missing", 
                    format!("Expected string or array, got {:?}", value)
                ));
            }
        };
        
        // Add additional arguments if any
        for arg in &args[1..] {
            let value = evaluate(arg, data, arena)?;
            if let Some(key) = value.as_str() {
                // Reuse the string reference if possible
                if let DataValue::String(s) = &value {
                    missing.push(DataValue::String(*s));
                } else {
                    missing.push(DataValue::String(arena.intern_str(key)));
                }
            } else {
                return Err(LogicError::operator_error(
                    "missing", 
                    format!("Expected string key, got {:?}", value)
                ));
            }
        }
    } else {
        // Regular data case - need to check each key
        match &value {
            DataValue::Array(items) => {
                // Reserve capacity to avoid reallocations - assuming ~50% might be missing
                missing.reserve(items.len() / 2 + 1);
                
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        // Direct property check without creating intermediate strings
                        if !has_property(data, key, arena) {
                            // Try to reuse string references when possible
                            if let DataValue::String(s) = item {
                                missing.push(DataValue::String(*s));
                            } else {
                                missing.push(DataValue::String(arena.intern_str(key)));
                            }
                        }
                    } else {
                        return Err(LogicError::operator_error(
                            "missing", 
                            format!("Expected string keys, got {:?}", item)
                        ));
                    }
                }
            },
            DataValue::String(key) => {
                // Single key case - optimize for the common case
                if !has_property(data, key, arena) {
                    missing.push(DataValue::String(*key));
                }
            },
            _ => {
                return Err(LogicError::operator_error(
                    "missing", 
                    format!("Expected string or array, got {:?}", value)
                ));
            }
        };
        
        // Process additional arguments if any
        for arg in &args[1..] {
            let value = evaluate(arg, data, arena)?;
            if let Some(key) = value.as_str() {
                if !has_property(data, key, arena) {
                    // Try to reuse the string reference when possible
                    if let DataValue::String(s) = &value {
                        missing.push(DataValue::String(*s));
                    } else {
                        missing.push(DataValue::String(arena.intern_str(key)));
                    }
                }
            } else {
                return Err(LogicError::operator_error(
                    "missing", 
                    format!("Expected string key, got {:?}", value)
                ));
            }
        }
    }
    
    // Optimize the empty case
    if missing.is_empty() {
        return Ok(DataValue::Array(&[]));
    }
    
    // Return the array of missing keys
    Ok(DataValue::Array(arena.alloc_slice_clone(&missing)))
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
                // With null data, all keys are missing
                let mut missing = Vec::with_capacity(items.len());
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        // Try to reuse existing string references
                        if let DataValue::String(s) = item {
                            missing.push(DataValue::String(*s));
                        } else {
                            missing.push(DataValue::String(arena.intern_str(key)));
                        }
                    } else {
                        return Err(LogicError::operator_error(
                            "missing_some", 
                            format!("Expected string keys, got {:?}", item)
                        ));
                    }
                }
                
                // Return the array of missing keys
                return Ok(DataValue::Array(arena.alloc_slice_clone(&missing)));
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
            
            // For small arrays, it's more efficient to check all keys 
            // before deciding what to return rather than building the missing vector incrementally
            if items.len() <= 8 {
                let mut present_count = 0;
                let mut missing = Vec::with_capacity(items.len());
                
                // First pass: count present keys and collect missing keys
                for item in items.iter() {
                    if let Some(key) = item.as_str() {
                        if has_property(data, key, arena) {
                            present_count += 1;
                        } else {
                            // Try to reuse existing string references
                            if let DataValue::String(s) = item {
                                missing.push(DataValue::String(*s));
                            } else {
                                missing.push(DataValue::String(arena.intern_str(key)));
                            }
                        }
                    } else {
                        return Err(LogicError::operator_error(
                            "missing_some", 
                            format!("Expected string keys, got {:?}", item)
                        ));
                    }
                }
                
                // If we have enough present keys, return empty array
                if present_count >= min_required {
                    return Ok(DataValue::Array(&[]));
                }
                
                // Return the array of missing keys
                return Ok(DataValue::Array(arena.alloc_slice_clone(&missing)));
            }
            
            // For larger arrays, use a direct approach
            let mut missing = Vec::with_capacity(items.len() / 2 + 1);
            let mut present_count = 0;
            
            // Check which keys are missing, stop early if we have enough present keys
            for item in items.iter() {
                if let Some(key) = item.as_str() {
                    if has_property(data, key, arena) {
                        present_count += 1;
                        // Early exit if we have enough present keys
                        if present_count >= min_required {
                            return Ok(DataValue::Array(&[]));
                        }
                    } else {
                        // Try to reuse existing string references
                        if let DataValue::String(s) = item {
                            missing.push(DataValue::String(*s));
                        } else {
                            missing.push(DataValue::String(arena.intern_str(key)));
                        }
                    }
                } else {
                    return Err(LogicError::operator_error(
                        "missing_some", 
                        format!("Expected string keys, got {:?}", item)
                    ));
                }
            }
            
            // Return the array of missing keys
            Ok(DataValue::Array(arena.alloc_slice_clone(&missing)))
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