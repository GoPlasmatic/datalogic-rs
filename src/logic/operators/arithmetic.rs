//! Arithmetic operators for logic expressions.
//!
//! This module provides implementations for arithmetic operators
//! such as add, subtract, multiply, etc.

use core::f64;

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::error::{LogicError, Result};

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

/// Helper function to safely convert a DataValue to f64
fn safe_to_f64(value: &DataValue) -> Result<f64> {
    value.coerce_to_number().ok_or(LogicError::NaNError).map(|n| n.as_f64())
}

pub fn eval_add<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => {
            Ok(arena.alloc(DataValue::float(0.0)))
        }
        1 => {
            let result = safe_to_f64(&args[0])?;
            Ok(arena.alloc(DataValue::float(result)))
        }
        _ => {
            let mut result = 0.0;
            for value in args {
                result += safe_to_f64(value)?;
            }

            Ok(arena.alloc(DataValue::float(result)))
        }
    }
}

/// Evaluates a subtraction operation with a single argument.
pub fn eval_sub<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => {
            Err(LogicError::InvalidArgumentsError)
        }
        1 => {
            let result = safe_to_f64(&args[0])?;
            Ok(arena.alloc(DataValue::float(-result)))
        }
        _ => {
            let first = safe_to_f64(&args[0])?;
            let mut result = first;
            
            for value in &args[1..] {
                result -= safe_to_f64(value)?;
            }

            Ok(arena.alloc(DataValue::float(result)))
        }
    }
}

/// Evaluates a division operation with a single argument.
pub fn eval_div<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => {
            Err(LogicError::InvalidArgumentsError)
        }
        1 => {
            let value = safe_to_f64(&args[0])?;
            if value == 0.0 {
                return Err(LogicError::NaNError);
            }
            Ok(arena.alloc(DataValue::float(1.0 / value)))
        }
        _ => {
            let first = safe_to_f64(&args[0])?;
            let mut result = first;
            
            for value in &args[1..] {
                let divisor = safe_to_f64(value)?;
                if divisor == 0.0 {
                    return Err(LogicError::NaNError);
                }
                result /= divisor;
            }

            Ok(arena.alloc(DataValue::float(result)))
        }
    }
}

/// Evaluates a modulo operation with a single argument.
pub fn eval_mod<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => {
            Err(LogicError::InvalidArgumentsError)
        }
        1 => {
            // Can't do modulo with a single value
            Err(LogicError::InvalidArgumentsError)
        }
        _ => {
            let first = safe_to_f64(&args[0])?;
            let mut result = first;
            
            for value in &args[1..] {
                let divisor = safe_to_f64(value)?;
                if divisor == 0.0 {
                    return Err(LogicError::NaNError);
                }
                result %= divisor;
            }

            Ok(arena.alloc(DataValue::float(result)))
        }
    }
}

/// Evaluates a multiplication operation with a single argument.
pub fn eval_mul<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => {
            Ok(arena.alloc(DataValue::float(1.0)))
        }
        1 => {
            let result = safe_to_f64(&args[0])?;
            Ok(arena.alloc(DataValue::float(result)))
        }
        _ => {
            let mut result = 1.0;
            for value in args {
                result *= safe_to_f64(value)?;
            }

            Ok(arena.alloc(DataValue::float(result)))
        }
    }
}

/// Evaluates a min operation with a single argument.
pub fn eval_min<'a>(args: &'a [DataValue<'a>]) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => {
            Err(LogicError::InvalidArgumentsError)
        }
        1 => {
            if !args[0].is_number() {
                return Err(LogicError::InvalidArgumentsError);
            }
            Ok(&args[0])
        }
        _ => {
            let mut min_value = &args[0];
            let mut min_num = f64::INFINITY;
            
            for value in args {
                if !value.is_number() {
                    return Err(LogicError::InvalidArgumentsError);
                }
                let val_num = value.as_f64().unwrap();
                
                if val_num < min_num {
                    min_value = value;
                    min_num = val_num;
                }
            }
            
            Ok(min_value)
        }
    }
}

/// Evaluates a max operation with a single argument.
pub fn eval_max<'a>(args: &'a [DataValue<'a>]) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => {
            Err(LogicError::InvalidArgumentsError)
        }
        1 => {
            if !args[0].is_number() {
                return Err(LogicError::InvalidArgumentsError);
            }
            Ok(&args[0])
        }
        _ => {
            let mut max_value = &args[0];
            let mut max_num = f64::NEG_INFINITY;
            
            for value in args {
                if !value.is_number() {
                    return Err(LogicError::InvalidArgumentsError);
                }
                let val_num = value.as_f64().unwrap();
                
                if val_num > max_num {
                    max_value = value;
                    max_num = val_num;
                }
            }
            
            Ok(max_value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use crate::logic::evaluator::evaluate;
    use serde_json::json;
    
    #[test]
    fn test_add() {
        let arena = DataArena::new();
        let data_json = json!({"a": 10, "b": 20, "c": "hello", "x": 1, "y": 2});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test adding numbers
        let token = parse_str(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(30.0));
        
        // Test adding multiple numbers
        let token = parse_str(r#"{"+": [{"var": "a"}, {"var": "b"}, 5]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(35.0));
        
        // Test addition with multiple operands
        let token = parse_str(r#"{"+": [1, 2, 3, 4]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(10.0));
        
        // Test addition with negative numbers
        let token = parse_str(r#"{"+": [-1, 0, 5]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(4.0));
        
        // Test addition with strings (coerced to numbers)
        let token = parse_str(r#"{"+": ["1", "2", "3"]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(6.0));
        
        // Test addition with booleans
        let token = parse_str(r#"{"+": [true, false, true]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(2.0));
        
        // Test with single operand (number)
        let token = parse_str(r#"{"+": [1]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(1.0));
        
        // Test with zero operands
        let token = parse_str(r#"{"+": []}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(0.0));
        
        // Test with single direct operand
        let token = parse_str(r#"{"+": 1}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(1.0));
        
        // Test with variable references
        let token = parse_str(r#"{"+": [{"var": "x"}, {"var": "y"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(3.0));
        
        // Test with dynamic array
        let token = parse_str(r#"{"+": {"preserve": [7, 8]}}"#, &arena).unwrap();
        println!("token: {:?}", token);
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_f64(), Some(15.0));
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