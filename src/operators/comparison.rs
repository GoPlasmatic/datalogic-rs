//! Comparison operators for value comparisons.
//!
//! This module provides equality and ordering comparison operators with support for
//! type coercion, datetime/duration handling, and chained comparisons.
//!
//! # Operators
//!
//! | Operator | Description | Example |
//! |----------|-------------|---------|
//! | `==` | Loose equality (with coercion) | `{"==": [1, "1"]}` → `true` |
//! | `===` | Strict equality (no coercion) | `{"===": [1, "1"]}` → `false` |
//! | `!=` | Loose inequality | `{"!=": [1, 2]}` → `true` |
//! | `!==` | Strict inequality | `{"!==": [1, "1"]}` → `true` |
//! | `>` | Greater than | `{">": [5, 3]}` → `true` |
//! | `>=` | Greater than or equal | `{">=": [5, 5]}` → `true` |
//! | `<` | Less than | `{"<": [3, 5]}` → `true` |
//! | `<=` | Less than or equal | `{"<=": [5, 5]}` → `true` |
//!
//! # Comparison Precedence
//!
//! When comparing values, the following precedence is used:
//!
//! 1. **DateTime**: If both values are parseable as ISO 8601 datetimes, compare chronologically
//! 2. **Duration**: If both values are parseable as durations, compare by total duration
//! 3. **String**: If both are strings (and not datetime/duration), compare lexicographically
//! 4. **Number**: Coerce to numbers and compare numerically
//!
//! # Chained Comparisons
//!
//! All comparison operators support chained comparisons with 3+ arguments:
//!
//! ```json
//! {"<": [1, 2, 3]}  // Equivalent to: 1 < 2 && 2 < 3, returns true
//! {"<": [1, 5, 3]}  // Equivalent to: 1 < 5 && 5 < 3, returns false
//! ```
//!
//! Chained comparisons use short-circuit evaluation - they stop at the first `false` result.
//!
//! # Error Handling
//!
//! Comparison throws a NaN error when:
//! - Comparing arrays or objects (except datetime/duration objects)
//! - Comparing a number with a non-numeric string

use serde_json::Value;

#[cfg(feature = "datetime")]
use super::helpers::{extract_datetime_value, extract_duration_value};
use crate::constants::INVALID_ARGS;
use crate::value_helpers::{coerce_to_number, loose_equals, strict_equals};
use crate::{CompiledNode, DataLogic, Result};

/// Returns true if a string could plausibly be a datetime or duration.
/// Filters out pure numeric strings and short strings that can't be either format.
#[cfg(feature = "datetime")]
#[inline]
fn could_be_datetime_or_duration(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 2 || !b[0].is_ascii_digit() {
        return false;
    }
    // Datetime: "YYYY-MM-DD..." requires '-' at position 4
    if b.len() >= 10 && b[4] == b'-' {
        return true;
    }
    // Duration: must contain a time-unit letter suffix (d/h/m/s)
    b.iter().any(|&c| matches!(c, b'd' | b'h' | b'm' | b's'))
}

// Helper function for == and === comparison
#[inline]
fn compare_equals(left: &Value, right: &Value, strict: bool, engine: &DataLogic) -> Result<bool> {
    // Fast path: same-type simple comparisons — skip datetime/duration entirely
    match (left, right) {
        (Value::Number(_), Value::Number(_))
        | (Value::Bool(_), Value::Bool(_))
        | (Value::Null, Value::Null) => {
            return if strict {
                Ok(strict_equals(left, right))
            } else {
                loose_equals(left, right, engine)
            };
        }
        // Two strings that can't be datetimes — skip extraction
        #[cfg(feature = "datetime")]
        (Value::String(l), Value::String(r))
            if !could_be_datetime_or_duration(l) || !could_be_datetime_or_duration(r) =>
        {
            return if strict {
                Ok(strict_equals(left, right))
            } else {
                loose_equals(left, right, engine)
            };
        }
        // Non-string primitives vs anything (except objects) — skip datetime extraction
        (Value::Number(_), _)
        | (_, Value::Number(_))
        | (Value::Bool(_), _)
        | (_, Value::Bool(_))
        | (Value::Null, _)
        | (_, Value::Null)
            if !matches!(left, Value::Object(_)) && !matches!(right, Value::Object(_)) =>
        {
            return if strict {
                Ok(strict_equals(left, right))
            } else {
                loose_equals(left, right, engine)
            };
        }
        _ => {}
    }

    #[cfg(feature = "datetime")]
    {
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
    }

    if strict {
        Ok(strict_equals(left, right))
    } else {
        loose_equals(left, right, engine)
    }
}

#[derive(Clone, Copy)]
enum OrdOp {
    Gt,
    Gte,
    Lt,
    Lte,
}

impl OrdOp {
    #[inline]
    fn apply_f64(self, l: f64, r: f64) -> bool {
        match self {
            OrdOp::Gt => l > r,
            OrdOp::Gte => l >= r,
            OrdOp::Lt => l < r,
            OrdOp::Lte => l <= r,
        }
    }

    #[inline]
    fn apply_str(self, l: &str, r: &str) -> bool {
        match self {
            OrdOp::Gt => l > r,
            OrdOp::Gte => l >= r,
            OrdOp::Lt => l < r,
            OrdOp::Lte => l <= r,
        }
    }

    #[cfg(feature = "datetime")]
    #[inline]
    fn apply_datetime(
        self,
        l: &crate::datetime::DataDateTime,
        r: &crate::datetime::DataDateTime,
    ) -> bool {
        match self {
            OrdOp::Gt => l > r,
            OrdOp::Gte => l >= r,
            OrdOp::Lt => l < r,
            OrdOp::Lte => l <= r,
        }
    }

