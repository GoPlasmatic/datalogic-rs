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
    use crate::JsonLogic;
    use serde_json::json;
    
    #[test]
    fn test_equal() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({"a": 10, "b": "10", "c": 20});
        
        // Test equal with same type
        let rule = builder.compare()
            .equalOp()
            .var("a")
            .right(builder.int(10));
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
        
        // Test equal with different types (number and string)
        let rule = builder.compare()
            .equalOp()
            .var("a")
            .var_right("b");
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
        
        // Test not equal
        let rule = builder.compare()
            .equalOp()
            .var("a")
            .var_right("c");
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }
    
    #[test]
    fn test_not_equal() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({"a": 10, "b": "10", "c": 20, "d": 30});
        
        // Test not equal with two arguments
        let rule = builder.compare()
            .notEqualOp()
            .var("a")
            .var_right("c");
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
        
        // Test not equal with same values
        let rule = builder.compare()
            .notEqualOp()
            .var("a")
            .right(builder.int(10));
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
        
        // Test not equal with multiple arguments (chain comparison)
        // For multiple arguments, we need to chain comparisons with AND
        let comparison1 = builder.compare().notEqualOp()
            .left(builder.int(10))
            .right(builder.int(10));
            
        let comparison2 = builder.compare().notEqualOp()
            .left(builder.int(10))
            .right(builder.int(10));
            
        let comparison3 = builder.compare().notEqualOp()
            .left(builder.int(10))
            .right(builder.int(10));
        
        let rule = builder.control().andOp()
            .operand(comparison1)
            .operand(comparison2)
            .operand(comparison3)
            .build();
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
        
        // Test not equal with different values in a chain
        let comparison1 = builder.compare().notEqualOp()
            .var("a")
            .var_right("b");
            
        let comparison2 = builder.compare().notEqualOp()
            .var("b")
            .var_right("c");
        
        let rule = builder.control().andOp()
            .operand(comparison1)
            .operand(comparison2)
            .build();
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }
    
    #[test]
    fn test_strict_equal() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({"a": 10, "b": "10", "c": 20});
        
        // Test strict equal with same type
        let rule = builder.compare()
            .strictEqualOp()
            .var("a")
            .right(builder.int(10));
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
        
        // Test strict equal with different types (number and string)
        let rule = builder.compare()
            .strictEqualOp()
            .var("a")
            .var_right("b");
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }
    
    #[test]
    fn test_greater_than() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({"a": 10, "b": 5, "c": "20"});
        
        // Test greater than with numbers
        let rule = builder.compare()
            .greaterThanOp()
            .var("a")
            .var_right("b");
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
        
        // Test greater than with string coercion
        let rule = builder.compare()
            .greaterThanOp()
            .var("c")
            .var_right("a");
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
    }
    
    #[test]
    fn test_less_than() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({"a": 10, "b": 5, "c": "20"});
        
        // Test less than with numbers
        let rule = builder.compare()
            .lessThanOp()
            .var("b")
            .var_right("a");
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
        
        // Test less than with string coercion
        let rule = builder.compare()
            .lessThanOp()
            .var("a")
            .var_right("c");
            
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
    }
} 