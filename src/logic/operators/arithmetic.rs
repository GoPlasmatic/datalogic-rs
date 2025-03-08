//! Arithmetic operators for logic expressions.
//!
//! This module provides implementations for arithmetic operators
//! such as add, subtract, multiply, etc.

use crate::arena::DataArena;
use crate::value::{DataValue, NumberValue};
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
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "+".to_string(),
            reason: format!("Expected at least 1 argument, got {}", args.len()),
        });
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        let value = evaluate(&args[0], data, arena)?;
        
        // Try to coerce to a number if it's a string
        if let DataValue::String(_) = value {
            if let Some(num) = value.coerce_to_number() {
                return Ok(arena.alloc(DataValue::Number(num)));
            }
            
            // If we can't coerce it to a number, return the string
            return Ok(value);
        }
        
        // For non-string values, return as is
        return Ok(value);
    }
    
    // For multiple arguments, add them all together
    let mut result = evaluate(&args[0], data, arena)?;
    
    for arg in args.iter().skip(1) {
        let current = evaluate(arg, data, arena)?;
        let addition_result = add_values(result, current, arena)?;
        result = addition_result;
    }
    
    Ok(result)
}

/// Adds two values together.
fn add_values<'a>(left: &'a DataValue<'a>, right: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // First try to coerce both values to numbers
    if let (Some(ln), Some(rn)) = (left.coerce_to_number(), right.coerce_to_number()) {
        // If both can be coerced to numbers, add them
        match (ln, rn) {
            (NumberValue::Integer(li), NumberValue::Integer(ri)) => {
                // Check for overflow
                match li.checked_add(ri) {
                    Some(result) => return Ok(arena.alloc(DataValue::integer(result))),
                    None => return Ok(arena.alloc(DataValue::float(li as f64 + ri as f64))),
                }
            },
            _ => {
                println!("Mixed types: {:?} and {:?}", ln, rn);
                // Use floating point operation for mixed or float types
                let lf = ln.as_f64();
                let rf = rn.as_f64();
                return Ok(arena.alloc(DataValue::float(lf + rf)));
            }
        }
    }
    
    // If numeric coercion fails, handle string cases
    match (left, right) {
        // String concatenation
        (DataValue::String(ls), DataValue::String(rs)) => {
            let result = format!("{}{}", ls, rs);
            Ok(arena.alloc(DataValue::string(arena, &result)))
        },
        
        // String + non-string: convert non-string to string and concatenate
        (DataValue::String(ls), _) => {
            let rs = right.to_string();
            let result = format!("{}{}", ls, rs);
            Ok(arena.alloc(DataValue::string(arena, &result)))
        },
        (_, DataValue::String(rs)) => {
            let ls = left.to_string();
            let result = format!("{}{}", ls, rs);
            Ok(arena.alloc(DataValue::string(arena, &result)))
        },
        
        // This should never happen since we already handled numeric coercion
        _ => {
            Err(LogicError::OperatorError {
                operator: "+".to_string(),
                reason: format!("Cannot add {:?} and {:?}", left, right),
            })
        }
    }
}

/// Evaluates a subtraction operation.
pub fn eval_subtract<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "-".to_string(),
            reason: format!("Expected at least 1 argument, got {}", args.len()),
        });
    }
    
    // Fast path for single argument (negation)
    if args.len() == 1 {
        let value = evaluate(&args[0], data, arena)?;
        
        if let Some(num) = value.coerce_to_number() {
            match num {
                NumberValue::Integer(i) => return Ok(arena.alloc(DataValue::integer(-i))),
                NumberValue::Float(f) => return Ok(arena.alloc(DataValue::float(-f))),
            }
        }
        
        return Err(LogicError::OperatorError {
            operator: "-".to_string(),
            reason: format!("Cannot negate {:?}", value),
        });
    }
    
    // For multiple arguments, subtract all from the first
    let first = evaluate(&args[0], data, arena)?;
    
    if args.len() == 2 {
        let second = evaluate(&args[1], data, arena)?;
        return subtract_values(first, second, arena);
    }
    
    let mut result = first;
    
    for arg in args.iter().skip(1) {
        let current = evaluate(arg, data, arena)?;
        let subtraction_result = subtract_values(result, current, arena)?;
        result = subtraction_result;
    }
    
    Ok(result)
}

