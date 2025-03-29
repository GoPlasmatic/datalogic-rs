//! Arithmetic operators for logic expressions.
//!
//! This module provides implementations for arithmetic operators
//! such as add, subtract, multiply, etc.

use chrono::Duration;
use core::f64;

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
}

/// Helper function to safely convert a DataValue to f64
fn safe_to_f64(value: &DataValue) -> Result<f64> {
    value
        .coerce_to_number()
        .ok_or(LogicError::NaNError)
        .map(|n| n.as_f64())
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

/// Helper function to create a datetime object
fn create_datetime_object(dt: DateTime<Utc>, arena: &DataArena) -> &DataValue<'_> {
    let dt_val = DataValue::datetime(dt);

    // Create an object with {"datetime": dt_val}
    let entries = arena.vec_into_slice(vec![(arena.intern_str("datetime"), dt_val)]);

    arena.alloc(DataValue::Object(entries))
}

/// Helper function to create a duration object
fn create_duration_object(dur: Duration, arena: &DataArena) -> &DataValue<'_> {
    let dur_val = DataValue::duration(dur);

    // Create an object with {"timestamp": dur_val}
    let entries = arena.vec_into_slice(vec![(arena.intern_str("timestamp"), dur_val)]);

    arena.alloc(DataValue::Object(entries))
}

/// Evaluates an addition operation.
pub fn eval_add<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        // Empty add operation returns 0
        return Ok(arena.alloc(DataValue::integer(0)));
    }

    // Check for datetime + duration first
    if args.len() == 2 {
        let left_dt = extract_datetime(&args[0], arena);
        let right_dur = extract_duration(&args[1], arena);

        if let (Some(dt), Some(dur)) = (left_dt, right_dur) {
            return Ok(create_datetime_object(dt + dur, arena));
        }

        // Check for duration + datetime (reverse order)
        let left_dur = extract_duration(&args[0], arena);
        let right_dt = extract_datetime(&args[1], arena);

        if let (Some(dur), Some(dt)) = (left_dur, right_dt) {
            return Ok(create_datetime_object(dt + dur, arena));
        }

        // Check for duration + duration
        if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
            return Ok(create_duration_object(dur1 + dur2, arena));
        }
    }

    // For numeric values, perform conventional addition
    let mut sum = 0.0;
    for arg in args {
        match arg {
            DataValue::Number(n) => {
                sum += n.as_f64();
            }
            DataValue::String(s) => {
                // Special case for empty string - treat as 0
                if s.is_empty() {
                    sum += 0.0;
                } else if let Ok(num) = s.parse::<f64>() {
                    sum += num;
                } else {
                    return Err(LogicError::NaNError);
                }
            }
            DataValue::Bool(b) => {
                sum += if *b { 1.0 } else { 0.0 };
            }
            DataValue::Null => {
                sum += 0.0;
            }
            _ => {
                // For other types, try to convert to number
                if let Some(n) = arg.coerce_to_number() {
                    sum += n.as_f64();
                } else {
                    return Err(LogicError::NaNError);
                }
            }
        }
    }

    // Check if it's an integer or a floating point
    if sum.fract() == 0.0 && sum >= i64::MIN as f64 && sum <= i64::MAX as f64 {
        Ok(arena.alloc(DataValue::integer(sum as i64)))
    } else {
        Ok(arena.alloc(DataValue::float(sum)))
    }
}

