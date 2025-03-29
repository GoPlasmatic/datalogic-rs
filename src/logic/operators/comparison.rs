//! Comparison operators for logic expressions.
//!
//! This module provides implementations for comparison operators
//! such as equal, not equal, greater than, etc.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;
use chrono::{DateTime, Duration, Utc};

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

/// Helper function to extract a datetime from a direct DateTime value or an object with a "datetime" key
fn extract_datetime<'a>(
    value: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Option<&'a DateTime<Utc>> {
    match value {
        DataValue::DateTime(dt) => Some(dt),
        DataValue::Object(entries) => {
            // Look for a "datetime" entry
            entries
                .iter()
                .find(|(key, _)| *key == arena.intern_str("datetime"))
                .and_then(|(_, value)| {
                    if let DataValue::DateTime(dt) = value {
                        Some(dt)
                    } else {
                        None
                    }
                })
        }
        _ => None,
    }
}

/// Helper function to extract a duration from a direct Duration value or an object with a "timestamp" key
fn extract_duration<'a>(value: &'a DataValue<'a>, arena: &'a DataArena) -> Option<&'a Duration> {
    match value {
        DataValue::Duration(dur) => Some(dur),
        DataValue::Object(entries) => {
            // Look for a "timestamp" entry
            entries
                .iter()
                .find(|(key, _)| *key == arena.intern_str("timestamp"))
                .and_then(|(_, value)| {
                    if let DataValue::Duration(dur) = value {
                        Some(dur)
                    } else {
                        None
                    }
                })
        }
        _ => None,
    }
}

/// Evaluates an equality comparison.
pub fn eval_equal<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], arena)?;
        let right = evaluate(args[i + 1], arena)?;

        // Fast path for identical references
        if std::ptr::eq(left as *const DataValue, right as *const DataValue) {
            continue;
        }

        // Try to extract datetime values
        let left_dt = extract_datetime(left, arena);
        let right_dt = extract_datetime(right, arena);

        // If both values are datetimes, compare them
        if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
            if left_dt != right_dt {
                return Ok(arena.false_value());
            }
            continue;
        }

        // Try to extract duration values
        let left_dur = extract_duration(left, arena);
        let right_dur = extract_duration(right, arena);

        // If both values are durations, compare them
        if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
            if left_dur != right_dur {
                return Ok(arena.false_value());
            }
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
                // But we already handled the case where both are datetime objects above
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
pub fn eval_strict_equal<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], arena)?;
        let right = evaluate(args[i + 1], arena)?;

        if !left.strict_equals(right) {
            return Ok(arena.false_value());
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a not equal comparison.
pub fn eval_not_equal<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], arena)?;
        let right = evaluate(args[i + 1], arena)?;

        // Try to extract datetime values
        let left_dt = extract_datetime(left, arena);
        let right_dt = extract_datetime(right, arena);

        // If both values are datetimes, compare them
        if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
            if left_dt == right_dt {
                return Ok(arena.false_value());
            }
            continue;
        }

        // Try to extract duration values
        let left_dur = extract_duration(left, arena);
        let right_dur = extract_duration(right, arena);

        // If both values are durations, compare them
        if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
            if left_dur == right_dur {
                return Ok(arena.false_value());
            }
            continue;
        }

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
pub fn eval_strict_not_equal<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], arena)?;
        let right = evaluate(args[i + 1], arena)?;

        if left.strict_equals(right) {
            return Ok(arena.false_value());
        }
    }

    Ok(arena.true_value())
}

