//! Comparison operators for logic expressions.
//!
//! This module provides implementations for comparison operators
//! such as equal, not equal, greater than, etc.

use crate::arena::DataArena;
use crate::context::EvalContext;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;
use chrono::{DateTime, Duration, FixedOffset};

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
fn extract_datetime_for_comparison<'a>(
    value: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Option<&'a DateTime<FixedOffset>> {
    match value {
        DataValue::DateTime(dt) => Some(dt),
        DataValue::String(s) => {
            // Try to parse string as datetime and cache it in arena
            if let Ok(dt) = crate::value::parse_datetime(s) {
                Some(arena.alloc(dt))
            } else {
                None
            }
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
                .find(|(key, _)| *key == arena.alloc_str("timestamp"))
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

/// Validate that sufficient arguments are provided for a comparison operation
fn validate_arguments(args: &[&Token]) -> Result<()> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    Ok(())
}

/// Helper function to evaluate all pairs of arguments with a comparison function
fn evaluate_pairwise<'a, F>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
    comparator: F,
) -> Result<&'a DataValue<'a>>
where
    F: Fn(&'a DataValue<'a>, &'a DataValue<'a>) -> Result<bool>,
{
    validate_arguments(args)?;

    for i in 0..args.len() - 1 {
        let left = evaluate(args[i], context, arena)?;
        let right = evaluate(args[i + 1], context, arena)?;

        if !comparator(left, right)? {
            return Ok(arena.false_value());
        }
    }

    Ok(arena.true_value())
}

/// Helper for equality comparison between two values with type coercion
fn values_are_equal<'a>(
    left: &'a DataValue<'a>,
    right: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<bool> {
    // Fast path for identical references
    if std::ptr::eq(left as *const DataValue, right as *const DataValue) {
        return Ok(true);
    }

    // Try to extract datetime values
    let left_dt = extract_datetime_for_comparison(left, arena);
    let right_dt = extract_datetime_for_comparison(right, arena);

    // If both values are datetimes, compare them
    if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
        return Ok(left_dt == right_dt);
    }

    // Try to extract duration values
    let left_dur = extract_duration(left, arena);
    let right_dur = extract_duration(right, arena);

    // If both values are durations, compare them
    if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
        return Ok(left_dur == right_dur);
    }

    match (left, right) {
        (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() == b.as_f64()),
        (DataValue::String(a), DataValue::String(b)) => Ok(a == b),
        (DataValue::Bool(a), DataValue::Bool(b)) => Ok(a == b),
        (DataValue::Null, DataValue::Null) => Ok(true),
        (DataValue::Number(_), DataValue::String(s)) => {
            // Try to parse the string as a number
            if let Ok(num) = s.parse::<f64>() {
                let left_num = left.coerce_to_number().unwrap();
                Ok(left_num.as_f64() == num)
            } else {
                // String is not a valid number
                Err(LogicError::NaNError)
            }
        }
        (DataValue::String(s), DataValue::Number(_)) => {
            // Try to parse the string as a number
            if let Ok(num) = s.parse::<f64>() {
                let right_num = right.coerce_to_number().unwrap();
                Ok(num == right_num.as_f64())
            } else {
                // String is not a valid number
                Err(LogicError::NaNError)
            }
        }
        (DataValue::Array(_), DataValue::Array(_)) => {
            // Arrays should be compared by reference, not by value
            Err(LogicError::NaNError)
        }
        (DataValue::Array(_), _) | (_, DataValue::Array(_)) => {
            // Arrays can't be compared with non-arrays
            Err(LogicError::NaNError)
        }
        (DataValue::Object(_), _) | (_, DataValue::Object(_)) => {
            // Objects can't be compared with anything else
            // But we already handled the case where both are datetime objects above
            Err(LogicError::NaNError)
        }
        _ => {
            // Try numeric coercion for other cases
            if let (Some(a), Some(b)) = (left.coerce_to_number(), right.coerce_to_number()) {
                Ok(a.as_f64() == b.as_f64())
            } else {
                // If numeric coercion fails, fall back to string comparison
                let left_str = left.coerce_to_string(arena);
                let right_str = right.coerce_to_string(arena);

                if let (DataValue::String(a), DataValue::String(b)) = (&left_str, &right_str) {
                    Ok(a == b)
                } else {
                    Ok(false)
                }
            }
        }
    }
}

/// Helper for strict equality comparison between two values
fn values_are_strict_equal<'a>(left: &'a DataValue<'a>, right: &'a DataValue<'a>) -> Result<bool> {
    Ok(left.strict_equals(right))
}

