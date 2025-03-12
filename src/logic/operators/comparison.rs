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
pub fn eval_equal<'a>(args: &'a [Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(&args[i], data, arena)?;
        let right = evaluate(&args[i + 1], data, arena)?;

        match (left, right) {
            (DataValue::Number(a), DataValue::Number(b)) => {
                if a.as_f64() != b.as_f64() {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::String(a), DataValue::String(b)) => {
                if a != b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Bool(a), DataValue::Bool(b)) => {
                if a != b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Null, DataValue::Null) => {
                return Ok(arena.true_value());
            }
            _ => {
                let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
                let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
                if left_num.as_f64() != right_num.as_f64() {
                    return Ok(arena.false_value());
                }
            }
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a strict equality comparison.
pub fn eval_strict_equal<'a>(args: &'a [Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(&args[i], data, arena)?;
        let right = evaluate(&args[i + 1], data, arena)?;

        if !left.strict_equals(right) {
            return Ok(arena.false_value());
        }
    }
    Ok(arena.true_value())
}

/// Evaluates a not-equal comparison.
pub fn eval_not_equal<'a>(args: &'a [Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(&args[i], data, arena)?;
        let right = evaluate(&args[i + 1], data, arena)?;

        match (left, right) {
            (DataValue::Number(a), DataValue::Number(b)) => {
                if a.as_f64() == b.as_f64() {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::String(a), DataValue::String(b)) => {
                if a == b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Bool(a), DataValue::Bool(b)) => {
                if a == b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Null, DataValue::Null) => {
                return Ok(arena.false_value());
            }
            _ => {
                let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
                let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
                if left_num.as_f64() == right_num.as_f64() {
                    return Ok(arena.false_value());
                }
            }
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a strict not-equal comparison.
pub fn eval_strict_not_equal<'a>(args: &'a [Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(&args[i], data, arena)?;
        let right = evaluate(&args[i + 1], data, arena)?;

        if left.strict_equals(right) {
            return Ok(arena.false_value());
        }
    }
    Ok(arena.true_value())
}

/// Evaluates a greater-than comparison.
pub fn eval_greater_than<'a>(args: &'a [Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    for i in 0..args.len() - 1 {
        let left = evaluate(&args[i], data, arena)?;
        let right = evaluate(&args[i + 1], data, arena)?;

        match (left, right) {
            (DataValue::Number(a), DataValue::Number(b)) => {
                if a.as_f64() <= b.as_f64() {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::String(a), DataValue::String(b)) => {
                if a <= b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Bool(a), DataValue::Bool(b)) => {
                if a <= b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Null, DataValue::Null) => {
                return Ok(arena.false_value());
            }
            _ => {
                let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
                let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
                if left_num.as_f64() <= right_num.as_f64() {
                    return Ok(arena.false_value());
                }
            }
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a greater-than-or-equal comparison.
pub fn eval_greater_than_or_equal<'a>(args: &'a [Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    for i in 0..args.len() - 1 {
        let left = evaluate(&args[i], data, arena)?;
        let right = evaluate(&args[i + 1], data, arena)?;

        match (left, right) {
            (DataValue::Number(a), DataValue::Number(b)) => {
                if a.as_f64() < b.as_f64() {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::String(a), DataValue::String(b)) => {
                if a < b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Bool(a), DataValue::Bool(b)) => {
                if a < b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Null, DataValue::Null) => {
                return Ok(arena.false_value());
            }
            _ => {
                let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
                let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
                if left_num.as_f64() < right_num.as_f64() {
                    return Ok(arena.false_value());
                }
            }
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a less-than comparison.
pub fn eval_less_than<'a>(args: &'a [Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    for i in 0..args.len() - 1 {
        let left = evaluate(&args[i], data, arena)?;
        let right = evaluate(&args[i + 1], data, arena)?;

        match (left, right) {
            (DataValue::Number(a), DataValue::Number(b)) => {
                if a.as_f64() >= b.as_f64() {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::String(a), DataValue::String(b)) => {
                if a >= b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Bool(a), DataValue::Bool(b)) => {
                if a >= b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Null, DataValue::Null) => {
                return Ok(arena.false_value());
            }
            _ => {
                let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
                let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
                if left_num.as_f64() >= right_num.as_f64() {
                    return Ok(arena.false_value());
                }
            }
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a less-than-or-equal comparison.
pub fn eval_less_than_or_equal<'a>(args: &'a [Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    for i in 0..args.len() - 1 {
        let left = evaluate(&args[i], data, arena)?;
        let right = evaluate(&args[i + 1], data, arena)?;

        match (left, right) {
            (DataValue::Number(a), DataValue::Number(b)) => {
                if a.as_f64() > b.as_f64() {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::String(a), DataValue::String(b)) => {
                if a > b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Bool(a), DataValue::Bool(b)) => {
                if a > b {
                    return Ok(arena.false_value());
                }
            }
            (DataValue::Null, DataValue::Null) => {
                return Ok(arena.false_value());
            }
            _ => {
                let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
                let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
                if left_num.as_f64() > right_num.as_f64() {
                    return Ok(arena.false_value());
                }
            }
        }
    }

    Ok(arena.true_value())
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