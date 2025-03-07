//! Arithmetic operators for logic expressions.
//!
//! This module provides implementations for arithmetic operators
//! such as add, subtract, multiply, etc.

use crate::arena::DataArena;
use crate::value::{DataValue, NumberValue, ValueCoercion};
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;

/// Enumeration of arithmetic operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithmeticOp {
    /// Addition (+)
    Add,
    /// Subtraction (-)
    Subtract,
    /// Multiplication (*)
    Multiply,
    /// Division (/)
    Divide,
    /// Modulo (%)
    Modulo,
    /// Minimum value
    Min,
    /// Maximum value
    Max,
}

/// Evaluates an addition operation.
pub fn eval_add<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error("+", format!("Expected at least 1 argument, got {}", args.len())));
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        let value = evaluate(&args[0], data, arena)?;
        
        // Try to coerce to a number if it's a string
        if let Some(_s) = value.as_str() {
            if let Some(num) = value.coerce_to_number() {
                return Ok(DataValue::Number(num));
            }
            
            // If we can't coerce it to a number, return the string
            return Ok(value);
        }
        
        // For non-string values, return as is
        return Ok(value);
    }
    
    // For multiple arguments, add them all together
    let mut result = evaluate(&args[0], data, arena)?;
    
    for item in args.iter().skip(1) {
        let next = evaluate(item, data, arena)?;
        result = add_values(result, next, arena)?;
    }
    
    Ok(result)
}

// Helper function to add two values
#[inline]
fn add_values<'a>(left: DataValue<'a>, right: DataValue<'a>, arena: &'a DataArena) -> Result<DataValue<'a>> {
    // First, try to coerce both values to numbers
    if let (Some(left_num), Some(right_num)) = (left.coerce_to_number(), right.coerce_to_number()) {
        // If both can be coerced to numbers, add them
        match (left_num, right_num) {
            (NumberValue::Integer(l), NumberValue::Integer(r)) => {
                return Ok(DataValue::integer(l + r));
            },
            _ => {
                return Ok(DataValue::float(left_num.as_f64() + right_num.as_f64()));
            }
        }
    }
    
    // If numeric coercion fails, handle string cases
    match (&left, &right) {
        // If both are strings, concatenate them
        (DataValue::String(left_str), DataValue::String(right_str)) => {
            let result = format!("{}{}", left_str, right_str);
            Ok(DataValue::String(arena.alloc_str(&result)))
        },
        
        // String + Non-String = String concatenation
        (DataValue::String(left_str), _) => {
            let mut result = String::with_capacity(left_str.len() + 20);
            result.push_str(left_str);
            right.coerce_append(&mut result);
            Ok(DataValue::String(arena.alloc_str(&result)))
        },
        
        // Non-String + String = String concatenation
        (_, DataValue::String(right_str)) => {
            let mut result = String::with_capacity(right_str.len() + 20);
            left.coerce_append(&mut result);
            result.push_str(right_str);
            Ok(DataValue::String(arena.alloc_str(&result)))
        },
        
        // If we get here, we can't add these values
        _ => {
            Err(LogicError::operator_error("+", format!("Cannot add {} and {}", left.type_name(), right.type_name())))
        }
    }
}

/// Evaluates a subtraction operation.
pub fn eval_subtract<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error("-", format!("Expected at least 1 argument, got {}", args.len())));
    }
    
    // Fast path for single argument (negate)
    if args.len() == 1 {
        let value = evaluate(&args[0], data, arena)?;
        
        // Try to coerce to a number
        if let Some(num) = value.coerce_to_number() {
            match num {
                NumberValue::Integer(i) => return Ok(DataValue::integer(-i)),
                NumberValue::Float(f) => return Ok(DataValue::float(-f)),
            }
        }
        
        return Err(LogicError::operator_error("-", format!("Cannot negate {}", value.type_name())));
    }
    
    // For multiple arguments, subtract them all from the first
    let mut result = evaluate(&args[0], data, arena)?;
    
    for item in args.iter().skip(1) {
        let next = evaluate(item, data, arena)?;
        result = subtract_values(result, next)?;
    }
    
    Ok(result)
}

// Helper function to subtract two values
#[inline]
fn subtract_values<'a>(left: DataValue<'a>, right: DataValue<'a>) -> Result<DataValue<'a>> {
    // Try to coerce both to numbers
    if let (Some(left_num), Some(right_num)) = (left.coerce_to_number(), right.coerce_to_number()) {
        // If both can be coerced to numbers, subtract them
        match (left_num, right_num) {
            (NumberValue::Integer(l), NumberValue::Integer(r)) => {
                Ok(DataValue::integer(l - r))
            },
            _ => {
                Ok(DataValue::float(left_num.as_f64() - right_num.as_f64()))
            }
        }
    } else {
        // If either can't be coerced to a number, return an error
        Err(LogicError::operator_error("-", format!("Cannot subtract {} from {}", right.type_name(), left.type_name())))
    }
}