/// Helper for not-equal comparison between two values with type coercion
fn values_are_not_equal<'a>(
    left: &'a DataValue<'a>,
    right: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<bool> {
    values_are_equal(left, right, arena).map(|result| !result)
}

/// Helper for strict not-equal comparison between two values
fn values_are_strict_not_equal<'a>(
    left: &'a DataValue<'a>,
    right: &'a DataValue<'a>,
) -> Result<bool> {
    values_are_strict_equal(left, right).map(|result| !result)
}

/// Helper for greater-than comparison between two values
fn value_is_greater_than<'a>(
    left: &'a DataValue<'a>,
    right: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<bool> {
    // Try to extract datetime values
    let left_dt = extract_datetime_for_comparison(left, arena);
    let right_dt = extract_datetime_for_comparison(right, arena);

    // If both values are datetimes, compare them
    if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
        return Ok(left_dt > right_dt);
    }

    // Try to extract duration values
    let left_dur = extract_duration(left, arena);
    let right_dur = extract_duration(right, arena);

    // If both values are durations, compare them
    if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
        return Ok(left_dur > right_dur);
    }

    match (left, right) {
        (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() > b.as_f64()),
        (DataValue::String(a), DataValue::String(b)) => Ok(a > b),
        (DataValue::Bool(a), DataValue::Bool(b)) => Ok(a > b),
        (DataValue::Null, DataValue::Null) => Ok(false),
        _ => {
            let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
            let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
            Ok(left_num.as_f64() > right_num.as_f64())
        }
    }
}

/// Helper for greater-than-or-equal comparison between two values
fn value_is_greater_than_or_equal<'a>(
    left: &'a DataValue<'a>,
    right: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<bool> {
    // Try to extract datetime values
    let left_dt = extract_datetime_for_comparison(left, arena);
    let right_dt = extract_datetime_for_comparison(right, arena);

    // If both values are datetimes, compare them
    if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
        return Ok(left_dt >= right_dt);
    }

    // Try to extract duration values
    let left_dur = extract_duration(left, arena);
    let right_dur = extract_duration(right, arena);

    // If both values are durations, compare them
    if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
        return Ok(left_dur >= right_dur);
    }

    match (left, right) {
        (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() >= b.as_f64()),
        (DataValue::String(a), DataValue::String(b)) => Ok(a >= b),
        (DataValue::Bool(a), DataValue::Bool(b)) => Ok(a >= b),
        (DataValue::Null, DataValue::Null) => Ok(true),
        _ => {
            let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
            let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
            Ok(left_num.as_f64() >= right_num.as_f64())
        }
    }
}

/// Helper for less-than comparison between two values
fn value_is_less_than<'a>(
    left: &'a DataValue<'a>,
    right: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<bool> {
    // Try to extract datetime values
    let left_dt = extract_datetime_for_comparison(left, arena);
    let right_dt = extract_datetime_for_comparison(right, arena);

    // If both values are datetimes, compare them
    if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
        return Ok(left_dt < right_dt);
    }

    // Try to extract duration values
    let left_dur = extract_duration(left, arena);
    let right_dur = extract_duration(right, arena);

    // If both values are durations, compare them
    if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
        return Ok(left_dur < right_dur);
    }

    match (left, right) {
        (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() < b.as_f64()),
        (DataValue::String(a), DataValue::String(b)) => Ok(a < b),
        (DataValue::Bool(a), DataValue::Bool(b)) => Ok(a < b),
        (DataValue::Null, DataValue::Null) => Ok(false),
        _ => {
            let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
            let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
            Ok(left_num.as_f64() < right_num.as_f64())
        }
    }
}

/// Helper for less-than-or-equal comparison between two values
fn value_is_less_than_or_equal<'a>(
    left: &'a DataValue<'a>,
    right: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<bool> {
    // Try to extract datetime values
    let left_dt = extract_datetime_for_comparison(left, arena);
    let right_dt = extract_datetime_for_comparison(right, arena);

    // If both values are datetimes, compare them
    if let (Some(left_dt), Some(right_dt)) = (left_dt, right_dt) {
        return Ok(left_dt <= right_dt);
    }

    // Try to extract duration values
    let left_dur = extract_duration(left, arena);
    let right_dur = extract_duration(right, arena);

    // If both values are durations, compare them
    if let (Some(left_dur), Some(right_dur)) = (left_dur, right_dur) {
        return Ok(left_dur <= right_dur);
    }

    match (left, right) {
        (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() <= b.as_f64()),
        (DataValue::String(a), DataValue::String(b)) => Ok(a <= b),
        (DataValue::Bool(a), DataValue::Bool(b)) => Ok(a <= b),
        (DataValue::Null, DataValue::Null) => Ok(true),
        _ => {
            let left_num = left.coerce_to_number().ok_or(LogicError::NaNError)?;
            let right_num = right.coerce_to_number().ok_or(LogicError::NaNError)?;
            Ok(left_num.as_f64() <= right_num.as_f64())
        }
    }
}

