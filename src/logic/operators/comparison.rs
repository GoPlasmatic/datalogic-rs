//! Comparison operators for logic expressions.
//!
//! This module provides implementations for comparison operators
//! such as equal, not equal, greater than, etc.

use crate::arena::DataArena;
use crate::value::DataValue;
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
pub fn eval_equal<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], data, arena)?;
        let right = evaluate(args[i + 1], data, arena)?;

        // Fast path for identical references
        if std::ptr::eq(left as *const DataValue, right as *const DataValue) {
            continue;
        }

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
                continue;
            }
            (DataValue::Number(_), DataValue::String(s)) => {
                // Try to parse the string as a number
                if let Ok(num) = s.parse::<f64>() {
                    let left_num = left.coerce_to_number().unwrap();
                    if left_num.as_f64() != num {
                        return Ok(arena.false_value());
                    }
                } else {
                    // String is not a valid number
                    return Err(LogicError::NaNError);
                }
            }
            (DataValue::String(s), DataValue::Number(_)) => {
                // Try to parse the string as a number
                if let Ok(num) = s.parse::<f64>() {
                    let right_num = right.coerce_to_number().unwrap();
                    if num != right_num.as_f64() {
                        return Ok(arena.false_value());
                    }
                } else {
                    // String is not a valid number
                    return Err(LogicError::NaNError);
                }
            }
            (DataValue::Array(_), DataValue::Array(_)) => {
                // Arrays should be compared by reference, not by value
                return Err(LogicError::NaNError);
            }
            (DataValue::Array(_), _) | (_, DataValue::Array(_)) => {
                // Arrays can't be compared with non-arrays
                return Err(LogicError::NaNError);
            }
            (DataValue::Object(_), _) | (_, DataValue::Object(_)) => {
                // Objects can't be compared with anything else
                return Err(LogicError::NaNError);
            }
            _ => {
                // Try numeric coercion for other cases
                if let (Some(a), Some(b)) = (left.coerce_to_number(), right.coerce_to_number()) {
                    if a.as_f64() != b.as_f64() {
                        return Ok(arena.false_value());
                    }
                } else {
                    // If numeric coercion fails, fall back to string comparison
                    let left_str = left.coerce_to_string(arena);
                    let right_str = right.coerce_to_string(arena);
                    
                    if let (DataValue::String(a), DataValue::String(b)) = (&left_str, &right_str) {
                        if a != b {
                            return Ok(arena.false_value());
                        }
                    } else {
                        return Ok(arena.false_value());
                    }
                }
            }
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a strict equality comparison.
pub fn eval_strict_equal<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], data, arena)?;
        let right = evaluate(args[i + 1], data, arena)?;

        if !left.strict_equals(right) {
            return Ok(arena.false_value());
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a not equal comparison.
pub fn eval_not_equal<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], data, arena)?;
        let right = evaluate(args[i + 1], data, arena)?;

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
pub fn eval_strict_not_equal<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], data, arena)?;
        let right = evaluate(args[i + 1], data, arena)?;

        if left.strict_equals(right) {
            return Ok(arena.false_value());
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a greater-than comparison.
pub fn eval_greater_than<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], data, arena)?;
        let right = evaluate(args[i + 1], data, arena)?;

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
pub fn eval_greater_than_or_equal<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], data, arena)?;
        let right = evaluate(args[i + 1], data, arena)?;

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
pub fn eval_less_than<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], data, arena)?;
        let right = evaluate(args[i + 1], data, arena)?;

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
pub fn eval_less_than_or_equal<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], data, arena)?;
        let right = evaluate(args[i + 1], data, arena)?;

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
    fn test_not_equal() {
        let arena = DataArena::new();
        let data_json = json!({"a": 10, "b": "10", "c": 20, "d": 30});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test not equal with two arguments
        let token = parse_str(r#"{"!=": [{"var": "a"}, {"var": "c"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test not equal with same values
        let token = parse_str(r#"{"!=": [{"var": "a"}, 10]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
        
        // Test not equal with multiple arguments (all equal)
        let token = parse_str(r#"{"!=": [10, 10, 10, 10]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(false));
        
        // Test not equal with multiple arguments (one pair not equal)
        let token = parse_str(r#"{"!=": [{"var": "a"}, {"var": "b"}, {"var": "c"}]}"#, &arena).unwrap();
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