/// Evaluates a multiplication operation.
pub fn eval_multiply<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error("*", format!("Expected at least 1 argument, got {}", args.len())));
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        let value = evaluate(&args[0], data, arena)?;
        
        // Try to coerce to a number
        if let Some(num) = value.coerce_to_number() {
            return Ok(DataValue::Number(num));
        }
        
        return Err(LogicError::operator_error("*", format!("Cannot use {} as a number", value.type_name())));
    }
    
    // For multiple arguments, multiply them all together
    let mut result = evaluate(&args[0], data, arena)?;
    
    for item in args.iter().skip(1) {
        let next = evaluate(item, data, arena)?;
        result = multiply_values(result, next)?;
    }
    
    Ok(result)
}

// Helper function to multiply two values
#[inline]
fn multiply_values<'a>(left: DataValue<'a>, right: DataValue<'a>) -> Result<DataValue<'a>> {
    // Try to coerce both to numbers
    if let (Some(left_num), Some(right_num)) = (left.coerce_to_number(), right.coerce_to_number()) {
        // If both can be coerced to numbers, multiply them
        match (left_num, right_num) {
            (NumberValue::Integer(l), NumberValue::Integer(r)) => {
                Ok(DataValue::integer(l * r))
            },
            _ => {
                Ok(DataValue::float(left_num.as_f64() * right_num.as_f64()))
            }
        }
    } else {
        // If either can't be coerced to a number, return an error
        Err(LogicError::operator_error("*", format!("Cannot multiply {} and {}", left.type_name(), right.type_name())))
    }
}

/// Evaluates a division operation.
pub fn eval_divide<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error("/", format!("Expected at least 1 argument, got {}", args.len())));
    }
    
    // Fast path for single argument (reciprocal)
    if args.len() == 1 {
        let value = evaluate(&args[0], data, arena)?;
        
        // Try to coerce to a number
        if let Some(num) = value.coerce_to_number() {
            match num {
                NumberValue::Integer(i) => {
                    if i == 0 {
                        return Err(LogicError::operator_error("/", "Division by zero"));
                    }
                    return Ok(DataValue::float(1.0 / i as f64));
                },
                NumberValue::Float(f) => {
                    if f == 0.0 {
                        return Err(LogicError::operator_error("/", "Division by zero"));
                    }
                    return Ok(DataValue::float(1.0 / f));
                }
            }
        }
        
        return Err(LogicError::operator_error("/", format!("Cannot divide 1 by {}", value.type_name())));
    }
    
    // For multiple arguments, divide the first by all the rest
    let mut result = evaluate(&args[0], data, arena)?;
    
    for item in args.iter().skip(1) {
        let next = evaluate(item, data, arena)?;
        result = divide_values(result, next)?;
    }
    
    Ok(result)
}

// Helper function to divide two values
#[inline]
fn divide_values<'a>(left: DataValue<'a>, right: DataValue<'a>) -> Result<DataValue<'a>> {
    // Try to coerce both to numbers
    if let (Some(left_num), Some(right_num)) = (left.coerce_to_number(), right.coerce_to_number()) {
        // Check for division by zero
        match right_num {
            NumberValue::Integer(0) => {
                return Err(LogicError::operator_error("/", "Division by zero"));
            },
            NumberValue::Float(0.0) => {
                return Err(LogicError::operator_error("/", "Division by zero"));
            },
            _ => {}
        }
        
        // Perform the division
        Ok(DataValue::float(left_num.as_f64() / right_num.as_f64()))
    } else {
        // If either can't be coerced to a number, return an error
        Err(LogicError::operator_error("/", format!("Cannot divide {} by {}", left.type_name(), right.type_name())))
    }
}

/// Evaluates a modulo operation.
pub fn eval_modulo<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::operator_error("%", format!("Expected at least 2 arguments, got {}", args.len())));
    }
    
    // Check that we have at least 2 arguments
    if args.len() < 2 {
        return Err(LogicError::operator_error("%", format!("Expected at least 2 arguments, got {}", args.len())));
    }
    
    // For multiple arguments, apply modulo in sequence
    let mut result = evaluate(&args[0], data, arena)?;
    
    for item in args.iter().skip(1) {
        let next = evaluate(item, data, arena)?;
        result = modulo_values(result, next)?;
    }
    
    Ok(result)
}