/// Evaluates an equality comparison.
pub fn eval_equal<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    evaluate_pairwise(args, context, arena, |left, right| {
        values_are_equal(left, right, arena)
    })
}

/// Evaluates a strict equality comparison.
pub fn eval_strict_equal<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    evaluate_pairwise(args, context, arena, |left, right| {
        values_are_strict_equal(left, right)
    })
}

/// Evaluates a not equal comparison.
pub fn eval_not_equal<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    evaluate_pairwise(args, context, arena, |left, right| {
        values_are_not_equal(left, right, arena)
    })
}

/// Evaluates a strict not-equal comparison.
pub fn eval_strict_not_equal<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    evaluate_pairwise(args, context, arena, |left, right| {
        values_are_strict_not_equal(left, right)
    })
}

/// Evaluates a greater-than comparison.
pub fn eval_greater_than<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    evaluate_pairwise(args, context, arena, |left, right| {
        value_is_greater_than(left, right, arena)
    })
}

/// Evaluates a greater-than-or-equal comparison.
pub fn eval_greater_than_or_equal<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    evaluate_pairwise(args, context, arena, |left, right| {
        value_is_greater_than_or_equal(left, right, arena)
    })
}

/// Evaluates a less-than comparison.
pub fn eval_less_than<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    evaluate_pairwise(args, context, arena, |left, right| {
        value_is_less_than(left, right, arena)
    })
}

/// Evaluates a less-than-or-equal comparison.
pub fn eval_less_than_or_equal<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    evaluate_pairwise(args, context, arena, |left, right| {
        value_is_less_than_or_equal(left, right, arena)
    })
}

#[cfg(test)]
mod tests {
    use crate::logic::Logic;
    use crate::logic::datalogic_core::DataLogicCore;
    use crate::logic::operators::comparison::ComparisonOp;
    use crate::logic::token::{OperatorType, Token};
    use crate::value::DataValue;
    use serde_json::json;

