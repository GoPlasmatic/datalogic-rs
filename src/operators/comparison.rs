use serde_json::Value;

use crate::datetime::{extract_datetime, extract_duration, is_datetime_object, is_duration_object};
use crate::value_helpers::{coerce_to_number, loose_equals_with_error, strict_equals};
use crate::{ContextStack, Evaluator, Operator, Result};

/// Equals operator (== for loose, === for strict)
pub struct EqualsOperator {
    pub strict: bool,
}

impl Operator for EqualsOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Err(crate::Error::InvalidArguments(
                "Invalid Arguments".to_string(),
            ));
        }

        // For chained equality (3+ arguments), check if all are equal
        let first = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

            // Compare first == current
            let result = compare_equals(&first, &current, self.strict)?;

            if !result {
                // Short-circuit on first inequality
                return Ok(Value::Bool(false));
            }
        }

        Ok(Value::Bool(true))
    }
}

// Helper function for == and === comparison
fn compare_equals(left: &Value, right: &Value, strict: bool) -> Result<bool> {
    // Handle datetime comparisons - both objects and strings
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
        return Ok(dt1 == dt2);
    }

    // Handle duration comparisons - both objects and strings
    let left_dur = if is_duration_object(left) {
        extract_duration(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

    let right_dur = if is_duration_object(right) {
        extract_duration(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

    if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
        return Ok(dur1 == dur2);
    }

    if strict {
        Ok(strict_equals(left, right))
    } else {
        loose_equals_with_error(left, right)
    }
}

/// Not equals operator (!= for loose, !== for strict)
pub struct NotEqualsOperator {
    pub strict: bool,
}

impl Operator for NotEqualsOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Err(crate::Error::InvalidArguments(
                "Invalid Arguments".to_string(),
            ));
        }

        // != returns true if arguments are not all equal
        // It's the logical negation of ==
        // But we need to handle lazy evaluation differently

        // Evaluate first two arguments
        let first = evaluator.evaluate(&args[0], context)?;
        let second = evaluator.evaluate(&args[1], context)?;

        // Compare them
        let equals = compare_equals(&first, &second, self.strict)?;

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
}

/// Greater than operator (>)
pub struct GreaterThanOperator;

impl Operator for GreaterThanOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Require at least 2 arguments
        if args.len() < 2 {
            return Err(crate::Error::InvalidArguments(
                "Invalid Arguments".to_string(),
            ));
        }

        // For chained comparisons (3+ arguments), check a > b > c > ...
        // This should be evaluated lazily - stop at first false
        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let curr = evaluator.evaluate(item, context)?;

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
}

// Helper function for > comparison
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
    let left_dur = if is_duration_object(left) {
        extract_duration(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

    let right_dur = if is_duration_object(right) {
        extract_duration(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

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

/// Greater than or equal operator (>=)
pub struct GreaterThanEqualOperator;

impl Operator for GreaterThanEqualOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Require at least 2 arguments
        if args.len() < 2 {
            return Err(crate::Error::InvalidArguments(
                "Invalid Arguments".to_string(),
            ));
        }

        // For chained comparisons (3+ arguments), check a >= b >= c >= ...
        // This should be evaluated lazily - stop at first false
        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let curr = evaluator.evaluate(item, context)?;

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
}

// Helper function for >= comparison
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
    let left_dur = if is_duration_object(left) {
        extract_duration(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

    let right_dur = if is_duration_object(right) {
        extract_duration(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

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

/// Less than operator (<) - supports variadic arguments
pub struct LessThanOperator;

impl Operator for LessThanOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Require at least 2 arguments
        if args.len() < 2 {
            return Err(crate::Error::InvalidArguments(
                "Invalid Arguments".to_string(),
            ));
        }

        // For chained comparisons (3+ arguments), check a < b < c < ...
        // This should be evaluated lazily - stop at first false
        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

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
}

// Helper function for < comparison
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
    let left_dur = if is_duration_object(left) {
        extract_duration(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

    let right_dur = if is_duration_object(right) {
        extract_duration(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

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

/// Less than or equal operator (<=) - supports variadic arguments
pub struct LessThanEqualOperator;

impl Operator for LessThanEqualOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Require at least 2 arguments
        if args.len() < 2 {
            return Err(crate::Error::InvalidArguments(
                "Invalid Arguments".to_string(),
            ));
        }

        // For chained comparisons (3+ arguments), check a <= b <= c <= ...
        // This should be evaluated lazily - stop at first false
        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

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
}

// Helper function for <= comparison
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
    let left_dur = if is_duration_object(left) {
        extract_duration(left)
    } else if let Value::String(s) = left {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

    let right_dur = if is_duration_object(right) {
        extract_duration(right)
    } else if let Value::String(s) = right {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    };

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