/// Evaluates a subtraction operation.
pub fn eval_sub<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Check for datetime and duration operations first
    if args.len() == 2 {
        let left_dt = extract_datetime(&args[0], arena);
        let right_dt = extract_datetime(&args[1], arena);

        // Datetime - Datetime = Duration
        if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
            let duration = dt1 - dt2;
            return Ok(create_duration_object(duration, arena));
        }

        let right_dur = extract_duration(&args[1], arena);

        // Datetime - Duration = Datetime
        if let (Some(dt), Some(dur)) = (left_dt, right_dur) {
            return Ok(create_datetime_object(dt - dur, arena));
        }

        let left_dur = extract_duration(&args[0], arena);

        // Duration - Duration = Duration
        if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
            return Ok(create_duration_object(dur1 - dur2, arena));
        }
    }

    // Regular numeric subtraction
    let first = &args[0];
    let first_value = match first {
        DataValue::Number(n) => n.as_f64(),
        DataValue::String(s) => {
            // Special case for empty string - treat as 0
            if s.is_empty() {
                0.0
            } else if let Ok(num) = s.parse::<f64>() {
                num
            } else {
                return Err(LogicError::NaNError);
            }
        }
        DataValue::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        DataValue::Null => 0.0,
        _ => {
            // For other types, try to convert to number
            if let Some(n) = first.coerce_to_number() {
                n.as_f64()
            } else {
                return Err(LogicError::NaNError);
            }
        }
    };

    // If only one argument, return negation
    if args.len() == 1 {
        if first_value.fract() == 0.0
            && -first_value >= i64::MIN as f64
            && -first_value <= i64::MAX as f64
        {
            return Ok(arena.alloc(DataValue::integer(-first_value as i64)));
        } else {
            return Ok(arena.alloc(DataValue::float(-first_value)));
        }
    }

    // Otherwise, subtract all other values from the first
    let mut result = first_value;
    for arg in &args[1..] {
        match arg {
            DataValue::Number(n) => {
                result -= n.as_f64();
            }
            DataValue::String(s) => {
                // Special case for empty string - treat as 0
                if s.is_empty() {
                    result -= 0.0;
                } else if let Ok(num) = s.parse::<f64>() {
                    result -= num;
                } else {
                    return Err(LogicError::NaNError);
                }
            }
            DataValue::Bool(b) => {
                result -= if *b { 1.0 } else { 0.0 };
            }
            DataValue::Null => {
                result -= 0.0;
            }
            _ => {
                // For other types, try to convert to number
                if let Some(n) = arg.coerce_to_number() {
                    result -= n.as_f64();
                } else {
                    return Err(LogicError::NaNError);
                }
            }
        }
    }

    // Check if it's an integer or a floating point
    if result.fract() == 0.0 && result >= i64::MIN as f64 && result <= i64::MAX as f64 {
        Ok(arena.alloc(DataValue::integer(result as i64)))
    } else {
        Ok(arena.alloc(DataValue::float(result)))
    }
}

/// Evaluates a multiplication operation.
pub fn eval_mul<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        // Empty multiply operation returns 1
        return Ok(arena.alloc(DataValue::integer(1)));
    }

    // Check for duration * number
    if args.len() == 2 {
        let left_dur = extract_duration(&args[0], arena);

        if let Some(dur) = left_dur {
            if let DataValue::Number(n) = &args[1] {
                let factor = n.as_f64();
                if factor.fract() == 0.0 && factor >= 0.0 {
                    let result_dur = dur * (factor as i32);
                    return Ok(create_duration_object(result_dur, arena));
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
                    return Ok(create_duration_object(result_dur, arena));
                }
            }
        }
    }

    // Regular numeric multiplication
    let mut product = 1.0;
    for arg in args {
        match arg {
            DataValue::Number(n) => {
                product *= n.as_f64();
            }
            DataValue::String(s) => {
                // Special case for empty string - treat as 0
                if s.is_empty() {
                    product *= 0.0;
                } else if let Ok(num) = s.parse::<f64>() {
                    product *= num;
                } else {
                    return Err(LogicError::NaNError);
                }
            }
            DataValue::Bool(b) => {
                product *= if *b { 1.0 } else { 0.0 };
            }
            DataValue::Null => {
                product *= 0.0;
            }
            _ => {
                // For other types, try to convert to number
                if let Some(n) = arg.coerce_to_number() {
                    product *= n.as_f64();
                } else {
                    return Err(LogicError::NaNError);
                }
            }
        }
    }

    // Check if it's an integer or a floating point
    if product.fract() == 0.0 && product >= i64::MIN as f64 && product <= i64::MAX as f64 {
        Ok(arena.alloc(DataValue::integer(product as i64)))
    } else {
        Ok(arena.alloc(DataValue::float(product)))
    }
}