/// Subtracts the right value from the left value.
fn subtract_values<'a>(left: &'a DataValue<'a>, right: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    let ln = left.coerce_to_number().ok_or_else(|| LogicError::OperatorError {
        operator: "-".to_string(),
        reason: format!("Cannot coerce {:?} to a number", left),
    })?;
    
    let rn = right.coerce_to_number().ok_or_else(|| LogicError::OperatorError {
        operator: "-".to_string(),
        reason: format!("Cannot coerce {:?} to a number", right),
    })?;
    
    match (ln, rn) {
        (NumberValue::Integer(li), NumberValue::Integer(ri)) => {
            // Check for overflow
            match li.checked_sub(ri) {
                Some(result) => Ok(arena.alloc(DataValue::integer(result))),
                None => Ok(arena.alloc(DataValue::float(li as f64 - ri as f64))),
            }
        },
        _ => {
            // Use floating point operation for mixed or float types
            let lf = ln.as_f64();
            let rf = rn.as_f64();
            Ok(arena.alloc(DataValue::float(lf - rf)))
        }
    }
}

/// Evaluates a multiplication operation.
pub fn eval_multiply<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "*".to_string(),
            reason: format!("Expected at least 1 argument, got {}", args.len()),
        });
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        let value = evaluate(&args[0], data, arena)?;
        return Ok(value);
    }
    
    // For multiple arguments, multiply them all together
    let mut result = evaluate(&args[0], data, arena)?;
    
    for arg in args.iter().skip(1) {
        let current = evaluate(arg, data, arena)?;
        let multiplication_result = multiply_values(result, current, arena)?;
        result = multiplication_result;
    }
    
    Ok(result)
}

/// Multiplies two values together.
fn multiply_values<'a>(left: &'a DataValue<'a>, right: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    let ln = left.coerce_to_number().ok_or_else(|| LogicError::OperatorError {
        operator: "*".to_string(),
        reason: format!("Cannot coerce {:?} to a number", left),
    })?;
    
    let rn = right.coerce_to_number().ok_or_else(|| LogicError::OperatorError {
        operator: "*".to_string(),
        reason: format!("Cannot coerce {:?} to a number", right),
    })?;
    
    match (ln, rn) {
        (NumberValue::Integer(li), NumberValue::Integer(ri)) => {
            // Check for overflow
            match li.checked_mul(ri) {
                Some(result) => Ok(arena.alloc(DataValue::integer(result))),
                None => Ok(arena.alloc(DataValue::float(li as f64 * ri as f64))),
            }
        },
        _ => {
            // Use floating point operation for mixed or float types
            let lf = ln.as_f64();
            let rf = rn.as_f64();
            Ok(arena.alloc(DataValue::float(lf * rf)))
        }
    }
}

/// Evaluates a division operation.
pub fn eval_divide<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "/".to_string(),
            reason: format!("Expected at least 1 argument, got {}", args.len()),
        });
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        return Err(LogicError::OperatorError {
            operator: "/".to_string(),
            reason: "Cannot divide a single number".to_string(),
        });
    }
    
    // For multiple arguments, divide the first by all others
    let first = evaluate(&args[0], data, arena)?;
    
    if args.len() == 2 {
        let second = evaluate(&args[1], data, arena)?;
        return divide_values(first, second, arena);
    }
    
    let mut result = first;
    
    for arg in args.iter().skip(1) {
        let current = evaluate(arg, data, arena)?;
        let division_result = divide_values(result, current, arena)?;
        result = division_result;
    }
    
    Ok(result)
}

