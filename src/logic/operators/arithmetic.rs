//! Arithmetic operators for logic expressions.
//!
//! This module provides implementations for arithmetic operators
//! such as add, subtract, multiply, etc.

use core::f64;

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::value::DataValue;

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
    value
        .coerce_to_number()
        .ok_or(LogicError::NaNError)
        .map(|n| n.as_f64())
}

pub fn eval_add<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => Ok(arena.alloc(DataValue::float(0.0))),
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
        0 => Err(LogicError::InvalidArgumentsError),
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
        0 => Err(LogicError::InvalidArgumentsError),
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
        0 => Err(LogicError::InvalidArgumentsError),
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
        0 => Ok(arena.alloc(DataValue::float(1.0))),
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
        0 => Err(LogicError::InvalidArgumentsError),
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
        0 => Err(LogicError::InvalidArgumentsError),
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
    use crate::DataLogicCore;
    use serde_json::json;

    #[test]
    fn test_add() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"x": 1, "y": 2});

        // Test addition of numbers using int() method instead of operand()
        let rule = builder.arithmetic().add_op().int(1).int(2).int(3).build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(6));

        // Test addition with strings using string() method
        let rule = builder
            .arithmetic()
            .add_op()
            .string("1")
            .string("2")
            .string("3")
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(6));

        // Test addition with booleans using bool() method
        let rule = builder
            .arithmetic()
            .add_op()
            .bool(true)
            .bool(false)
            .bool(true)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(2));

        // Test with single operand (number) using int() method
        let rule = builder.arithmetic().add_op().int(1).build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(1));

        // Test with zero operands
        let rule = builder.arithmetic().add_op().build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(0));

        // Test with variable references
        let rule = builder.arithmetic().add_op().var("x").var("y").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn test_subtract() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 30, "b": 10});

        // Test subtracting numbers
        let rule = builder.arithmetic().subtract_op().var("a").var("b").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(20));

        // Test subtracting multiple numbers
        let rule = builder
            .arithmetic()
            .subtract_op()
            .var("a")
            .var("b")
            .operand(builder.int(5))
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(15));
    }

    #[test]
    fn test_multiply() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 5, "b": 4});

        // Test multiplying numbers using direct var and int methods
        let rule = builder
            .arithmetic()
            .multiply_op()
            .var("a")
            .var("b")
            .int(2)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(40));

        // Test with float values
        let rule = builder
            .arithmetic()
            .multiply_op()
            .var("a")
            .float(1.5)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(7.5));
    }

    #[test]
    fn test_divide() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 20, "b": 4});

        // Test basic division with variables
        let rule = builder.arithmetic().divide_op().var("a").var("b").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(5));

        // Test division with mixed types (int and float)
        let rule = builder.arithmetic().divide_op().int(10).float(2.5).build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(4));

        // Test division with string conversion
        let rule = builder.arithmetic().divide_op().string("9").int(3).build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn test_modulo() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 23, "b": 5});

        // Test modulo
        let rule = builder.arithmetic().modulo_op().var("a").var("b").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(3));
    }
}
