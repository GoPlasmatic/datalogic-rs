//! Missing operators for logic expressions.
//!
//! This module provides implementations for missing operators
//! such as missing and missing_some.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::operators::variable;

/// Evaluates a missing operation.
/// Checks whether the specified variables are missing from the data.
pub fn eval_missing<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(DataValue::Array(&[])));
    }

    let mut missing = arena.get_data_value_vec();

    for arg in args {
        let value = evaluate(arg, data, arena)?;
        
        if let DataValue::String(name) = value {
            // Create a variable token with this name
            let none_ref: Option<&Token> = None;
            if let Ok(var_value) = variable::evaluate_variable(name, &none_ref, data, arena) {
                // If the variable exists, continue to the next one
                if var_value != arena.null_value() {
                    continue;
                }
            }
            
            // If we get here, the variable is missing
            missing.push(DataValue::String(*name));
        } else if let DataValue::Array(names) = value {
            // Check each name in the array
            for name_value in *names {
                if let DataValue::String(name) = name_value {
                    // Check if the variable exists
                    let none_ref: Option<&Token> = None;
                    if let Ok(var_value) = variable::evaluate_variable(name, &none_ref, data, arena) {
                        if var_value != arena.null_value() {
                            continue;
                        }
                    }
                    
                    // Variable is missing
                    missing.push(DataValue::String(*name));
                }
                // Ignore non-string names
            }
        }
        // Ignore non-string, non-array values
    }

    let result = DataValue::Array(arena.alloc_slice_clone(&missing));
    arena.release_data_value_vec(missing);
    
    Ok(arena.alloc(result))
}

/// Evaluates a missing_some operation.
/// Checks whether at least the specified number of variables are present in the data.
pub fn eval_missing_some<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument (minimum number of required fields)
    let min_required = evaluate(args[0], data, arena)?;
    let min_count = min_required.coerce_to_number()
        .map(|n| n.as_i64().unwrap_or(0))
        .unwrap_or(0) as usize;

    // Evaluate the second argument (array of field names)
    let fields = evaluate(args[1], data, arena)?;
    
    if let DataValue::Array(names) = fields {
        // Count how many fields are present
        let mut found_count = 0;
        let mut missing = arena.get_data_value_vec();
        
        for name_value in *names {
            if let DataValue::String(name) = name_value {
                // Check if the variable exists
                let none_ref: Option<&Token> = None;
                if let Ok(var_value) = variable::evaluate_variable(name, &none_ref, data, arena) {
                    if var_value != arena.null_value() {
                        found_count += 1;
                        continue;
                    }
                }
                
                // Variable is missing
                missing.push(DataValue::String(*name));
            }
            // Ignore non-string names
        }
        
        // If we have enough fields, return an empty array
        if found_count >= min_count {
            arena.release_data_value_vec(missing);
            return Ok(arena.alloc(DataValue::Array(&[])));
        }
        
        // Otherwise return the missing fields
        let result = DataValue::Array(arena.alloc_slice_clone(&missing));
        arena.release_data_value_vec(missing);
        
        Ok(arena.alloc(result))
    } else {
        // If the second argument is not an array, return an empty array
        Ok(arena.alloc(DataValue::Array(&[])))
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