    #[cfg(feature = "datetime")]
    #[inline]
    fn apply_duration(
        self,
        l: &crate::datetime::DataDuration,
        r: &crate::datetime::DataDuration,
    ) -> bool {
        match self {
            OrdOp::Gt => l > r,
            OrdOp::Gte => l >= r,
            OrdOp::Lt => l < r,
            OrdOp::Lte => l <= r,
        }
    }
}

/// Generic ordered comparison helper handling numbers, strings, datetimes, and durations.
#[inline]
fn compare_ordered(left: &Value, right: &Value, op: OrdOp, engine: &DataLogic) -> Result<bool> {
    // Fast path: both numbers — most common case
    if let (Value::Number(l), Value::Number(r)) = (left, right) {
        return Ok(op.apply_f64(
            l.as_f64().unwrap_or(f64::NAN),
            r.as_f64().unwrap_or(f64::NAN),
        ));
    }

    // Fast path: both strings that can't be datetimes — skip datetime parsing
    #[cfg(feature = "datetime")]
    if let (Value::String(l), Value::String(r)) = (left, right)
        && (!could_be_datetime_or_duration(l) || !could_be_datetime_or_duration(r))
    {
        return Ok(op.apply_str(l, r));
    }

    #[cfg(feature = "datetime")]
    {
        // Handle datetime comparisons first - both objects and strings
        let left_dt = extract_datetime_value(left);
        let right_dt = extract_datetime_value(right);

        if let (Some(dt1), Some(dt2)) = (&left_dt, &right_dt) {
            return Ok(op.apply_datetime(dt1, dt2));
        }

        // Handle duration comparisons - skip if already parsed as datetime (mutually exclusive)
        let left_dur = if left_dt.is_none() {
            extract_duration_value(left)
        } else {
            None
        };
        let right_dur = if right_dt.is_none() {
            extract_duration_value(right)
        } else {
            None
        };

        if let (Some(dur1), Some(dur2)) = (&left_dur, &right_dur) {
            return Ok(op.apply_duration(dur1, dur2));
        }
    }

    // Arrays and objects cannot be compared (after checking for special objects)
    if matches!(left, Value::Array(_) | Value::Object(_))
        || matches!(right, Value::Array(_) | Value::Object(_))
    {
        return Err(crate::constants::nan_error());
    }

    // If both are strings, do string comparison
    if let (Value::String(l), Value::String(r)) = (left, right) {
        return Ok(op.apply_str(l, r));
    }

    // Check if both can be coerced to numbers
    let left_num = coerce_to_number(left, engine);
    let right_num = coerce_to_number(right, engine);

    if let (Some(l), Some(r)) = (left_num, right_num) {
        return Ok(op.apply_f64(l, r));
    }

    // If one is a number and the other is a string that can't be coerced, throw NaN
    if (matches!(left, Value::Number(_)) && matches!(right, Value::String(_)))
        || (matches!(right, Value::Number(_)) && matches!(left, Value::String(_)))
    {
        return Err(crate::constants::nan_error());
    }

    Ok(false)
}

// =============================================================================
// Arena-mode comparison operators
// =============================================================================
//
// Each pre-evaluates args via `evaluate_arena_node` (so var-lookups borrow
// into input data via `InputRef` without cloning), materializes a
// `Cow<Value>` for the existing helpers, and returns a Bool from the
// preallocated singleton — zero arena allocation for the result.

use crate::arena::{ArenaContextStack, ArenaValue, arena_to_value_cow};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_strict_equals_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let first_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let first = arena_to_value_cow(first_av);
    for arg in &args[1..] {
        let cur_av = engine.evaluate_arena_node(arg, actx, arena)?;
        let cur = arena_to_value_cow(cur_av);
        if !compare_equals(&first, &cur, true, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_strict_not_equals_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let a = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let b = engine.evaluate_arena_node(&args[1], actx, arena)?;
    let eq = compare_equals(&arena_to_value_cow(a), &arena_to_value_cow(b), true, engine)?;
    Ok(crate::arena::pool::singleton_bool(!eq))
}

#[inline]
pub(crate) fn evaluate_equals_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let first_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let first = arena_to_value_cow(first_av);
    for arg in &args[1..] {
        let cur_av = engine.evaluate_arena_node(arg, actx, arena)?;
        let cur = arena_to_value_cow(cur_av);
        if !compare_equals(&first, &cur, false, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_not_equals_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let a = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let b = engine.evaluate_arena_node(&args[1], actx, arena)?;
    let eq = compare_equals(
        &arena_to_value_cow(a),
        &arena_to_value_cow(b),
        false,
        engine,
    )?;
    Ok(crate::arena::pool::singleton_bool(!eq))
}

#[inline]
fn evaluate_ord_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    op: OrdOp,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let mut prev_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let mut prev_cow = arena_to_value_cow(prev_av);
    for arg in &args[1..] {
        let cur_av = engine.evaluate_arena_node(arg, actx, arena)?;
        let cur_cow = arena_to_value_cow(cur_av);
        if !compare_ordered(&prev_cow, &cur_cow, op, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
        let _ = prev_av;
        prev_av = cur_av;
        prev_cow = cur_cow;
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_greater_than_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    evaluate_ord_arena(args, actx, engine, arena, OrdOp::Gt)
}

#[inline]
pub(crate) fn evaluate_greater_than_equal_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    evaluate_ord_arena(args, actx, engine, arena, OrdOp::Gte)
}

#[inline]
pub(crate) fn evaluate_less_than_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    evaluate_ord_arena(args, actx, engine, arena, OrdOp::Lt)
}

#[inline]
pub(crate) fn evaluate_less_than_equal_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    evaluate_ord_arena(args, actx, engine, arena, OrdOp::Lte)
}
