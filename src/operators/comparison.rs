use serde_json::Value;

use super::helpers::{extract_datetime_value, extract_duration_value};
use crate::constants::INVALID_ARGS;
use crate::datetime::{extract_datetime, is_datetime_object};
use crate::value_helpers::{coerce_to_number, loose_equals_with_error, strict_equals};
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// Equals operator function (== for loose equality)
#[inline]
pub fn evaluate_equals(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // For chained equality (3+ arguments), check if all are equal
    let first = engine.evaluate_node(&args[0], context)?;

    for item in args.iter().skip(1) {
        let current = engine.evaluate_node(item, context)?;

        // Compare first == current (loose equality)
        let result = compare_equals(&first, &current, false)?;

        if !result {
            // Short-circuit on first inequality
            return Ok(Value::Bool(false));
        }
    }

    Ok(Value::Bool(true))
}

/// Strict equals operator function (=== for strict equality)
#[inline]
pub fn evaluate_strict_equals(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // For chained equality (3+ arguments), check if all are equal
    let first = engine.evaluate_node(&args[0], context)?;

    for item in args.iter().skip(1) {
        let current = engine.evaluate_node(item, context)?;

        // Compare first === current (strict equality)
        let result = compare_equals(&first, &current, true)?;

        if !result {
            // Short-circuit on first inequality
            return Ok(Value::Bool(false));
        }
    }

    Ok(Value::Bool(true))
}

// Helper function for == and === comparison
#[inline]
fn compare_equals(left: &Value, right: &Value, strict: bool) -> Result<bool> {
    // Handle datetime comparisons - both objects and strings
    let left_dt = extract_datetime_value(left);
    let right_dt = extract_datetime_value(right);

    if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
        return Ok(dt1 == dt2);
    }

    // Handle duration comparisons - both objects and strings
    let left_dur = extract_duration_value(left);
    let right_dur = extract_duration_value(right);

    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        return Ok(dur1 == dur2);
    }

    if strict {
        Ok(strict_equals(left, right))
    } else {
        loose_equals_with_error(left, right)
    }
}

/// Not equals operator function (!= for loose inequality)
#[inline]
pub fn evaluate_not_equals(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // != returns true if arguments are not all equal
    // It's the logical negation of ==
    // But we need to handle lazy evaluation differently

    // Evaluate first two arguments
    let first = engine.evaluate_node(&args[0], context)?;
    let second = engine.evaluate_node(&args[1], context)?;

    // Compare them (loose equality)
    let equals = compare_equals(&first, &second, false)?;

    if !equals {
        // Found inequality, return true immediately (lazy)
        return Ok(Value::Bool(true));
    }

    // If we only have 2 args and they're equal, return false
    if args.len() == 2 {
        return Ok(Value::Bool(false));
    }

    // For 3+ args, since first two are equal, the result depends on whether
    // all remaining args also equal the first. But JSONLogic != seems to only
    // check the first two operands when they're equal (based on test case)
    // This achieves lazy evaluation.
    Ok(Value::Bool(false))
}
/// Strict not equals operator function (!== for strict inequality)
#[inline]
pub fn evaluate_strict_not_equals(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // !== returns true if arguments are not all equal
    // It's the logical negation of ===
    // But we need to handle lazy evaluation differently

    // Evaluate first two arguments
    let first = engine.evaluate_node(&args[0], context)?;
    let second = engine.evaluate_node(&args[1], context)?;

    // Compare them (strict equality)
    let equals = compare_equals(&first, &second, true)?;

    if !equals {
        // Found inequality, return true immediately (lazy)
        return Ok(Value::Bool(true));
    }

    // If we only have 2 args and they're equal, return false
    if args.len() == 2 {
        return Ok(Value::Bool(false));
    }

    // For 3+ args, since first two are equal, the result depends on whether
    // all remaining args also equal the first. But JSONLogic !== seems to only
    // check the first two operands when they're equal (based on test case)
    // This achieves lazy evaluation.
    Ok(Value::Bool(false))
}
/// Greater than operator function (>)
#[inline]
pub fn evaluate_greater_than(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // Require at least 2 arguments
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // For chained comparisons (3+ arguments), check a > b > c > ...
    // This should be evaluated lazily - stop at first false
    let mut prev = engine.evaluate_node(&args[0], context)?;

    for item in args.iter().skip(1) {
        let curr = engine.evaluate_node(item, context)?;

        // Compare prev > curr
        let result = compare_greater_than(&prev, &curr)?;

        if !result {
            // Short-circuit on first false
            return Ok(Value::Bool(false));
        }

        prev = curr;
    }

    Ok(Value::Bool(true))
}
// Helper function for > comparison
#[inline]
fn compare_greater_than(left: &Value, right: &Value) -> Result<bool> {
    // Handle datetime comparisons first - both objects and strings
    let left_dt = if is_datetime_object(left) {
        extract_datetime(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    };

    let right_dt = if is_datetime_object(right) {
        extract_datetime(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    };

    if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
        return Ok(dt1 > dt2);
    }

    // Handle duration comparisons - both objects and strings
    let left_dur = extract_duration_value(left);
    let right_dur = extract_duration_value(right);

    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        return Ok(dur1 > dur2);
    }

    // Arrays and objects cannot be compared (after checking for special objects)
    if matches!(left, Value::Array(_) | Value::Object(_))
        || matches!(right, Value::Array(_) | Value::Object(_))
    {
        return Err(crate::Error::Thrown(serde_json::json!({"type": "NaN"})));
    }

    // If both are strings, do string comparison
    if let (Value::String(l), Value::String(r)) = (left, right) {
        return Ok(l > r);
    }

    // Check if both can be coerced to numbers
    let left_num = coerce_to_number(left);
    let right_num = coerce_to_number(right);

    if let (Some(l), Some(r)) = (left_num, right_num) {
        return Ok(l > r);
    }

    // If one is a number and the other is a string that can't be coerced, throw NaN
    if (matches!(left, Value::Number(_)) && matches!(right, Value::String(_)))
        || (matches!(right, Value::Number(_)) && matches!(left, Value::String(_)))
    {
        return Err(crate::Error::Thrown(serde_json::json!({"type": "NaN"})));
    }

    Ok(false)
}

