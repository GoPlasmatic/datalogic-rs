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

mod loose;

use crate::arena::{ContextStack, DataValue, coerce_to_number_cfg};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use loose::loose_equals;

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
    fn apply_datetime(self, l: &datavalue::DataDateTime, r: &datavalue::DataDateTime) -> bool {
        match self {
            OrdOp::Gt => l > r,
            OrdOp::Gte => l >= r,
            OrdOp::Lt => l < r,
            OrdOp::Lte => l <= r,
        }
    }

    #[cfg(feature = "datetime")]
    #[inline]
    fn apply_duration(self, l: &datavalue::DataDuration, r: &datavalue::DataDuration) -> bool {
        match self {
            OrdOp::Gt => l > r,
            OrdOp::Gte => l >= r,
            OrdOp::Lt => l < r,
            OrdOp::Lte => l <= r,
        }
    }
}

// =============================================================================
// Arena-mode comparison operators
// =============================================================================
//
// Equality and ordering are dispatched on `&DataValue` directly. Primitive
// operands take an arena-native fast path; collection-vs-collection equality
// falls through to `DataValue`'s `PartialEq` in the datavalue crate.

/// View an arena value as `&str` if it's a string variant.
#[inline]
fn value_as_str_in_op<'a>(av: &'a DataValue<'a>) -> Option<&'a str> {
    match av {
        DataValue::String(s) => Some(*s),
        _ => None,
    }
}

/// Arena-native equality. Loose mode goes through [`loose::loose_equals`];
/// strict mode is a direct [`PartialEq`] with one carve-out: numeric
/// variants compare as `f64` so `Integer(1) === Float(1.0)` is `true`.
#[inline]
pub(crate) fn compare_equals(
    left: &DataValue<'_>,
    right: &DataValue<'_>,
    strict: bool,
    engine: &Engine,
) -> Result<bool> {
    // Datetime / duration takes precedence on string/object operands.
    #[cfg(feature = "datetime")]
    {
        use crate::operators::datetime::{extract_datetime, extract_duration};
        let probe_dt = match (left, right) {
            (DataValue::Number(_) | DataValue::Bool(_) | DataValue::Null, _)
            | (_, DataValue::Number(_) | DataValue::Bool(_) | DataValue::Null) => false,
            (DataValue::String(s), _) | (_, DataValue::String(s))
                if !could_be_datetime_or_duration(s) =>
            {
                false
            }
            _ => true,
        };
        if probe_dt {
            let left_dt = extract_datetime(left);
            let right_dt = extract_datetime(right);
            if let (Some(dt1), Some(dt2)) = (&left_dt, &right_dt) {
                return Ok(dt1 == dt2);
            }
            let left_dur = extract_duration(left);
            let right_dur = extract_duration(right);
            if let (Some(dur1), Some(dur2)) = (&left_dur, &right_dur) {
                return Ok(dur1 == dur2);
            }
        }
    }

    if !strict {
        return loose_equals(left, right, engine);
    }

    // Strict: direct equality. Number variants compare as f64 so
    // `Integer(1) === Float(1.0)` is `true` (matches the legacy primitive
    // fast path; differs from the variant-aware `PartialEq` on `NumberValue`).
    if let (DataValue::Number(a), DataValue::Number(b)) = (left, right) {
        return Ok(a.as_f64() == b.as_f64());
    }
    Ok(left == right)
}