/// Divides the left value by the right value.
fn divide_values<'a>(left: &'a DataValue<'a>, right: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    let ln = left.coerce_to_number().ok_or_else(|| LogicError::OperatorError {
        operator: "/".to_string(),
        reason: format!("Cannot coerce {:?} to a number", left),
    })?;
    
    let rn = right.coerce_to_number().ok_or_else(|| LogicError::OperatorError {
        operator: "/".to_string(),
        reason: format!("Cannot coerce {:?} to a number", right),
    })?;
    
    match rn {
        NumberValue::Integer(0) => {
            return Err(LogicError::OperatorError {
                operator: "/".to_string(),
                reason: "Division by zero".to_string(),
            });
        }
        NumberValue::Float(r) if r == 0.0 => {
            return Err(LogicError::OperatorError {
                operator: "/".to_string(),
                reason: "Division by zero".to_string(),
            });
        }
        _ => {}
    }
    
    // Always use floating point for division to handle fractions
    let lf = ln.as_f64();
    let rf = rn.as_f64();
    Ok(arena.alloc(DataValue::float(lf / rf)))
}

/// Evaluates a modulo operation.
pub fn eval_modulo<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check arguments
    if args.len() < 2 {
        return Err(LogicError::OperatorError {
            operator: "%".to_string(),
            reason: format!("Expected at least 2 arguments, got {}", args.len()),
        });
    }
    
    // Get the first value
    let mut result = evaluate(&args[0], data, arena)?;
    
    // Apply modulo with each subsequent value
    for arg in args.iter().skip(1) {
        let right = evaluate(arg, data, arena)?;
        
        // Get the numeric values
        let left_num = result.coerce_to_number().ok_or_else(|| LogicError::OperatorError {
            operator: "%".to_string(),
            reason: format!("Cannot coerce {:?} to a number", result),
        })?;
        
        let right_num = right.coerce_to_number().ok_or_else(|| LogicError::OperatorError {
            operator: "%".to_string(),
            reason: format!("Cannot coerce {:?} to a number", right),
        })?;
        
        // Check for modulo by zero
        match right_num {
            NumberValue::Integer(0) => {
                return Err(LogicError::OperatorError {
                    operator: "%".to_string(),
                    reason: "Modulo by zero".to_string(),
                });
            }
            NumberValue::Float(f) if f == 0.0 => {
                return Err(LogicError::OperatorError {
                    operator: "%".to_string(),
                    reason: "Modulo by zero".to_string(),
                });
            }
            _ => {}
        }
        
        // Compute the modulo
        let new_value = match (left_num, right_num) {
            (NumberValue::Integer(l), NumberValue::Integer(r)) => {
                DataValue::integer(l % r)
            },
            _ => {
                let lf = left_num.as_f64();
                let rf = right_num.as_f64();
                DataValue::float(lf % rf)
            }
        };
        
        // Store the result in the arena
        result = arena.alloc(new_value);
    }
    
    Ok(result)
}

/// Evaluates a minimum operation.
pub fn eval_min<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "min".to_string(),
            reason: format!("Expected at least 1 argument, got {}", args.len()),
        });
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        return evaluate(&args[0], data, arena);
    }
    
    // For multiple arguments, find the minimum
    let mut min_value = evaluate(&args[0], data, arena)?;
    
    for arg in args.iter().skip(1) {
        let current = evaluate(arg, data, arena)?;
        
        // Compare the values
        match min_value.partial_cmp(current) {
            Some(std::cmp::Ordering::Greater) => {
                min_value = current;
            },
            None => {
                // If we can't compare, keep the existing minimum
            },
            _ => {
                // For Less or Equal, keep the existing minimum
            }
        }
    }
    
    Ok(min_value)
}

/// Evaluates a maximum operation.
pub fn eval_max<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "max".to_string(),
            reason: format!("Expected at least 1 argument, got {}", args.len()),
        });
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        return evaluate(&args[0], data, arena);
    }
    
    // For multiple arguments, find the maximum
    let mut max_value = evaluate(&args[0], data, arena)?;
    
    for arg in args.iter().skip(1) {
        let current = evaluate(arg, data, arena)?;
        
        // Compare the values
        match max_value.partial_cmp(current) {
            Some(std::cmp::Ordering::Less) => {
                max_value = current;
            },
            None => {
                // If we can't compare, keep the existing maximum
            },
            _ => {
                // For Greater or Equal, keep the existing maximum
            }
        }
    }
    
    Ok(max_value)
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