/// Evaluates a greater-than comparison.
pub fn eval_greater_than<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], arena)?;
        let right = evaluate(args[i + 1], arena)?;

        // Try to extract datetime values
        let left_dt = extract_datetime(left, arena);
        let right_dt = extract_datetime(right, arena);

        // If both values are datetimes, compare them
        if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
            if left_dt <= right_dt {
                return Ok(arena.false_value());
            }
            continue;
        }

        // Try to extract duration values
        let left_dur = extract_duration(left, arena);
        let right_dur = extract_duration(right, arena);

        // If both values are durations, compare them
        if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
            if left_dur <= right_dur {
                return Ok(arena.false_value());
            }
            continue;
        }

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
pub fn eval_greater_than_or_equal<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], arena)?;
        let right = evaluate(args[i + 1], arena)?;

        // Try to extract datetime values
        let left_dt = extract_datetime(left, arena);
        let right_dt = extract_datetime(right, arena);

        // If both values are datetimes, compare them
        if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
            if left_dt < right_dt {
                return Ok(arena.false_value());
            }
            continue;
        }

        // Try to extract duration values
        let left_dur = extract_duration(left, arena);
        let right_dur = extract_duration(right, arena);

        // If both values are durations, compare them
        if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
            if left_dur < right_dur {
                return Ok(arena.false_value());
            }
            continue;
        }

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
                return Ok(arena.true_value());
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
pub fn eval_less_than<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], arena)?;
        let right = evaluate(args[i + 1], arena)?;

        // Try to extract datetime values
        let left_dt = extract_datetime(left, arena);
        let right_dt = extract_datetime(right, arena);

        // If both values are datetimes, compare them
        if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
            if left_dt >= right_dt {
                return Ok(arena.false_value());
            }
            continue;
        }

        // Try to extract duration values
        let left_dur = extract_duration(left, arena);
        let right_dur = extract_duration(right, arena);

        // If both values are durations, compare them
        if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
            if left_dur >= right_dur {
                return Ok(arena.false_value());
            }
            continue;
        }

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
pub fn eval_less_than_or_equal<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], arena)?;
        let right = evaluate(args[i + 1], arena)?;

        // Try to extract datetime values
        let left_dt = extract_datetime(left, arena);
        let right_dt = extract_datetime(right, arena);

        // If both values are datetimes, compare them
        if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
            if left_dt > right_dt {
                return Ok(arena.false_value());
            }
            continue;
        }

        // Try to extract duration values
        let left_dur = extract_duration(left, arena);
        let right_dur = extract_duration(right, arena);

        // If both values are durations, compare them
        if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
            if left_dur > right_dur {
                return Ok(arena.false_value());
            }
            continue;
        }

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
                return Ok(arena.true_value());
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
    use crate::logic::datalogic_core::DataLogicCore;
    use serde_json::json;

    #[test]
    fn test_equality() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 10, "b": "10", "c": 20, "d": 10});

        // Test equal with same type
        let rule = builder.compare().equal_op().var("a").int(10).build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test equal with different types (number and string)
        let rule = builder.compare().equal_op().var("a").var("b").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test not equal
        let rule = builder.compare().equal_op().var("a").var("c").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));

        // Test variadic equal (a = d = 10)
        let rule = builder
            .compare()
            .equal_op()
            .var("a")
            .var("d")
            .int(10)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test variadic equal failing (a = c = 10)
        let rule = builder
            .compare()
            .equal_op()
            .var("a")
            .var("c")
            .int(10)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_not_equal() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 10, "b": "10", "c": 20, "d": 30});

        // Test not equal with two arguments
        let rule = builder.compare().not_equal_op().var("a").var("c").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test not equal with same values
        let rule = builder.compare().not_equal_op().var("a").int(10).build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));

        // Test not equal with multiple arguments (chain comparison)
        // For multiple arguments, we need to chain comparisons with AND
        let comparison1 = builder.compare().not_equal_op().var("a").int(10).build();

        let comparison2 = builder.compare().not_equal_op().var("b").int(10).build();

        let comparison3 = builder.compare().not_equal_op().var("c").int(10).build();

        let rule = builder
            .control()
            .and_op()
            .operand(comparison1)
            .operand(comparison2)
            .operand(comparison3)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));

        // Test not equal with different values in a chain
        let comparison1 = builder.compare().not_equal_op().var("a").var("b").build();

        let comparison2 = builder.compare().not_equal_op().var("b").var("c").build();

        let rule = builder
            .control()
            .and_op()
            .operand(comparison1)
            .operand(comparison2)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_strict_equal() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 10, "b": "10", "c": 20});

        // Test strict equal with same type
        let rule = builder.compare().strict_equal_op().var("a").int(10).build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test strict equal with different types (number and string)
        let rule = builder
            .compare()
            .strict_equal_op()
            .var("a")
            .var("b")
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_greater_than() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 10, "b": 5, "c": "20", "d": 30, "e": 3});

        // Test greater than with numbers
        let rule = builder
            .compare()
            .greater_than_op()
            .var("a")
            .var("b")
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test greater than with string coercion
        let rule = builder
            .compare()
            .greater_than_op()
            .var("c")
            .var("a")
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test variadic greater than (d > a > b > e), which should be true
        let rule = builder
            .compare()
            .greater_than_op()
            .var("d") // 30
            .var("a") // 10
            .var("b") // 5
            .var("e") // 3
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test variadic greater than (a > b > c) which should be false
        let rule = builder
            .compare()
            .greater_than_op()
            .var("a") // 10
            .var("b") // 5
            .var("c") // "20"
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_less_than() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 10, "b": 5, "c": "20"});

        // Test less than with numbers
        let rule = builder.compare().less_than_op().var("b").var("a").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test less than with string coercion
        let rule = builder.compare().less_than_op().var("a").var("c").build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test variadic less than (b < a < c)
        let rule = builder
            .compare()
            .less_than_op()
            .var("b") // 5
            .var("a") // 10
            .var("c") // "20"
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
    }
}