/// Arena-native ordered comparison. Mirrors `compare_ordered` exactly.
#[inline]
fn compare_ordered(
    left: &DataValue<'_>,
    right: &DataValue<'_>,
    op: OrdOp,
    engine: &Engine,
) -> Result<bool> {
    // Number vs Number — most common case. Both operands are guaranteed
    // numeric by the `matches!` guards, so `as_f64()` cannot return None
    // (every NumberValue variant converts losslessly to f64).
    if let (DataValue::Number(_), DataValue::Number(_)) = (left, right) {
        let lf = left
            .as_f64()
            .expect("DataValue::Number is always f64-convertible");
        let rf = right
            .as_f64()
            .expect("DataValue::Number is always f64-convertible");
        return Ok(op.apply_f64(lf, rf));
    }

    // String vs String (non-datetime fast path).
    #[cfg(feature = "datetime")]
    if let (Some(l), Some(r)) = (value_as_str_in_op(left), value_as_str_in_op(right)) {
        if !could_be_datetime_or_duration(l) || !could_be_datetime_or_duration(r) {
            return Ok(op.apply_str(l, r));
        }
    }
    #[cfg(not(feature = "datetime"))]
    if let (Some(l), Some(r)) = (value_as_str_in_op(left), value_as_str_in_op(right)) {
        return Ok(op.apply_str(l, r));
    }

    #[cfg(feature = "datetime")]
    {
        use crate::operators::datetime::{extract_datetime, extract_duration};
        let left_dt = extract_datetime(left);
        let right_dt = extract_datetime(right);
        if let (Some(dt1), Some(dt2)) = (&left_dt, &right_dt) {
            return Ok(op.apply_datetime(dt1, dt2));
        }
        let left_dur = if left_dt.is_none() {
            extract_duration(left)
        } else {
            None
        };
        let right_dur = if right_dt.is_none() {
            extract_duration(right)
        } else {
            None
        };
        if let (Some(dur1), Some(dur2)) = (&left_dur, &right_dur) {
            return Ok(op.apply_duration(dur1, dur2));
        }
    }

    // Arrays / Objects can't be ordered.
    let is_collection =
        |av: &DataValue<'_>| matches!(av, DataValue::Array(_) | DataValue::Object(_));
    if is_collection(left) || is_collection(right) {
        return Err(crate::Error::nan());
    }

    // String vs String — datetime-shaped that fell through.
    if let (Some(l), Some(r)) = (value_as_str_in_op(left), value_as_str_in_op(right)) {
        return Ok(op.apply_str(l, r));
    }

    // Numeric coercion fallback.
    let l_num = coerce_to_number_cfg(left, engine);
    let r_num = coerce_to_number_cfg(right, engine);
    if let (Some(l), Some(r)) = (l_num, r_num) {
        return Ok(op.apply_f64(l, r));
    }

    // Number-String mismatch — NaN error.
    let is_num = |av: &DataValue<'_>| matches!(av, DataValue::Number(_));
    let is_str = |av: &DataValue<'_>| matches!(av, DataValue::String(_));
    if (is_num(left) && is_str(right)) || (is_num(right) && is_str(left)) {
        return Err(crate::Error::nan());
    }

    Ok(false)
}

#[inline]
pub(crate) fn evaluate_strict_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let first_av = engine.dispatch_node(&args[0], ctx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.dispatch_node(arg, ctx, arena)?;
        if !compare_equals(first_av, cur_av, true, engine)? {
            return Ok(crate::arena::singletons::singleton_false());
        }
    }
    Ok(crate::arena::singletons::singleton_true())
}

#[inline]
pub(crate) fn evaluate_strict_not_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let a = engine.dispatch_node(&args[0], ctx, arena)?;
    let b = engine.dispatch_node(&args[1], ctx, arena)?;
    let eq = compare_equals(a, b, true, engine)?;
    Ok(crate::arena::singletons::singleton_bool(!eq))
}

#[inline]
pub(crate) fn evaluate_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let first_av = engine.dispatch_node(&args[0], ctx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.dispatch_node(arg, ctx, arena)?;
        if !compare_equals(first_av, cur_av, false, engine)? {
            return Ok(crate::arena::singletons::singleton_false());
        }
    }
    Ok(crate::arena::singletons::singleton_true())
}

#[inline]
pub(crate) fn evaluate_not_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let a = engine.dispatch_node(&args[0], ctx, arena)?;
    let b = engine.dispatch_node(&args[1], ctx, arena)?;
    let eq = compare_equals(a, b, false, engine)?;
    Ok(crate::arena::singletons::singleton_bool(!eq))
}

#[inline]
fn evaluate_ord<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: OrdOp,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let mut prev_av = engine.dispatch_node(&args[0], ctx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.dispatch_node(arg, ctx, arena)?;
        if !compare_ordered(prev_av, cur_av, op, engine)? {
            return Ok(crate::arena::singletons::singleton_false());
        }
        prev_av = cur_av;
    }
    Ok(crate::arena::singletons::singleton_true())
}

#[inline]
pub(crate) fn evaluate_greater_than<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    evaluate_ord(args, ctx, engine, arena, OrdOp::Gt)
}

#[inline]
pub(crate) fn evaluate_greater_than_equal<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    evaluate_ord(args, ctx, engine, arena, OrdOp::Gte)
}

#[inline]
pub(crate) fn evaluate_less_than<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    evaluate_ord(args, ctx, engine, arena, OrdOp::Lt)
}

#[inline]
pub(crate) fn evaluate_less_than_equal<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    evaluate_ord(args, ctx, engine, arena, OrdOp::Lte)
}
