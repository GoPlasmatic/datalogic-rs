//! Comparison operators for logic expressions.
//!
//! This module provides implementations for comparison operators
//! such as equal, not equal, greater than, etc.

use crate::arena::DataArena;
use crate::value::{DataValue, ValueComparison};
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;

/// Enumeration of comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    /// Equal (==)
    Equal,
    /// Strict equal (===)
    StrictEqual,
    /// Not equal (!=)
    NotEqual,
    /// Strict not equal (!==)
    StrictNotEqual,
    /// Greater than (>)
    GreaterThan,
    /// Greater than or equal (>=)
    GreaterThanOrEqual,
    /// Less than (<)
    LessThan,
    /// Less than or equal (<=)
    LessThanOrEqual,
}

/// Evaluates an equality comparison.
pub fn eval_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    check_args_count("==", args, 2)?;
    
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    Ok(DataValue::bool(left.equals(&right)))
}

/// Evaluates a strict equality comparison.
pub fn eval_strict_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    check_args_count("===", args, 2)?;
    
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    Ok(DataValue::bool(left.strict_equals(&right)))
}

/// Evaluates a not equal comparison.
pub fn eval_not_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    check_args_count("!=", args, 2)?;
    
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    Ok(DataValue::bool(!left.equals(&right)))
}

/// Evaluates a strict not equal comparison.
pub fn eval_strict_not_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    check_args_count("!==", args, 2)?;
    
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    Ok(DataValue::bool(!left.strict_equals(&right)))
}

/// Evaluates a greater than comparison.
pub fn eval_greater_than<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error(">", format!("Expected at least 2 arguments, got {}", args.len())));
    }
    
    // If only one argument, always return false (nothing is greater than itself)
    if args.len() == 1 {
        return Ok(DataValue::bool(false));
    }
    
    // Evaluate all arguments
    let mut prev = evaluate(&args[0], data, arena)?;
    
    // Check each pair of adjacent arguments
    for item in args.iter().skip(1) {
        let current = evaluate(item, data, arena)?;
        
        // If any pair is not in the correct order, return false
        if !prev.greater_than(&current)? {
            return Ok(DataValue::bool(false));
        }
        
        prev = current;
    }
    
    // All pairs are in the correct order
    Ok(DataValue::bool(true))
}

/// Evaluates a greater than or equal comparison.
pub fn eval_greater_than_or_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error(">=", format!("Expected at least 2 arguments, got {}", args.len())));
    }
    
    // If only one argument, always return true (everything is greater than or equal to itself)
    if args.len() == 1 {
        return Ok(DataValue::bool(true));
    }
    
    // Evaluate all arguments
    let mut prev = evaluate(&args[0], data, arena)?;
    
    // Check each pair of adjacent arguments
    for item in args.iter().skip(1) {
        let current = evaluate(item, data, arena)?;
        
        // If any pair is not in the correct order, return false
        if !prev.greater_than_equal(&current)? {
            return Ok(DataValue::bool(false));
        }
        
        prev = current;
    }
    
    // All pairs are in the correct order
    Ok(DataValue::bool(true))
}

/// Evaluates a less than comparison.
pub fn eval_less_than<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error("<", format!("Expected at least 2 arguments, got {}", args.len())));
    }
    
    // If only one argument, always return false (nothing is less than itself)
    if args.len() == 1 {
        return Ok(DataValue::bool(false));
    }
    
    // Evaluate all arguments
    let mut prev = evaluate(&args[0], data, arena)?;
    
    // Check each pair of adjacent arguments
    for item in args.iter().skip(1) {
        let current = evaluate(item, data, arena)?;
        
        // If any pair is not in the correct order, return false
        if !prev.less_than(&current)? {
            return Ok(DataValue::bool(false));
        }
        
        prev = current;
    }
    
    // All pairs are in the correct order
    Ok(DataValue::bool(true))
}

/// Evaluates a less than or equal comparison.
pub fn eval_less_than_or_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error("<=", format!("Expected at least 2 arguments, got {}", args.len())));
    }
    
    // If only one argument, always return true (everything is less than or equal to itself)
    if args.len() == 1 {
        return Ok(DataValue::bool(true));
    }
    
    // Evaluate all arguments
    let mut prev = evaluate(&args[0], data, arena)?;
    
    // Check each pair of adjacent arguments
    for item in args.iter().skip(1) {
        let current = evaluate(item, data, arena)?;
        
        // If any pair is not in the correct order, return false
        if !prev.less_than_equal(&current)? {
            return Ok(DataValue::bool(false));
        }
        
        prev = current;
    }
    
    // All pairs are in the correct order
    Ok(DataValue::bool(true))
}

/// Checks that the number of arguments is correct.
fn check_args_count(op: &str, args: &[Token], expected: usize) -> Result<()> {
    if args.len() != expected {
        return Err(LogicError::operator_error(op, format!("Expected {} arguments, got {}", expected, args.len())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;
    
    #[test]
    fn test_equal() {
        let arena = DataArena::new();
        let data_json = json!({"a": 10, "b": "10", "c": 20});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test equal with same type
        let token = parse_str(r#"{"==": [{"var": "a"}, 10]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test equal with different types (number and string)
        let token = parse_str(r#"{"==": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test not equal
        let token = parse_str(r#"{"==": [{"var": "a"}, {"var": "c"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
    }
    
    #[test]
    fn test_strict_equal() {
        let arena = DataArena::new();
        let data_json = json!({"a": 10, "b": "10", "c": 20});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test strict equal with same type
        let token = parse_str(r#"{"===": [{"var": "a"}, 10]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test strict equal with different types (number and string)
        let token = parse_str(r#"{"===": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
    }
    
    #[test]
    fn test_greater_than() {
        let arena = DataArena::new();
        let data_json = json!({"a": 10, "b": 5, "c": "20"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test greater than with numbers
        let token = parse_str(r#"{">": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test greater than with string coercion
        let token = parse_str(r#"{">": [{"var": "c"}, {"var": "a"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }
    
    #[test]
    fn test_less_than() {
        let arena = DataArena::new();
        let data_json = json!({"a": 10, "b": 5, "c": "20"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test less than with numbers
        let token = parse_str(r#"{"<": [{"var": "b"}, {"var": "a"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test less than with string coercion
        let token = parse_str(r#"{"<": [{"var": "a"}, {"var": "c"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }
} 