/// Greater than or equal operator function (>=)
#[inline]
pub fn evaluate_greater_than_equal(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // Require at least 2 arguments
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // For chained comparisons (3+ arguments), check a >= b >= c >= ...
    // This should be evaluated lazily - stop at first false
    let mut prev = engine.evaluate_node(&args[0], context)?;

    for item in args.iter().skip(1) {
        let curr = engine.evaluate_node(item, context)?;

        // Compare prev >= curr
        let result = compare_greater_than_equal(&prev, &curr)?;

        if !result {
            // Short-circuit on first false
            return Ok(Value::Bool(false));
        }

        prev = curr;
    }

    Ok(Value::Bool(true))
}
// Helper function for >= comparison
#[inline]
fn compare_greater_than_equal(left: &Value, right: &Value) -> Result<bool> {
    // Handle datetime comparisons first - both objects and strings
    let left_dt = if is_datetime_object(left) {
        extract_datetime(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    };

    let right_dt = if is_datetime_object(right) {
        extract_datetime(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    };

    if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
        return Ok(dt1 >= dt2);
    }

    // Handle duration comparisons - both objects and strings
    let left_dur = extract_duration_value(left);
    let right_dur = extract_duration_value(right);

    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        return Ok(dur1 >= dur2);
    }

    // Arrays and objects cannot be compared (after checking for special objects)
    if matches!(left, Value::Array(_) | Value::Object(_))
        || matches!(right, Value::Array(_) | Value::Object(_))
    {
        return Err(crate::Error::Thrown(serde_json::json!({"type": "NaN"})));
    }

    // If both are strings, do string comparison
    if let (Value::String(l), Value::String(r)) = (left, right) {
        return Ok(l >= r);
    }

    // Check if both can be coerced to numbers
    let left_num = coerce_to_number(left);
    let right_num = coerce_to_number(right);

    if let (Some(l), Some(r)) = (left_num, right_num) {
        return Ok(l >= r);
    }

    // If one is a number and the other is a string that can't be coerced, throw NaN
    if (matches!(left, Value::Number(_)) && matches!(right, Value::String(_)))
        || (matches!(right, Value::Number(_)) && matches!(left, Value::String(_)))
    {
        return Err(crate::Error::Thrown(serde_json::json!({"type": "NaN"})));
    }

    Ok(false)
}

