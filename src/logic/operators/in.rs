//! In operator implementation.
//!
//! This module provides the implementation of the in operator.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Evaluates an "in" operation.
pub fn eval_in<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "in".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }

    let needle = evaluate(args[0], data, arena)?;
    let haystack = evaluate(args[1], data, arena)?;

    let result = match haystack {
        DataValue::String(s) => {
            let needle_str = match needle {
                DataValue::String(ns) => *ns,
                _ => arena.alloc_str(&needle.to_string()),
            };
            s.contains(needle_str)
        }
        DataValue::Array(arr) => {
            arr.iter().any(|item| {
                match (item, needle) {
                    (DataValue::Number(a), DataValue::Number(b)) => a == b,
                    (DataValue::String(a), DataValue::String(b)) => a == b,
                    (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
                    _ => false,
                }
            })
        }
        DataValue::Object(obj) => {
            // For objects, check if needle is a key in the object
            if let DataValue::String(key) = needle {
                obj.iter().any(|(k, _)| *k == *key)
            } else {
                // If needle is not a string, convert it to a string and check
                let key_str = needle.to_string();
                obj.iter().any(|(k, _)| *k == key_str)
            }
        }
        _ => false,
    };

    if result {
        Ok(arena.true_value())
    } else {
        Ok(arena.false_value())
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