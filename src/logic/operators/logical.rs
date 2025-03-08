//! Logical operators for logic expressions.
//!
//! This module provides implementations for logical operators
//! such as and, or, not, etc.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;

/// Enumeration of logical operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    /// Logical AND
    And,
    /// Logical OR
    Or,
    /// Logical NOT
    Not,
}

/// Evaluates a logical AND operation.
pub fn eval_and<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check that we have at least 1 argument
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "and".to_string(),
            reason: format!("Expected at least 1 argument, got {}", args.len()),
        });
    }
    
    // If there's only one argument, just evaluate and return it
    if args.len() == 1 {
        return evaluate(&args[0], data, arena);
    }
    
    // Evaluate arguments in order, short-circuiting if any is falsy
    for (i, arg) in args.iter().enumerate() {
        let value = evaluate(arg, data, arena)?;
        
        // If this value is falsy and it's not the last argument, return it
        if !value.coerce_to_bool() {
            return Ok(value);
        }
        
        // If this is the last value and it's truthy, return it
        if i == args.len() - 1 {
            return Ok(value);
        }
    }
    
    // This should never happen with the iterator approach, but just in case
    // If all arguments are truthy, return the last one
    evaluate(&args[args.len() - 1], data, arena)
}

/// Evaluates a logical OR operation.
pub fn eval_or<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check that we have at least 1 argument
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "or".to_string(),
            reason: format!("Expected at least 1 argument, got {}", args.len()),
        });
    }
    
    // If there's only one argument, just evaluate and return it
    if args.len() == 1 {
        return evaluate(&args[0], data, arena);
    }
    
    // Evaluate arguments in order, short-circuiting if any is truthy
    for (i, arg) in args.iter().enumerate() {
        let value = evaluate(arg, data, arena)?;
        
        // If this value is truthy and it's not the last argument, return it
        if value.coerce_to_bool() {
            return Ok(value);
        }
        
        // If this is the last value and it's falsy, return it
        if i == args.len() - 1 {
            return Ok(value);
        }
    }
    
    // This should never happen with the iterator approach, but just in case
    // If all arguments are falsy, return the last one
    evaluate(&args[args.len() - 1], data, arena)
}

/// Evaluates a logical NOT operation.
pub fn eval_not<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check that we have exactly 1 argument
    if args.len() != 1 {
        return Err(LogicError::OperatorError {
            operator: "not".to_string(),
            reason: format!("Expected 1 argument, got {}", args.len()),
        });
    }
    
    // Evaluate the argument and negate its boolean value
    let value = evaluate(&args[0], data, arena)?;
    let result = !value.coerce_to_bool();
    
    // Return the negated result
    Ok(arena.bool_value(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;
    
    #[test]
    fn test_and() {
        let arena = DataArena::new();
        let data_json = json!({"a": true, "b": false, "c": 42});
        let data = DataValue::from_json(&data_json, &arena);
        
        // Test and with all truthy values
        let token = parse_str(r#"{"and": [{"var": "a"}, {"var": "c"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_i64(), Some(42));
        
        // Test and with a falsy value
        let token = parse_str(r#"{"and": [{"var": "a"}, {"var": "b"}, {"var": "c"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
        
        // Test and with short-circuit
        let token = parse_str(r#"{"and": [{"var": "b"}, {"var": "nonexistent"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
    }
    
    #[test]
    fn test_or() {
        let arena = DataArena::new();
        let data_json = json!({"a": true, "b": false, "c": 42});
        let data = DataValue::from_json(&data_json, &arena);
        
        // Test or with a truthy value
        let token = parse_str(r#"{"or": [{"var": "b"}, {"var": "a"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test or with all falsy values
        let token = parse_str(r#"{"or": [{"var": "b"}, false, null, 0]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_i64(), Some(0));
        
        // Test or with short-circuit
        let token = parse_str(r#"{"or": [{"var": "a"}, {"var": "nonexistent"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }
    
    #[test]
    fn test_not() {
        let arena = DataArena::new();
        let data_json = json!({"a": true, "b": false, "c": 42});
        let data = DataValue::from_json(&data_json, &arena);
        
        // Test not with a truthy value
        let token = parse_str(r#"{"!": [{"var": "a"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
        
        // Test not with a falsy value
        let token = parse_str(r#"{"!": [{"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test not with a number
        let token = parse_str(r#"{"!": [{"var": "c"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
    }
} 