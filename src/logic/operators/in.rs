//! In operator implementation.
//!
//! This module provides the implementation of the in operator.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Evaluates an in operation.
pub fn eval_in<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check that we have exactly 2 arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "in".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the arguments
    let needle = evaluate(&args[0], data, arena)?;
    let haystack = evaluate(&args[1], data, arena)?;
    
    // Check if the needle is in the haystack
    match haystack {
        // Check if the needle is in the array
        DataValue::Array(items) => {
            for item in items.iter() {
                if item == needle {
                    return Ok(arena.true_value());
                }
            }
            Ok(arena.false_value())
        },
        
        // Check if the needle is in the string
        DataValue::String(s) => {
            let needle_str = match needle {
                DataValue::String(s) => s,
                _ => return Err(LogicError::OperatorError {
                    operator: "in".to_string(),
                    reason: format!("Expected string needle for string haystack, got {:?}", needle),
                }),
            };
            
            Ok(arena.bool_value(s.contains(needle_str)))
        },
        
        // Check if the needle is a key in the object
        DataValue::Object(entries) => {
            let needle_str = match needle {
                DataValue::String(s) => s,
                _ => return Err(LogicError::OperatorError {
                    operator: "in".to_string(),
                    reason: format!("Expected string needle for object haystack, got {:?}", needle),
                }),
            };
            
            for (key, _) in entries.iter() {
                if key == needle_str {
                    return Ok(arena.true_value());
                }
            }
            Ok(arena.false_value())
        },
        
        // Other types are not supported
        _ => Err(LogicError::OperatorError {
            operator: "in".to_string(),
            reason: format!("Expected array, string, or object haystack, got {:?}", haystack),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_evaluate_in_array() {
        let arena = DataArena::new();
        let data_json = json!({
            "array": [1, 2, 3, 4, 5],
            "value": 3
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Parse and evaluate an in expression with array
        let token = parse_str(r#"{"in": [{"var": "value"}, {"var": "array"}]}"#, &arena).unwrap();
        
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn test_evaluate_in_string() {
        let arena = DataArena::new();
        let data_json = json!({
            "string": "hello world",
            "substring": "world"
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Parse and evaluate an in expression with string
        let token = parse_str(r#"{"in": [{"var": "substring"}, {"var": "string"}]}"#, &arena).unwrap();
        
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn test_evaluate_in_object() {
        let arena = DataArena::new();
        let data_json = json!({
            "object": {"a": 1, "b": 2, "c": 3},
            "key": "b"
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Parse and evaluate an in expression with object
        let token = parse_str(r#"{"in": [{"var": "key"}, {"var": "object"}]}"#, &arena).unwrap();
        
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }
} 