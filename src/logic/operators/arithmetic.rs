//! Arithmetic operators for logic expressions.
//!
//! This module provides implementations for arithmetic operators
//! such as add, subtract, multiply, etc.

use chrono::Duration;
use core::f64;
use std::cmp::Ordering;

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::value::DataValue;
use chrono::{DateTime, Utc};

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
    /// Absolute value
    Abs,
    /// Ceiling (round up)
    Ceil,
    /// Floor (round down)
    Floor,
}

/// Helper function to safely convert a DataValue to f64
fn safe_to_f64(value: &DataValue) -> Result<f64> {
    value
        .coerce_to_number()
        .ok_or(LogicError::NaNError)
        .map(|n| n.as_f64())
}

/// Helper function to create appropriate number type based on value
fn create_number(value: f64, arena: &DataArena) -> &DataValue<'_> {
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        arena.alloc(DataValue::integer(value as i64))
    } else {
        arena.alloc(DataValue::float(value))
    }
}

/// Helper function to extract a datetime from a direct DateTime value or an object with a "datetime" key
fn extract_datetime<'a>(value: &'a DataValue<'a>, arena: &'a DataArena) -> Option<DateTime<Utc>> {
    match value {
        DataValue::DateTime(dt) => Some(*dt),
        DataValue::Object(entries) => {
            // Look for a "datetime" entry
            entries
                .iter()
                .find(|(key, _)| *key == arena.intern_str("datetime"))
                .and_then(|(_, value)| {
                    if let DataValue::DateTime(dt) = value {
                        Some(*dt)
                    } else {
                        None
                    }
                })
        }
        _ => None,
    }
}

/// Helper function to extract a duration from a direct Duration value or an object with a "timestamp" key
fn extract_duration<'a>(value: &'a DataValue<'a>, arena: &'a DataArena) -> Option<Duration> {
    match value {
        DataValue::Duration(dur) => Some(*dur),
        DataValue::Object(entries) => {
            // Look for a "timestamp" entry
            entries
                .iter()
                .find(|(key, _)| *key == arena.intern_str("timestamp"))
                .and_then(|(_, value)| {
                    if let DataValue::Duration(dur) = value {
                        Some(*dur)
                    } else {
                        None
                    }
                })
        }
        _ => None,
    }
}

/// Process potential datetime and duration operations for addition
fn process_datetime_duration_add<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Option<&'a DataValue<'a>> {
    if args.len() != 2 {
        return None;
    }

    // Check for datetime + duration
    let left_dt = extract_datetime(&args[0], arena);
    let right_dur = extract_duration(&args[1], arena);
    if let (Some(dt), Some(dur)) = (left_dt, right_dur) {
        return Some(arena.alloc(DataValue::datetime(dt + dur)));
    }

    // Check for duration + datetime (reverse order)
    let left_dur = extract_duration(&args[0], arena);
    let right_dt = extract_datetime(&args[1], arena);
    if let (Some(dur), Some(dt)) = (left_dur, right_dt) {
        return Some(arena.alloc(DataValue::datetime(dt + dur)));
    }

    // Check for duration + duration
    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        return Some(arena.alloc(DataValue::duration(dur1 + dur2)));
    }

    None
}

/// Process potential datetime and duration operations for subtraction
fn process_datetime_duration_sub<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Option<&'a DataValue<'a>> {
    if args.len() != 2 {
        return None;
    }

    // Check for datetime - datetime = duration
    let left_dt = extract_datetime(&args[0], arena);
    let right_dt = extract_datetime(&args[1], arena);
    if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
        let duration = dt1 - dt2;
        return Some(arena.alloc(DataValue::duration(duration)));
    }

    // Check for datetime - duration = datetime
    let right_dur = extract_duration(&args[1], arena);
    if let (Some(dt), Some(dur)) = (left_dt, right_dur) {
        return Some(arena.alloc(DataValue::datetime(dt - dur)));
    }

    // Check for duration - duration = duration
    let left_dur = extract_duration(&args[0], arena);
    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        return Some(arena.alloc(DataValue::duration(dur1 - dur2)));
    }

    None
}