/// Evaluates a division operation.
pub fn eval_div<'a>(args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Single operand case: return 1/x (reciprocal)
    if args.len() == 1 {
        let value = match &args[0] {
            DataValue::Number(n) => n.as_f64(),
            DataValue::String(s) => {
                if let Ok(num) = s.parse::<f64>() {
                    num
                } else {
                    return Err(LogicError::NaNError);
                }
            }
            DataValue::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            DataValue::Null => 0.0,
            _ => {
                // For other types, try to convert to number
                if let Some(n) = args[0].coerce_to_number() {
                    n.as_f64()
                } else {
                    return Err(LogicError::NaNError);
                }
            }
        };

        if value == 0.0 {
            return Err(LogicError::NaNError);
        }

        let result = 1.0 / value;

        // Check if it's an integer or a floating point
        if result.fract() == 0.0 && result >= i64::MIN as f64 && result <= i64::MAX as f64 {
            return Ok(arena.alloc(DataValue::integer(result as i64)));
        } else {
            return Ok(arena.alloc(DataValue::float(result)));
        }
    }

    // Check for duration / number
    if args.len() == 2 {
        let left_dur = extract_duration(&args[0], arena);

        if let Some(dur) = left_dur {
            if let DataValue::Number(n) = &args[1] {
                let divisor = n.as_f64();
                if divisor.fract() == 0.0 && divisor > 0.0 {
                    let result_dur = dur / (divisor as i32);
                    return Ok(create_duration_object(result_dur, arena));
                }
            }
        }

        // Check for duration / duration = number (returns a scalar)
        let right_dur = extract_duration(&args[1], arena);

        if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
            if dur2.num_seconds() == 0 {
                return Err(LogicError::NaNError);
            }

            // Calculate the ratio
            let ratio = dur1.num_seconds() as f64 / dur2.num_seconds() as f64;

            // Check if it's an integer or a floating point
            if ratio.fract() == 0.0 && ratio >= i64::MIN as f64 && ratio <= i64::MAX as f64 {
                return Ok(arena.alloc(DataValue::integer(ratio as i64)));
            } else {
                return Ok(arena.alloc(DataValue::float(ratio)));
            }
        }
    }

    // Regular numeric division
    let first = &args[0];
    let first_value = match first {
        DataValue::Number(n) => n.as_f64(),
        DataValue::String(s) => {
            if let Ok(num) = s.parse::<f64>() {
                num
            } else {
                return Err(LogicError::NaNError);
            }
        }
        DataValue::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        DataValue::Null => 0.0,
        _ => {
            // For other types, try to convert to number
            if let Some(n) = first.coerce_to_number() {
                n.as_f64()
            } else {
                return Err(LogicError::NaNError);
            }
        }
    };

    // Divide the first value by all other values
    let mut result = first_value;
    for arg in &args[1..] {
        let divisor = match arg {
            DataValue::Number(n) => n.as_f64(),
            DataValue::String(s) => {
                if let Ok(num) = s.parse::<f64>() {
                    num
                } else {
                    return Err(LogicError::NaNError);
                }
            }
            DataValue::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            DataValue::Null => 0.0,
            _ => {
                // For other types, try to convert to number
                if let Some(n) = arg.coerce_to_number() {
                    n.as_f64()
                } else {
                    return Err(LogicError::NaNError);
                }
            }
        };

        if divisor == 0.0 {
            return Err(LogicError::NaNError);
        }

        result /= divisor;
    }

    // Check if it's an integer or a floating point
    if result.fract() == 0.0 && result >= i64::MIN as f64 && result <= i64::MAX as f64 {
        Ok(arena.alloc(DataValue::integer(result as i64)))
    } else {
        Ok(arena.alloc(DataValue::float(result)))
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

/// Evaluates a min operation with a single argument.
pub fn eval_min<'a>(args: &'a [DataValue<'a>]) -> Result<&'a DataValue<'a>> {
    match args.len() {
        0 => Err(LogicError::InvalidArgumentsError),
        1 => {
            if !args[0].is_number() && !args[0].is_datetime() && !args[0].is_duration() {
                return Err(LogicError::InvalidArgumentsError);
            }
            Ok(&args[0])
        }
        _ => {
            // Special case for datetime and duration
            if args.iter().all(|v| v.is_datetime()) {
                let mut min_value = &args[0];

                for value in &args[1..] {
                    if value.as_datetime().unwrap() < min_value.as_datetime().unwrap() {
                        min_value = value;
                    }
                }

                return Ok(min_value);
            } else if args.iter().all(|v| v.is_duration()) {
                let mut min_value = &args[0];

                for value in &args[1..] {
                    if value.as_duration().unwrap() < min_value.as_duration().unwrap() {
                        min_value = value;
                    }
                }

                return Ok(min_value);
            }

            // Default numeric min
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
            if !args[0].is_number() && !args[0].is_datetime() && !args[0].is_duration() {
                return Err(LogicError::InvalidArgumentsError);
            }
            Ok(&args[0])
        }
        _ => {
            // Special case for datetime and duration
            if args.iter().all(|v| v.is_datetime()) {
                let mut max_value = &args[0];

                for value in &args[1..] {
                    if value.as_datetime().unwrap() > max_value.as_datetime().unwrap() {
                        max_value = value;
                    }
                }

                return Ok(max_value);
            } else if args.iter().all(|v| v.is_duration()) {
                let mut max_value = &args[0];

                for value in &args[1..] {
                    if value.as_duration().unwrap() > max_value.as_duration().unwrap() {
                        max_value = value;
                    }
                }

                return Ok(max_value);
            }

            // Default numeric max
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
        assert!(result.is_object());

        // Check that it's an object with a "datetime" key
        let entries = result.as_object().unwrap();
        let datetime_entry = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("datetime"));
        assert!(datetime_entry.is_some());

        let (_, dt_val) = datetime_entry.unwrap();
        assert!(dt_val.is_datetime());
        assert_eq!(*dt_val.as_datetime().unwrap(), dt2);

        // Test subtracting duration from datetime
        let args = [DataValue::datetime(dt2), DataValue::duration(duration)];
        let result = eval_sub(&args, &arena).unwrap();
        assert!(result.is_object());

        // Check that it's an object with a "datetime" key
        let entries = result.as_object().unwrap();
        let datetime_entry = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("datetime"));
        assert!(datetime_entry.is_some());

        let (_, dt_val) = datetime_entry.unwrap();
        assert!(dt_val.is_datetime());
        assert_eq!(*dt_val.as_datetime().unwrap(), dt1);

        // Test calculating duration between two datetimes
        let args = [DataValue::datetime(dt2), DataValue::datetime(dt1)];
        let result = eval_sub(&args, &arena).unwrap();
        assert!(result.is_object());

        // Check that it's an object with a "timestamp" key
        let entries = result.as_object().unwrap();
        let timestamp_entry = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("timestamp"));
        assert!(timestamp_entry.is_some());

        let (_, dur_val) = timestamp_entry.unwrap();
        assert!(dur_val.is_duration());
        assert_eq!(dur_val.as_duration().unwrap().num_days(), 1);
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
        assert!(result.is_object());

        // Check that it's an object with a "timestamp" key
        let entries = result.as_object().unwrap();
        let timestamp_entry = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("timestamp"));
        assert!(timestamp_entry.is_some());

        let (_, dur_val) = timestamp_entry.unwrap();
        assert!(dur_val.is_duration());
        assert_eq!(dur_val.as_duration().unwrap().num_hours(), 36);

        // Test subtracting durations
        let args = [
            DataValue::duration(duration1),
            DataValue::duration(duration2),
        ];
        let result = eval_sub(&args, &arena).unwrap();
        assert!(result.is_object());

        // Check that it's an object with a "timestamp" key
        let entries = result.as_object().unwrap();
        let timestamp_entry = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("timestamp"));
        assert!(timestamp_entry.is_some());

        let (_, dur_val) = timestamp_entry.unwrap();
        assert!(dur_val.is_duration());
        assert_eq!(dur_val.as_duration().unwrap().num_hours(), 12);

        // Test multiplying duration by number
        let args = [DataValue::duration(duration2), DataValue::integer(2)];
        let result = eval_mul(&args, &arena).unwrap();
        assert!(result.is_object());

        // Check that it's an object with a "timestamp" key
        let entries = result.as_object().unwrap();
        let timestamp_entry = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("timestamp"));
        assert!(timestamp_entry.is_some());

        let (_, dur_val) = timestamp_entry.unwrap();
        assert!(dur_val.is_duration());
        assert_eq!(dur_val.as_duration().unwrap().num_hours(), 24);

        // Test dividing duration by number
        let args = [DataValue::duration(duration1), DataValue::integer(2)];
        let result = eval_div(&args, &arena).unwrap();
        assert!(result.is_object());

        // Check that it's an object with a "timestamp" key
        let entries = result.as_object().unwrap();
        let timestamp_entry = entries
            .iter()
            .find(|(key, _)| *key == arena.intern_str("timestamp"));
        assert!(timestamp_entry.is_some());

        let (_, dur_val) = timestamp_entry.unwrap();
        assert!(dur_val.is_duration());
        assert_eq!(dur_val.as_duration().unwrap().num_hours(), 12);
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
