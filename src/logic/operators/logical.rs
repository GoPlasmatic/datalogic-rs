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
    /// Logical Double Negation
    DoubleNegation,
}

/// Evaluates an AND operation.
pub fn eval_and<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Ok(arena.true_value());
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        let value = evaluate(args[0], data, arena)?;
        return Ok(arena.bool_value(value.coerce_to_bool()));
    }
    
    // Evaluate each argument with short-circuit evaluation
    for arg in args {
        let value = evaluate(arg, data, arena)?;
        
        // If any argument is false, the result is false
        if !value.coerce_to_bool() {
            return Ok(arena.false_value());
        }
    }
    
    // All arguments are true
    Ok(arena.true_value())
}

/// Evaluates an OR operation.
pub fn eval_or<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Ok(arena.false_value());
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        let value = evaluate(args[0], data, arena)?;
        return Ok(arena.bool_value(value.coerce_to_bool()));
    }
    
    // Evaluate each argument with short-circuit evaluation
    for arg in args {
        let value = evaluate(arg, data, arena)?;
        
        // If any argument is true, the result is true
        if value.coerce_to_bool() {
            return Ok(arena.true_value());
        }
    }
    
    // All arguments are false
    Ok(arena.false_value())
}

/// Evaluates a logical NOT operation.
pub fn eval_not<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 1 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let value = evaluate(args[0], data, arena)?;
    Ok(arena.alloc(DataValue::Bool(!value.coerce_to_bool())))
}

/// Evaluates a logical double negation (!!).
pub fn eval_double_negation<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 1 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let value = evaluate(args[0], data, arena)?;
    Ok(arena.alloc(DataValue::Bool(value.coerce_to_bool())))
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