/// Process potential duration operations for multiplication
fn process_duration_multiplication<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Option<&'a DataValue<'a>> {
    if args.len() != 2 {
        return None;
    }

    // Check for duration * number
    let left_dur = extract_duration(&args[0], arena);
    if let Some(dur) = left_dur {
        if let DataValue::Number(n) = &args[1] {
            let factor = n.as_f64();
            if factor.fract() == 0.0 && factor >= 0.0 {
                let result_dur = dur * (factor as i32);
                return Some(arena.alloc(DataValue::duration(result_dur)));
            }
        }
    }

    // Check for number * duration (reverse order)
    let right_dur = extract_duration(&args[1], arena);
    if let Some(dur) = right_dur {
        if let DataValue::Number(n) = &args[0] {
            let factor = n.as_f64();
            if factor.fract() == 0.0 && factor >= 0.0 {
                let result_dur = dur * (factor as i32);
                return Some(arena.alloc(DataValue::duration(result_dur)));
            }
        }
    }

    None
}

/// Process potential duration operations for division
fn process_duration_division<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Option<&'a DataValue<'a>> {
    if args.len() != 2 {
        return None;
    }

    // Check for duration / number
    let left_dur = extract_duration(&args[0], arena);
    if let Some(dur) = left_dur {
        if let DataValue::Number(n) = &args[1] {
            let divisor = n.as_f64();
            if divisor.fract() == 0.0 && divisor > 0.0 {
                let result_dur = dur / (divisor as i32);
                return Some(arena.alloc(DataValue::duration(result_dur)));
            }
        }
    }

    // Check for duration / duration = number (returns a scalar)
    let right_dur = extract_duration(&args[1], arena);
    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        if dur2.num_seconds() == 0 {
            return None; // Will be handled as a division by zero error later
        }

        // Calculate the ratio
        let ratio = dur1.num_seconds() as f64 / dur2.num_seconds() as f64;
        return Some(create_number(ratio, arena));
    }

    None
}

/// Process numeric addition
fn process_numeric_add<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        // Empty add operation returns 0
        return Ok(arena.alloc(DataValue::integer(0)));
    }

    let mut sum = 0.0;
    for arg in args {
        if let Some(n) = arg.coerce_to_number() {
            sum += n.as_f64();
        } else {
            return Err(LogicError::NaNError);
        }
    }

    Ok(create_number(sum, arena))
}

/// Process numeric subtraction
fn process_numeric_sub<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Get first value
    let first_value = match args[0].coerce_to_number() {
        Some(n) => n.as_f64(),
        None => return Err(LogicError::NaNError),
    };

    // If only one argument, return negation
    if args.len() == 1 {
        return Ok(create_number(-first_value, arena));
    }

    // Otherwise, subtract all other values from the first
    let mut result = first_value;
    for arg in &args[1..] {
        match arg.coerce_to_number() {
            Some(n) => result -= n.as_f64(),
            None => return Err(LogicError::NaNError),
        }
    }

    Ok(create_number(result, arena))
}

/// Process numeric multiplication
fn process_numeric_mul<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        // Empty multiply operation returns 1
        return Ok(arena.alloc(DataValue::integer(1)));
    }

    let mut product = 1.0;
    for arg in args {
        match arg.coerce_to_number() {
            Some(n) => product *= n.as_f64(),
            None => return Err(LogicError::NaNError),
        }
    }

    Ok(create_number(product, arena))
}