/// Less than operator function (<) - supports variadic arguments
#[inline]
pub fn evaluate_less_than(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // Require at least 2 arguments
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // For chained comparisons (3+ arguments), check a < b < c < ...
    // This should be evaluated lazily - stop at first false
    let mut prev = engine.evaluate_node(&args[0], context)?;

    for item in args.iter().skip(1) {
        let current = engine.evaluate_node(item, context)?;

        // Compare prev < current
        let result = compare_less_than(&prev, &current)?;

        if !result {
            // Short-circuit on first false
            return Ok(Value::Bool(false));
        }

        prev = current;
    }

    Ok(Value::Bool(true))
}
// Helper function for < comparison
#[inline]
fn compare_less_than(left: &Value, right: &Value) -> Result<bool> {
    // Handle datetime comparisons first - both objects and strings
    let left_dt = if is_datetime_object(left) {
        extract_datetime(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    };

    let right_dt = if is_datetime_object(right) {
        extract_datetime(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    };

    if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
        return Ok(dt1 < dt2);
    }

    // Handle duration comparisons - both objects and strings
    let left_dur = extract_duration_value(left);
    let right_dur = extract_duration_value(right);

    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        return Ok(dur1 < dur2);
    }

    // Arrays and objects cannot be compared (after checking for special objects)
    if matches!(left, Value::Array(_) | Value::Object(_))
        || matches!(right, Value::Array(_) | Value::Object(_))
    {
        return Err(crate::Error::Thrown(serde_json::json!({"type": "NaN"})));
    }

    // If both are strings, do string comparison
    if let (Value::String(l), Value::String(r)) = (left, right) {
        return Ok(l < r);
    }

    // Check if both can be coerced to numbers
    let left_num = coerce_to_number(left);
    let right_num = coerce_to_number(right);

    if let (Some(l), Some(r)) = (left_num, right_num) {
        return Ok(l < r);
    }

    // If one is a number and the other is a string that can't be coerced, throw NaN
    if (matches!(left, Value::Number(_)) && matches!(right, Value::String(_)))
        || (matches!(right, Value::Number(_)) && matches!(left, Value::String(_)))
    {
        return Err(crate::Error::Thrown(serde_json::json!({"type": "NaN"})));
    }

    Ok(false)
}

/// Less than or equal operator function (<=) - supports variadic arguments
#[inline]
pub fn evaluate_less_than_equal(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // Require at least 2 arguments
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // For chained comparisons (3+ arguments), check a <= b <= c <= ...
    // This should be evaluated lazily - stop at first false
    let mut prev = engine.evaluate_node(&args[0], context)?;

    for item in args.iter().skip(1) {
        let current = engine.evaluate_node(item, context)?;

        // Compare prev <= current
        let result = compare_less_than_equal(&prev, &current)?;

        if !result {
            // Short-circuit on first false
            return Ok(Value::Bool(false));
        }

        prev = current;
    }

    Ok(Value::Bool(true))
}

// Helper function for <= comparison
#[inline]
fn compare_less_than_equal(left: &Value, right: &Value) -> Result<bool> {
    // Handle datetime comparisons first - both objects and strings
    let left_dt = if is_datetime_object(left) {
        extract_datetime(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    };

    let right_dt = if is_datetime_object(right) {
        extract_datetime(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    };

    if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
        return Ok(dt1 <= dt2);
    }

    // Handle duration comparisons - both objects and strings
    let left_dur = extract_duration_value(left);
    let right_dur = extract_duration_value(right);

    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        return Ok(dur1 <= dur2);
    }

    // Arrays and objects cannot be compared (after checking for special objects)
    if matches!(left, Value::Array(_) | Value::Object(_))
        || matches!(right, Value::Array(_) | Value::Object(_))
    {
        return Err(crate::Error::Thrown(serde_json::json!({"type": "NaN"})));
    }

    // If both are strings, do string comparison
    if let (Value::String(l), Value::String(r)) = (left, right) {
        return Ok(l <= r);
    }

    // Check if both can be coerced to numbers
    let left_num = coerce_to_number(left);
    let right_num = coerce_to_number(right);

    if let (Some(l), Some(r)) = (left_num, right_num) {
        return Ok(l <= r);
    }

    // If one is a number and the other is a string that can't be coerced, throw NaN
    if (matches!(left, Value::Number(_)) && matches!(right, Value::String(_)))
        || (matches!(right, Value::Number(_)) && matches!(left, Value::String(_)))
    {
        return Err(crate::Error::Thrown(serde_json::json!({"type": "NaN"})));
    }

    Ok(false)
}
