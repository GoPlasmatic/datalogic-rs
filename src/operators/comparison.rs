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

use crate::value_helpers::{loose_equals, strict_equals};
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

// =============================================================================
// Arena-mode comparison operators
// =============================================================================
//
// Equality and ordering are dispatched on `&ArenaValue` directly. Primitive
// operands take an arena-native fast path; only collection-vs-collection
// equality (rare) materializes once via `arena_to_value_cow`.

use crate::arena::{ArenaContextStack, ArenaValue, arena_to_value_cow, coerce_arena_to_number_cfg};
use bumpalo::Bump;

/// View an arena value as `&str` if it's a string variant.
#[inline]
fn arena_as_str<'a>(av: &'a ArenaValue<'a>) -> Option<&'a str> {
    match av {
        ArenaValue::String(s) => Some(*s),
        _ => None,
    }
}

/// Tagged primitive view of an `ArenaValue`. Returns `None` for collections
/// (Array/Object) and DateTime/Duration which need the slow path.
enum ArenaKind<'a> {
    Null,
    Bool(bool),
    Num(f64),
    Str(&'a str),
}

#[inline]
fn arena_kind<'a>(av: &'a ArenaValue<'a>) -> Option<ArenaKind<'a>> {
    match av {
        ArenaValue::Null => Some(ArenaKind::Null),
        ArenaValue::Bool(b) => Some(ArenaKind::Bool(*b)),
        ArenaValue::Number(n) => Some(ArenaKind::Num(n.as_f64())),
        ArenaValue::String(s) => Some(ArenaKind::Str(s)),
        _ => None,
    }
}

/// Arena-native equality. Mirrors `compare_equals` for primitive operands;
/// collections fall back to the legacy `Value`-based helper.
#[inline]
pub(crate) fn compare_equals_arena(
    left: &ArenaValue<'_>,
    right: &ArenaValue<'_>,
    strict: bool,
    engine: &DataLogic,
) -> Result<bool> {
    // Datetime / duration takes precedence on string/object operands.
    #[cfg(feature = "datetime")]
    {
        use crate::operators::helpers::{extract_datetime_arena, extract_duration_arena};
        let probe_dt = match (left, right) {
            (ArenaValue::Number(_) | ArenaValue::Bool(_) | ArenaValue::Null, _)
            | (_, ArenaValue::Number(_) | ArenaValue::Bool(_) | ArenaValue::Null) => false,
            (ArenaValue::String(s), _) | (_, ArenaValue::String(s))
                if !could_be_datetime_or_duration(s) =>
            {
                false
            }
            _ => true,
        };
        if probe_dt {
            let left_dt = extract_datetime_arena(left);
            let right_dt = extract_datetime_arena(right);
            if let (Some(dt1), Some(dt2)) = (&left_dt, &right_dt) {
                return Ok(dt1 == dt2);
            }
            let left_dur = extract_duration_arena(left);
            let right_dur = extract_duration_arena(right);
            if let (Some(dur1), Some(dur2)) = (&left_dur, &right_dur) {
                return Ok(dur1 == dur2);
            }
        }
    }

    // Primitive arena-native fast path. Returns `None` only when one side is
    // a collection or both can't be compared without value-mode coercion.
    if let Some(eq) = compare_equals_primitive(left, right, strict, engine) {
        return Ok(eq);
    }

    // Collection-vs-collection (or other unhandled combo) — fall back to
    // value-mode helper via a `Cow` materialization.
    let l = arena_to_value_cow(left);
    let r = arena_to_value_cow(right);
    if strict {
        Ok(strict_equals(&l, &r))
    } else {
        loose_equals(&l, &r, engine)
    }
}

/// Arena-native primitive equality. `Some(eq)` when both operands are
/// non-collection (Number/Bool/String/Null variants); `None` when either
/// side is a collection or when loose-coercion needs the value-mode path.
#[inline]
fn compare_equals_primitive(
    left: &ArenaValue<'_>,
    right: &ArenaValue<'_>,
    strict: bool,
    engine: &DataLogic,
) -> Option<bool> {
    let lk = arena_kind(left)?;
    let rk = arena_kind(right)?;
    use ArenaKind::*;
    match (lk, rk) {
        (Null, Null) => Some(true),
        (Bool(a), Bool(b)) => Some(a == b),
        (Str(a), Str(b)) => Some(a == b),
        (Num(a), Num(b)) => Some(a == b),
        _ if strict => Some(false),
        // Loose coercion table — mirrors `loose_equals_core` for primitive cases.
        (Num(n), Str(s)) | (Str(s), Num(n)) => match s.parse::<f64>().ok() {
            Some(sf) => Some(sf == n),
            // Defer to value-mode for Incompatible vs NotEqual semantics.
            None => None,
        },
        (Num(n), Bool(b)) | (Bool(b), Num(n)) => Some(n == if b { 1.0 } else { 0.0 }),
        (Str(s), Bool(b)) | (Bool(b), Str(s)) => Some(s == if b { "true" } else { "false" }),
        (Null, Num(n)) | (Num(n), Null) => {
            if engine.config().loose_equality_errors {
                None
            } else {
                Some(n == 0.0)
            }
        }
        (Null, Bool(b)) | (Bool(b), Null) => {
            if engine.config().loose_equality_errors {
                None
            } else {
                Some(!b)
            }
        }
        (Null, Str(s)) | (Str(s), Null) => {
            if engine.config().loose_equality_errors {
                None
            } else {
                Some(s.is_empty())
            }
        }
    }
}