/// Process numeric division
fn process_numeric_div<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Get first value
    let first_value = match args[0].coerce_to_number() {
        Some(n) => n.as_f64(),
        None => return Err(LogicError::NaNError),
    };

    // Single operand case: return 1/x (reciprocal)
    if args.len() == 1 {
        if first_value == 0.0 {
            return Err(LogicError::NaNError);
        }
        return Ok(create_number(1.0 / first_value, arena));
    }

    // Divide the first value by all other values
    let mut result = first_value;
    for arg in &args[1..] {
        let divisor = match arg.coerce_to_number() {
            Some(n) => n.as_f64(),
            None => return Err(LogicError::NaNError),
        };

        if divisor == 0.0 {
            return Err(LogicError::NaNError);
        }

        result /= divisor;
    }

    Ok(create_number(result, arena))
}

/// Evaluates an addition operation.
pub fn eval_add<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // First check for datetime/duration operations
    if let Some(result) = process_datetime_duration_add(args, arena) {
        return Ok(result);
    }

    // Fall back to numeric addition
    process_numeric_add(args, arena)
}

/// Evaluates a subtraction operation.
pub fn eval_sub<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // First check for datetime/duration operations
    if let Some(result) = process_datetime_duration_sub(args, arena) {
        return Ok(result);
    }

    // Fall back to numeric subtraction
    process_numeric_sub(args, arena)
}

/// Evaluates a multiplication operation.
pub fn eval_mul<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // First check for duration operations
    if let Some(result) = process_duration_multiplication(args, arena) {
        return Ok(result);
    }

    // Fall back to numeric multiplication
    process_numeric_mul(args, arena)
}

/// Evaluates a division operation.
pub fn eval_div<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // First check for duration operations
    if let Some(result) = process_duration_division(args, arena) {
        return Ok(result);
    }

    // Fall back to numeric division
    process_numeric_div(args, arena)
}

/// Evaluates a modulo operation.
pub fn eval_mod<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => Err(LogicError::InvalidArgumentsError),
        1 => Err(LogicError::InvalidArgumentsError), // Can't do modulo with a single value
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

            Ok(create_number(result, arena))
        }
    }
}

/// Common implementation for min and max operations
fn eval_min_max<'a>(args: &'a [DataValue<'a>], is_min: bool) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => Err(LogicError::InvalidArgumentsError),
        1 => {
            if !args[0].is_number() && !args[0].is_datetime() && !args[0].is_duration() {
                return Err(LogicError::InvalidArgumentsError);
            }
            Ok(&args[0])
        }
        _ => {
            // Special case for datetime
            if args.iter().all(|v| v.is_datetime()) {
                let mut result_value = &args[0];

                for value in &args[1..] {
                    let comparison = value
                        .as_datetime()
                        .unwrap()
                        .cmp(result_value.as_datetime().unwrap());
                    if (is_min && comparison == Ordering::Less)
                        || (!is_min && comparison == Ordering::Greater)
                    {
                        result_value = value;
                    }
                }

                return Ok(result_value);
            }
            // Special case for duration
            else if args.iter().all(|v| v.is_duration()) {
                let mut result_value = &args[0];

                for value in &args[1..] {
                    let comparison = value
                        .as_duration()
                        .unwrap()
                        .cmp(result_value.as_duration().unwrap());
                    if (is_min && comparison == Ordering::Less)
                        || (!is_min && comparison == Ordering::Greater)
                    {
                        result_value = value;
                    }
                }

                return Ok(result_value);
            }

            // Default numeric min/max
            let mut result_value = &args[0];
            let mut result_num = if is_min {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };

            for value in args {
                if !value.is_number() {
                    return Err(LogicError::InvalidArgumentsError);
                }
                let val_num = value.as_f64().unwrap();

                let should_update = if is_min {
                    val_num < result_num
                } else {
                    val_num > result_num
                };
                if should_update {
                    result_value = value;
                    result_num = val_num;
                }
            }

            Ok(result_value)
        }
    }
}

/// Evaluates a min operation with a single argument.
pub fn eval_min<'a>(args: &'a [DataValue<'a>]) -> Result<&'a DataValue<'a>> {
    eval_min_max(args, true)
}

/// Evaluates a max operation with a single argument.
pub fn eval_max<'a>(args: &'a [DataValue<'a>]) -> Result<&'a DataValue<'a>> {
    eval_min_max(args, false)
}