    #[test]
    fn test_equality() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"a": 10, "b": "10", "c": 20, "d": 10});

        // Test equal with same type
        // Create {"==": [{"var": "a"}, 10]}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let ten_token = Token::literal(DataValue::integer(10));
        let ten_ref = arena.alloc(ten_token);

        let equal_args = vec![a_var_ref, ten_ref];
        let equal_array_token = Token::ArrayLiteral(&equal_args);
        let equal_array_ref = arena.alloc(equal_array_token);

        let equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::Equal),
            equal_array_ref,
        );
        let equal_ref = arena.alloc(equal_token);

        let rule = Logic::new(equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test equal with different types (number and string)
        // Create {"==": [{"var": "a"}, {"var": "b"}]}
        let b_var_token = Token::variable("b", None);
        let b_var_ref = arena.alloc(b_var_token);

        let equal_args = vec![a_var_ref, b_var_ref];
        let equal_array_token = Token::ArrayLiteral(&equal_args);
        let equal_array_ref = arena.alloc(equal_array_token);

        let equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::Equal),
            equal_array_ref,
        );
        let equal_ref = arena.alloc(equal_token);

        let rule = Logic::new(equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test not equal
        // Create {"==": [{"var": "a"}, {"var": "c"}]}
        let c_var_token = Token::variable("c", None);
        let c_var_ref = arena.alloc(c_var_token);

        let equal_args = vec![a_var_ref, c_var_ref];
        let equal_array_token = Token::ArrayLiteral(&equal_args);
        let equal_array_ref = arena.alloc(equal_array_token);

        let equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::Equal),
            equal_array_ref,
        );
        let equal_ref = arena.alloc(equal_token);

        let rule = Logic::new(equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));

        // Test variadic equal (a = d = 10)
        // Create {"==": [{"var": "a"}, {"var": "d"}, 10]}
        let d_var_token = Token::variable("d", None);
        let d_var_ref = arena.alloc(d_var_token);

        let equal_args = vec![a_var_ref, d_var_ref, ten_ref];
        let equal_array_token = Token::ArrayLiteral(&equal_args);
        let equal_array_ref = arena.alloc(equal_array_token);

        let equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::Equal),
            equal_array_ref,
        );
        let equal_ref = arena.alloc(equal_token);

        let rule = Logic::new(equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test variadic equal failing (a = c = 10)
        // Create {"==": [{"var": "a"}, {"var": "c"}, 10]}
        let equal_args = vec![a_var_ref, c_var_ref, ten_ref];
        let equal_array_token = Token::ArrayLiteral(&equal_args);
        let equal_array_ref = arena.alloc(equal_array_token);

        let equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::Equal),
            equal_array_ref,
        );
        let equal_ref = arena.alloc(equal_token);

        let rule = Logic::new(equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_not_equal() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"a": 10, "b": "10", "c": 20, "d": 30});

        // Test not equal with two arguments
        // Create {"!=": [{"var": "a"}, {"var": "c"}]}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let c_var_token = Token::variable("c", None);
        let c_var_ref = arena.alloc(c_var_token);

        let not_equal_args = vec![a_var_ref, c_var_ref];
        let not_equal_array_token = Token::ArrayLiteral(&not_equal_args);
        let not_equal_array_ref = arena.alloc(not_equal_array_token);

        let not_equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::NotEqual),
            not_equal_array_ref,
        );
        let not_equal_ref = arena.alloc(not_equal_token);

        let rule = Logic::new(not_equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test not equal with same values
        // Create {"!=": [{"var": "a"}, 10]}
        let ten_token = Token::literal(DataValue::integer(10));
        let ten_ref = arena.alloc(ten_token);

        let not_equal_args = vec![a_var_ref, ten_ref];
        let not_equal_array_token = Token::ArrayLiteral(&not_equal_args);
        let not_equal_array_ref = arena.alloc(not_equal_array_token);

        let not_equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::NotEqual),
            not_equal_array_ref,
        );
        let not_equal_ref = arena.alloc(not_equal_token);

        let rule = Logic::new(not_equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));

        // Create multiple comparisons with AND (a != 10 && b != 10 && c != 10)
        // First, create the individual comparisons
        let b_var_token = Token::variable("b", None);
        let b_var_ref = arena.alloc(b_var_token);

        // {"!=": [{"var": "a"}, 10]}
        let ne1_args = vec![a_var_ref, ten_ref];
        let ne1_array_token = Token::ArrayLiteral(&ne1_args);
        let ne1_array_ref = arena.alloc(ne1_array_token);

        let ne1_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::NotEqual),
            ne1_array_ref,
        );
        let ne1_ref = arena.alloc(ne1_token);

        // {"!=": [{"var": "b"}, 10]}
        let ne2_args = vec![b_var_ref, ten_ref];
        let ne2_array_token = Token::ArrayLiteral(&ne2_args);
        let ne2_array_ref = arena.alloc(ne2_array_token);

        let ne2_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::NotEqual),
            ne2_array_ref,
        );
        let ne2_ref = arena.alloc(ne2_token);

        // {"!=": [{"var": "c"}, 10]}
        let ne3_args = vec![c_var_ref, ten_ref];
        let ne3_array_token = Token::ArrayLiteral(&ne3_args);
        let ne3_array_ref = arena.alloc(ne3_array_token);

        let ne3_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::NotEqual),
            ne3_array_ref,
        );
        let ne3_ref = arena.alloc(ne3_token);

        // Create {"and": [{"!=": [{"var": "a"}, 10]}, {"!=": [{"var": "b"}, 10]}, {"!=": [{"var": "c"}, 10]}]}
        let and_args = vec![ne1_ref, ne2_ref, ne3_ref];
        let and_array_token = Token::ArrayLiteral(&and_args);
        let and_array_ref = arena.alloc(and_array_token);

        let and_token = Token::operator(
            OperatorType::Control(crate::logic::operators::control::ControlOp::And),
            and_array_ref,
        );
        let and_ref = arena.alloc(and_token);

        let rule = Logic::new(and_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_strict_equal() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"a": 10, "b": "10", "c": 20});

        // Test strict equal with same type
        // Create: {"===": [{"var": "a"}, 10]}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let ten_token = Token::literal(DataValue::integer(10));
        let ten_ref = arena.alloc(ten_token);

        let equal_args = vec![a_var_ref, ten_ref];
        let equal_array_token = Token::ArrayLiteral(&equal_args);
        let equal_array_ref = arena.alloc(equal_array_token);

        let strict_equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::StrictEqual),
            equal_array_ref,
        );
        let strict_equal_ref = arena.alloc(strict_equal_token);

        let rule = Logic::new(strict_equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test strict equal with different types (number and string)
        // Create: {"===": [{"var": "a"}, {"var": "b"}]}
        let b_var_token = Token::variable("b", None);
        let b_var_ref = arena.alloc(b_var_token);

        let equal_args = vec![a_var_ref, b_var_ref];
        let equal_array_token = Token::ArrayLiteral(&equal_args);
        let equal_array_ref = arena.alloc(equal_array_token);

        let strict_equal_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::StrictEqual),
            equal_array_ref,
        );
        let strict_equal_ref = arena.alloc(strict_equal_token);

        let rule = Logic::new(strict_equal_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_greater_than() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"a": 10, "b": 5, "c": "20", "d": 30, "e": 3});

        // Test greater than with numbers
        // Create {">": [{"var": "a"}, {"var": "b"}]}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let b_var_token = Token::variable("b", None);
        let b_var_ref = arena.alloc(b_var_token);

        let gt_args = vec![a_var_ref, b_var_ref];
        let gt_array_token = Token::ArrayLiteral(&gt_args);
        let gt_array_ref = arena.alloc(gt_array_token);

        let gt_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::GreaterThan),
            gt_array_ref,
        );
        let gt_ref = arena.alloc(gt_token);

        let rule = Logic::new(gt_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test greater than with string coercion
        // Create {">": [{"var": "c"}, {"var": "a"}]}
        let c_var_token = Token::variable("c", None);
        let c_var_ref = arena.alloc(c_var_token);

        let gt_args = vec![c_var_ref, a_var_ref];
        let gt_array_token = Token::ArrayLiteral(&gt_args);
        let gt_array_ref = arena.alloc(gt_array_token);

        let gt_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::GreaterThan),
            gt_array_ref,
        );
        let gt_ref = arena.alloc(gt_token);

        let rule = Logic::new(gt_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test variadic greater than (d > a > b > e)
        // Create {">": [{"var": "d"}, {"var": "a"}, {"var": "b"}, {"var": "e"}]}
        let d_var_token = Token::variable("d", None);
        let d_var_ref = arena.alloc(d_var_token);

        let e_var_token = Token::variable("e", None);
        let e_var_ref = arena.alloc(e_var_token);

        let gt_args = vec![d_var_ref, a_var_ref, b_var_ref, e_var_ref];
        let gt_array_token = Token::ArrayLiteral(&gt_args);
        let gt_array_ref = arena.alloc(gt_array_token);

        let gt_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::GreaterThan),
            gt_array_ref,
        );
        let gt_ref = arena.alloc(gt_token);

        let rule = Logic::new(gt_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test variadic greater than (a > b > c)
        // Create {">": [{"var": "a"}, {"var": "b"}, {"var": "c"}]}
        let gt_args = vec![a_var_ref, b_var_ref, c_var_ref];
        let gt_array_token = Token::ArrayLiteral(&gt_args);
        let gt_array_ref = arena.alloc(gt_array_token);

        let gt_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::GreaterThan),
            gt_array_ref,
        );
        let gt_ref = arena.alloc(gt_token);

        let rule = Logic::new(gt_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_less_than() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"a": 10, "b": 5, "c": "20"});

        // Test less than with numbers
        // Create {"<": [{"var": "b"}, {"var": "a"}]}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let b_var_token = Token::variable("b", None);
        let b_var_ref = arena.alloc(b_var_token);

        let lt_args = vec![b_var_ref, a_var_ref];
        let lt_array_token = Token::ArrayLiteral(&lt_args);
        let lt_array_ref = arena.alloc(lt_array_token);

        let lt_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::LessThan),
            lt_array_ref,
        );
        let lt_ref = arena.alloc(lt_token);

        let rule = Logic::new(lt_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test less than with string coercion
        // Create {"<": [{"var": "a"}, {"var": "c"}]}
        let c_var_token = Token::variable("c", None);
        let c_var_ref = arena.alloc(c_var_token);

        let lt_args = vec![a_var_ref, c_var_ref];
        let lt_array_token = Token::ArrayLiteral(&lt_args);
        let lt_array_ref = arena.alloc(lt_array_token);

        let lt_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::LessThan),
            lt_array_ref,
        );
        let lt_ref = arena.alloc(lt_token);

        let rule = Logic::new(lt_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test variadic less than (b < a < c)
        // Create {"<": [{"var": "b"}, {"var": "a"}, {"var": "c"}]}
        let lt_args = vec![b_var_ref, a_var_ref, c_var_ref];
        let lt_array_token = Token::ArrayLiteral(&lt_args);
        let lt_array_ref = arena.alloc(lt_array_token);

        let lt_token = Token::operator(
            OperatorType::Comparison(ComparisonOp::LessThan),
            lt_array_ref,
        );
        let lt_ref = arena.alloc(lt_token);

        let rule = Logic::new(lt_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));
    }
}
