//! Comparison operators for logic expressions.
//!
//! This module provides implementations for comparison operators
//! such as equal, not equal, greater than, etc.

use std::cmp::Ordering;
use crate::arena::DataArena;
use crate::value::DataValue;
use crate::value::NumberValue;
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
) -> Result<&'a DataValue<'a>> {
    check_args_count("==", args, 2)?;
    
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    Ok(arena.bool_value(left.equals(right)))
}

/// Evaluates a strict equality comparison.
pub fn eval_strict_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    check_args_count("===", args, 2)?;
    
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    Ok(arena.bool_value(left.strict_equals(right)))
}

/// Evaluates a not-equal comparison.
pub fn eval_not_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    check_args_count("!=", args, 2)?;
    
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    Ok(arena.bool_value(!left.equals(right)))
}

/// Evaluates a strict not-equal comparison.
pub fn eval_strict_not_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    check_args_count("!==", args, 2)?;
    
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    Ok(arena.bool_value(!left.strict_equals(right)))
}

/// Evaluates a greater-than comparison.
pub fn eval_greater_than<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    check_args_count(">", args, 2)?;
    
    // If we have more than 2 arguments, check if each pair is in strictly descending order
    if args.len() > 2 {
        let mut prev = evaluate(&args[0], data, arena)?;
        
        for i in 1..args.len() {
            let current = evaluate(&args[i], data, arena)?;
            
            // Check if prev > current
            match compare_with_coercion(prev, current) {
                Some(Ordering::Greater) => {
                    // Continue to next pair
                    prev = current;
                },
                _ => {
                    // Not strictly greater than, return false
                    return Ok(arena.false_value());
                }
            }
        }
        
        // All pairs are in strictly descending order
        return Ok(arena.true_value());
    }
    
    // Simple case with just 2 arguments
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    // Get the ordering between the values
    match compare_with_coercion(left, right) {
        Some(Ordering::Greater) => Ok(arena.true_value()),
        _ => Ok(arena.false_value()),
    }
}

/// Evaluates a greater-than-or-equal comparison.
pub fn eval_greater_than_or_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    check_args_count(">=", args, 2)?;
    
    // If we have more than 2 arguments, check if each pair is in non-ascending order
    if args.len() > 2 {
        let mut prev = evaluate(&args[0], data, arena)?;
        
        for i in 1..args.len() {
            let current = evaluate(&args[i], data, arena)?;
            
            // Check if prev >= current
            match compare_with_coercion(prev, current) {
                Some(Ordering::Greater) | Some(Ordering::Equal) => {
                    // Continue to next pair
                    prev = current;
                },
                _ => {
                    // Not greater than or equal, return false
                    return Ok(arena.false_value());
                }
            }
        }
        
        // All pairs are in non-ascending order
        return Ok(arena.true_value());
    }
    
    // Simple case with just 2 arguments
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    // Get the ordering between the values
    match compare_with_coercion(left, right) {
        Some(Ordering::Greater) | Some(Ordering::Equal) => Ok(arena.true_value()),
        _ => Ok(arena.false_value()),
    }
}

/// Evaluates a less-than comparison.
pub fn eval_less_than<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    check_args_count("<", args, 2)?;
    
    // If we have more than 2 arguments, check if each pair is in strictly ascending order
    if args.len() > 2 {
        let mut prev = evaluate(&args[0], data, arena)?;
        
        for i in 1..args.len() {
            let current = evaluate(&args[i], data, arena)?;
            
            // Check if prev < current
            match compare_with_coercion(prev, current) {
                Some(Ordering::Less) => {
                    // Continue to next pair
                    prev = current;
                },
                _ => {
                    // Not strictly less than, return false
                    return Ok(arena.false_value());
                }
            }
        }
        
        // All pairs are in strictly ascending order
        return Ok(arena.true_value());
    }
    
    // Simple case with just 2 arguments
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    // Get the ordering between the values
    match compare_with_coercion(left, right) {
        Some(Ordering::Less) => Ok(arena.true_value()),
        _ => Ok(arena.false_value()),
    }
}

/// Evaluates a less-than-or-equal comparison.
pub fn eval_less_than_or_equal<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    check_args_count("<=", args, 2)?;
    
    // If we have more than 2 arguments, check if each pair is in non-descending order
    if args.len() > 2 {
        let mut prev = evaluate(&args[0], data, arena)?;
        
        for i in 1..args.len() {
            let current = evaluate(&args[i], data, arena)?;
            
            // Check if prev <= current
            match compare_with_coercion(prev, current) {
                Some(Ordering::Less) | Some(Ordering::Equal) => {
                    // Continue to next pair
                    prev = current;
                },
                _ => {
                    // Not less than or equal, return false
                    return Ok(arena.false_value());
                }
            }
        }
        
        // All pairs are in non-descending order
        return Ok(arena.true_value());
    }
    
    // Simple case with just 2 arguments
    let left = evaluate(&args[0], data, arena)?;
    let right = evaluate(&args[1], data, arena)?;
    
    // Get the ordering between the values
    match compare_with_coercion(left, right) {
        Some(Ordering::Less) | Some(Ordering::Equal) => Ok(arena.true_value()),
        _ => Ok(arena.false_value()),
    }
}

/// Checks that the number of arguments is as expected.
fn check_args_count(op: &str, args: &[Token], expected: usize) -> Result<()> {
    if args.len() < expected {
        return Err(LogicError::OperatorError {
            operator: op.to_string(),
            reason: format!("Expected at least {} arguments, got {}", expected, args.len()),
        });
    }
    Ok(())
}

/// Helper function to compare values with type coercion
#[inline]
fn compare_with_coercion<'a>(left: &'a DataValue<'a>, right: &'a DataValue<'a>) -> Option<Ordering> {
    // First try direct comparison
    if let Some(ordering) = left.partial_cmp(right) {
        return Some(ordering);
    }
    
    // Handle mixed types
    match (left, right) {
        (DataValue::Number(a), DataValue::String(b)) => {
            if let Ok(b_num) = b.parse::<f64>() {
                let a_f64 = match a {
                    NumberValue::Integer(i) => *i as f64,
                    NumberValue::Float(f) => *f,
                };
                
                if a_f64 > b_num {
                    return Some(Ordering::Greater);
                } else if a_f64 < b_num {
                    return Some(Ordering::Less);
                } else {
                    return Some(Ordering::Equal);
                }
            }
        },
        (DataValue::String(a), DataValue::Number(b)) => {
            if let Ok(a_num) = a.parse::<f64>() {
                let b_f64 = match b {
                    NumberValue::Integer(i) => *i as f64,
                    NumberValue::Float(f) => *f,
                };
                
                if a_num > b_f64 {
                    return Some(Ordering::Greater);
                } else if a_num < b_f64 {
                    return Some(Ordering::Less);
                } else {
                    return Some(Ordering::Equal);
                }
            }
        },
        _ => {}
    }
    
    None
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