/// Evaluates an absolute value operation.
pub fn eval_abs<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // For a single argument, take its absolute value
    if args.len() == 1 {
        let value = &args[0];
        if !value.is_number() {
            return Err(LogicError::InvalidArgumentsError);
        }

        let num = safe_to_f64(value)?;
        return Ok(create_number(num.abs(), arena));
    }

    // For multiple arguments, take the absolute value of each and return as an array
    let mut result = Vec::with_capacity(args.len());
    for value in args {
        if !value.is_number() {
            return Err(LogicError::InvalidArgumentsError);
        }

        let num = safe_to_f64(value)?;
        result.push(DataValue::float(num.abs()));
    }

    Ok(arena.alloc(DataValue::Array(arena.alloc_data_value_slice(&result))))
}

/// Evaluates a ceiling operation.
pub fn eval_ceil<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // For a single argument, take its ceiling
    if args.len() == 1 {
        let value = &args[0];
        if !value.is_number() {
            return Err(LogicError::InvalidArgumentsError);
        }

        let num = safe_to_f64(value)?;
        return Ok(create_number(num.ceil(), arena));
    }

    // For multiple arguments, take the ceiling of each and return as an array
    let mut result = Vec::with_capacity(args.len());
    for value in args {
        if !value.is_number() {
            return Err(LogicError::InvalidArgumentsError);
        }

        let num = safe_to_f64(value)?;
        result.push(DataValue::float(num.ceil()));
    }

    Ok(arena.alloc(DataValue::Array(arena.alloc_data_value_slice(&result))))
}