/// Arena-native ordered comparison. Mirrors `compare_ordered` exactly.
#[inline]
fn compare_ordered_arena(
    left: &ArenaValue<'_>,
    right: &ArenaValue<'_>,
    op: OrdOp,
    engine: &DataLogic,
) -> Result<bool> {
    // Number vs Number — most common case.
    let l_is_num = matches!(left, ArenaValue::Number(_));
    let r_is_num = matches!(right, ArenaValue::Number(_));
    if l_is_num && r_is_num {
        let lf = left.as_f64().unwrap_or(f64::NAN);
        let rf = right.as_f64().unwrap_or(f64::NAN);
        return Ok(op.apply_f64(lf, rf));
    }

    // String vs String (non-datetime fast path).
    #[cfg(feature = "datetime")]
    if let (Some(l), Some(r)) = (arena_as_str(left), arena_as_str(right))
        && (!could_be_datetime_or_duration(l) || !could_be_datetime_or_duration(r))
    {
        return Ok(op.apply_str(l, r));
    }
    #[cfg(not(feature = "datetime"))]
    if let (Some(l), Some(r)) = (arena_as_str(left), arena_as_str(right)) {
        return Ok(op.apply_str(l, r));
    }

    #[cfg(feature = "datetime")]
    {
        use crate::operators::helpers::{extract_datetime_arena, extract_duration_arena};
        let left_dt = extract_datetime_arena(left);
        let right_dt = extract_datetime_arena(right);
        if let (Some(dt1), Some(dt2)) = (&left_dt, &right_dt) {
            return Ok(op.apply_datetime(dt1, dt2));
        }
        let left_dur = if left_dt.is_none() {
            extract_duration_arena(left)
        } else {
            None
        };
        let right_dur = if right_dt.is_none() {
            extract_duration_arena(right)
        } else {
            None
        };
        if let (Some(dur1), Some(dur2)) = (&left_dur, &right_dur) {
            return Ok(op.apply_duration(dur1, dur2));
        }
    }

    // Arrays / Objects can't be ordered.
    let is_collection =
        |av: &ArenaValue<'_>| matches!(av, ArenaValue::Array(_) | ArenaValue::Object(_));
    if is_collection(left) || is_collection(right) {
        return Err(crate::constants::nan_error());
    }

    // String vs String — datetime-shaped that fell through.
    if let (Some(l), Some(r)) = (arena_as_str(left), arena_as_str(right)) {
        return Ok(op.apply_str(l, r));
    }

    // Numeric coercion fallback.
    let l_num = coerce_arena_to_number_cfg(left, engine);
    let r_num = coerce_arena_to_number_cfg(right, engine);
    if let (Some(l), Some(r)) = (l_num, r_num) {
        return Ok(op.apply_f64(l, r));
    }

    // Number-String mismatch — NaN error.
    let is_str = |av: &ArenaValue<'_>| matches!(av, ArenaValue::String(_));
    if (l_is_num && is_str(right)) || (r_is_num && is_str(left)) {
        return Err(crate::constants::nan_error());
    }

    Ok(false)
}

#[inline]
pub(crate) fn evaluate_strict_equals_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let first_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.evaluate_arena_node(arg, actx, arena)?;
        if !compare_equals_arena(first_av, cur_av, true, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_strict_not_equals_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let a = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let b = engine.evaluate_arena_node(&args[1], actx, arena)?;
    let eq = compare_equals_arena(a, b, true, engine)?;
    Ok(crate::arena::pool::singleton_bool(!eq))
}

#[inline]
pub(crate) fn evaluate_equals_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let first_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.evaluate_arena_node(arg, actx, arena)?;
        if !compare_equals_arena(first_av, cur_av, false, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_not_equals_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let a = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let b = engine.evaluate_arena_node(&args[1], actx, arena)?;
    let eq = compare_equals_arena(a, b, false, engine)?;
    Ok(crate::arena::pool::singleton_bool(!eq))
}

#[inline]
fn evaluate_ord_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    op: OrdOp,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let mut prev_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.evaluate_arena_node(arg, actx, arena)?;
        if !compare_ordered_arena(prev_av, cur_av, op, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
        prev_av = cur_av;
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_greater_than_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    evaluate_ord_arena(args, actx, engine, arena, OrdOp::Gt)
}

#[inline]
pub(crate) fn evaluate_greater_than_equal_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    evaluate_ord_arena(args, actx, engine, arena, OrdOp::Gte)
}

#[inline]
pub(crate) fn evaluate_less_than_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    evaluate_ord_arena(args, actx, engine, arena, OrdOp::Lt)
}

#[inline]
pub(crate) fn evaluate_less_than_equal_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    evaluate_ord_arena(args, actx, engine, arena, OrdOp::Lte)
}