// Helper function to perform modulo on two values
#[inline]
fn modulo_values<'a>(left: DataValue<'a>, right: DataValue<'a>) -> Result<DataValue<'a>> {
    // Try to coerce both to numbers
    if let (Some(left_num), Some(right_num)) = (left.coerce_to_number(), right.coerce_to_number()) {
        // Check for modulo by zero
        match right_num {
            NumberValue::Integer(0) => {
                return Err(LogicError::operator_error("%", "Modulo by zero"));
            },
            NumberValue::Float(0.0) => {
                return Err(LogicError::operator_error("%", "Modulo by zero"));
            },
            _ => {}
        }
        
        // Perform the modulo operation
        Ok(DataValue::float(left_num.as_f64() % right_num.as_f64()))
    } else {
        // If either can't be coerced to a number, return an error
        Err(LogicError::operator_error("%", format!("Cannot perform modulo on {} and {}", left.type_name(), right.type_name())))
    }
}

/// Evaluates a min operation.
pub fn eval_min<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Check that we have at least 1 argument
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "min".to_string(),
            reason: "Expected at least 1 argument".to_string(),
        });
    }
    
    // Evaluate all arguments and find the minimum
    let mut min_value: Option<NumberValue> = None;
    let mut min_index = 0;
    
    for (i, arg) in args.iter().enumerate() {
        let value = evaluate(arg, data, arena)?;
        
        // Try to convert to number
        if let Some(num) = value.coerce_to_number() {
            if min_value.is_none() || num < min_value.unwrap() {
                min_value = Some(num);
                min_index = i;
            }
        }
    }
    
    // Return the original value at the min index (to preserve type)
    if min_value.is_some() {
        evaluate(&args[min_index], data, arena)
    } else {
        // If no valid numbers, return null
        Ok(DataValue::null())
    }
}

/// Evaluates a max operation.
pub fn eval_max<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Check that we have at least 1 argument
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "max".to_string(),
            reason: "Expected at least 1 argument".to_string(),
        });
    }
    
    // Evaluate all arguments and find the maximum
    let mut max_value: Option<NumberValue> = None;
    let mut max_index = 0;
    
    for (i, arg) in args.iter().enumerate() {
        let value = evaluate(arg, data, arena)?;
        
        // Try to convert to number
        if let Some(num) = value.coerce_to_number() {
            if max_value.is_none() || num > max_value.unwrap() {
                max_value = Some(num);
                max_index = i;
            }
        }
    }
    
    // Return the original value at the max index (to preserve type)
    if max_value.is_some() {
        evaluate(&args[max_index], data, arena)
    } else {
        // If no valid numbers, return null
        Ok(DataValue::null())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;
    
    #[test]
    fn test_add() {
        let arena = DataArena::new();
        let data_json = json!({"a": 10, "b": 20, "c": "hello"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test adding numbers
        let token = parse_str(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(30.0));
        
        // Test adding multiple numbers
        let token = parse_str(r#"{"+": [{"var": "a"}, {"var": "b"}, 5]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(35.0));
        
        // Test adding strings
        let token = parse_str(r#"{"+": [{"var": "c"}, " world"]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("hello world"));
        
        // Test adding number and string
        let token = parse_str(r#"{"+": [{"var": "c"}, {"var": "a"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("hello10"));
    }
    
    #[test]
    fn test_subtract() {
        let arena = DataArena::new();
        let data_json = json!({"a": 30, "b": 10});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test subtracting numbers
        let token = parse_str(r#"{"-": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(20.0));
        
        // Test subtracting multiple numbers
        let token = parse_str(r#"{"-": [{"var": "a"}, {"var": "b"}, 5]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(15.0));
    }
    
    #[test]
    fn test_multiply() {
        let arena = DataArena::new();
        let data_json = json!({"a": 5, "b": 4});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test multiplying numbers
        let token = parse_str(r#"{"*": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(20.0));
        
        // Test multiplying multiple numbers
        let token = parse_str(r#"{"*": [{"var": "a"}, {"var": "b"}, 2]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(40.0));
    }
    
    #[test]
    fn test_divide() {
        let arena = DataArena::new();
        let data_json = json!({"a": 20, "b": 4});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test dividing numbers
        let token = parse_str(r#"{"/": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(5.0));
    }
    
    #[test]
    fn test_modulo() {
        let arena = DataArena::new();
        let data_json = json!({"a": 23, "b": 5});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test modulo
        let token = parse_str(r#"{"%": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(3.0));
    }
} 