/// Evaluates a floor operation.
pub fn eval_floor<'a>(
    args: &'a [DataValue<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // For a single argument, take its floor
    if args.len() == 1 {
        let value = &args[0];
        if !value.is_number() {
            return Err(LogicError::InvalidArgumentsError);
        }

        let num = safe_to_f64(value)?;
        return Ok(create_number(num.floor(), arena));
    }

    // For multiple arguments, take the floor of each and return as an array
    let mut result = Vec::with_capacity(args.len());
    for value in args {
        if !value.is_number() {
            return Err(LogicError::InvalidArgumentsError);
        }

        let num = safe_to_f64(value)?;
        result.push(DataValue::float(num.floor()));
    }

    Ok(arena.alloc(DataValue::Array(arena.alloc_data_value_slice(&result))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_numeric_operations() {
        let arena = DataArena::new();

        // Addition
        let args = [DataValue::integer(3), DataValue::integer(4)];
        let result = eval_add(&args, &arena).unwrap();
        assert_eq!(result.as_f64().unwrap(), 7.0);

        // Subtraction
        let args = [DataValue::integer(7), DataValue::integer(3)];
        let result = eval_sub(&args, &arena).unwrap();
        assert_eq!(result.as_f64().unwrap(), 4.0);

        // Multiplication
        let args = [DataValue::integer(3), DataValue::integer(4)];
        let result = eval_mul(&args, &arena).unwrap();
        assert_eq!(result.as_f64().unwrap(), 12.0);

        // Division
        let args = [DataValue::integer(12), DataValue::integer(4)];
        let result = eval_div(&args, &arena).unwrap();
        assert_eq!(result.as_f64().unwrap(), 3.0);

        // Modulo
        let args = [DataValue::integer(7), DataValue::integer(3)];
        let result = eval_mod(&args, &arena).unwrap();
        assert_eq!(result.as_f64().unwrap(), 1.0);
    }

    #[test]
    fn test_datetime_operations() {
        let arena = DataArena::new();

        // Create test values
        let dt1 = Utc.with_ymd_and_hms(2022, 7, 6, 13, 20, 6).unwrap();
        let dt2 = Utc.with_ymd_and_hms(2022, 7, 7, 13, 20, 6).unwrap();
        let duration = Duration::days(1);

        // Test adding duration to datetime
        let args = [DataValue::datetime(dt1), DataValue::duration(duration)];
        let result = eval_add(&args, &arena).unwrap();
        assert!(result.is_datetime());

        let result_dt = result.as_datetime().unwrap();
        assert_eq!(*result_dt, dt2);

        // Test subtracting duration from datetime
        let args = [DataValue::datetime(dt2), DataValue::duration(duration)];
        let result = eval_sub(&args, &arena).unwrap();
        assert!(result.is_datetime());

        let result_dt = result.as_datetime().unwrap();
        assert_eq!(*result_dt, dt1);

        // Test calculating duration between two datetimes
        let args = [DataValue::datetime(dt2), DataValue::datetime(dt1)];
        let result = eval_sub(&args, &arena).unwrap();
        assert!(result.is_duration());

        let result_dur = result.as_duration().unwrap();
        assert_eq!(result_dur.num_days(), 1);
    }

    #[test]
    fn test_duration_operations() {
        let arena = DataArena::new();

        // Create test values
        let duration1 = Duration::days(1);
        let duration2 = Duration::hours(12);

        // Test adding durations
        let args = [
            DataValue::duration(duration1),
            DataValue::duration(duration2),
        ];
        let result = eval_add(&args, &arena).unwrap();
        assert!(result.is_duration());

        let result_dur = result.as_duration().unwrap();
        assert_eq!(result_dur.num_hours(), 36);

        // Test subtracting durations
        let args = [
            DataValue::duration(duration1),
            DataValue::duration(duration2),
        ];
        let result = eval_sub(&args, &arena).unwrap();
        assert!(result.is_duration());

        let result_dur = result.as_duration().unwrap();
        assert_eq!(result_dur.num_hours(), 12);

        // Test multiplying duration by number
        let args = [DataValue::duration(duration2), DataValue::integer(2)];
        let result = eval_mul(&args, &arena).unwrap();
        assert!(result.is_duration());

        let result_dur = result.as_duration().unwrap();
        assert_eq!(result_dur.num_hours(), 24);

        // Test dividing duration by number
        let args = [DataValue::duration(duration1), DataValue::integer(2)];
        let result = eval_div(&args, &arena).unwrap();
        assert!(result.is_duration());

        let result_dur = result.as_duration().unwrap();
        assert_eq!(result_dur.num_hours(), 12);
    }

    #[test]
    fn test_min_max() {
        let _arena = DataArena::new();

        // Test min with numbers
        let args = [
            DataValue::integer(3),
            DataValue::integer(5),
            DataValue::integer(2),
        ];
        let result = eval_min(&args).unwrap();
        assert_eq!(result.as_i64().unwrap(), 2);

        // Test max with numbers
        let args = [DataValue::integer(5), DataValue::integer(10)];
        let result = eval_max(&args).unwrap();
        assert_eq!(result.as_i64().unwrap(), 10);

        // Test min with datetimes
        let dt1 = Utc.with_ymd_and_hms(2022, 7, 6, 13, 20, 6).unwrap();
        let dt2 = Utc.with_ymd_and_hms(2022, 7, 7, 13, 20, 6).unwrap();
        let args = [DataValue::datetime(dt1), DataValue::datetime(dt2)];
        let result = eval_min(&args).unwrap();
        assert_eq!(*result.as_datetime().unwrap(), dt1);

        // Test max with datetimes
        let result = eval_max(&args).unwrap();
        assert_eq!(*result.as_datetime().unwrap(), dt2);

        // Test min with durations
        let duration1 = Duration::days(1);
        let duration2 = Duration::days(2);
        let args = [
            DataValue::duration(duration1),
            DataValue::duration(duration2),
        ];
        let result = eval_min(&args).unwrap();
        assert_eq!(result.as_duration().unwrap().num_days(), 1);

        // Test max with durations
        let result = eval_max(&args).unwrap();
        assert_eq!(result.as_duration().unwrap().num_days(), 2);
    }
}
