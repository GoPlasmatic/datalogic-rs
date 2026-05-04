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

use crate::arena::{DataContextStack, DataValue, coerce_to_number_cfg};
use crate::constants::NAN_ERROR;
use crate::{CompiledNode, DataLogic, Error, Result};
use bumpalo::Bump;

// ─── Loose equality (==/!=) ──────────────────────────────────────────────────
//
// Reached from the comparison-arena collection-fallback path (rare — array-vs-
// array / object-vs-object) and from the primitive `==`/`!=` arms. Strict
// equality (`===`) compares values directly without going through here.
//
// Loose coercion table:
//
// | Left Type | Right Type | Behavior                       |
// |-----------|------------|--------------------------------|
// | Number    | String     | Parse string as number         |
// | Number    | Bool       | `true` → `1`, `false` → `0`    |
// | String    | Bool       | Compare to `"true"`/`"false"`  |
// | Null      | Number     | `null` equals `0`              |
// | Null      | Bool       | `null` equals `false`          |
// | Null      | String     | `null` equals `""`             |

enum LooseEqualsResult {
    Equal,
    NotEqual,
    Incompatible,
}

fn loose_equals_core(left: &DataValue<'_>, right: &DataValue<'_>) -> LooseEqualsResult {
    use LooseEqualsResult::*;

    match (left, right) {
        // Same-type cases
        (DataValue::Null, DataValue::Null) => Equal,
        (DataValue::Bool(a), DataValue::Bool(b)) => {
            if a == b {
                Equal
            } else {
                NotEqual
            }
        }
        (DataValue::String(a), DataValue::String(b)) => {
            if a == b {
                Equal
            } else {
                NotEqual
            }
        }
        (DataValue::Number(a), DataValue::Number(b)) => {
            let a_f = a.as_f64();
            let b_f = b.as_f64();
            if !a_f.is_nan() && !b_f.is_nan() && a_f == b_f {
                Equal
            } else {
                NotEqual
            }
        }

        // Number-String coercion
        (DataValue::Number(n), DataValue::String(s))
        | (DataValue::String(s), DataValue::Number(n)) => match s.parse::<f64>().ok() {
            Some(s_f) if n.as_f64() == s_f => Equal,
            Some(_) => NotEqual,
            None => Incompatible,
        },

        // Number-Bool coercion
        (DataValue::Number(n), DataValue::Bool(b)) | (DataValue::Bool(b), DataValue::Number(n)) => {
            if n.as_f64() == (if *b { 1.0 } else { 0.0 }) {
                Equal
            } else {
                NotEqual
            }
        }

        // String-Bool coercion
        (DataValue::String(s), DataValue::Bool(b)) | (DataValue::Bool(b), DataValue::String(s)) => {
            if *s == (if *b { "true" } else { "false" }) {
                Equal
            } else {
                NotEqual
            }
        }

        // Null coercions
        (DataValue::Null, DataValue::Number(n)) | (DataValue::Number(n), DataValue::Null) => {
            if n.as_f64() == 0.0 { Equal } else { NotEqual }
        }
        (DataValue::Null, DataValue::Bool(b)) | (DataValue::Bool(b), DataValue::Null) => {
            if !*b {
                Equal
            } else {
                NotEqual
            }
        }
        (DataValue::Null, DataValue::String(s)) | (DataValue::String(s), DataValue::Null) => {
            if s.is_empty() { Equal } else { NotEqual }
        }

        // Composite mixed with primitive: incompatible
        (DataValue::Array(_), _) | (_, DataValue::Array(_))
            if !matches!((left, right), (DataValue::Array(_), DataValue::Array(_))) =>
        {
            Incompatible
        }
        (DataValue::Object(_), _) | (_, DataValue::Object(_))
            if !matches!((left, right), (DataValue::Object(_), DataValue::Object(_))) =>
        {
            Incompatible
        }

        // Array-array structural compare
        (DataValue::Array(a), DataValue::Array(b)) => {
            if a == b {
                Equal
            } else {
                Incompatible
            }
        }

        _ => NotEqual,
    }
}

/// Compare two values with loose equality. When the engine config has
/// `loose_equality_errors` enabled, type-incompatible operands return an
/// error; otherwise they compare as not-equal.
fn loose_equals(left: &DataValue<'_>, right: &DataValue<'_>, engine: &DataLogic) -> Result<bool> {
    match loose_equals_core(left, right) {
        LooseEqualsResult::Equal => Ok(true),
        LooseEqualsResult::NotEqual => Ok(false),
        LooseEqualsResult::Incompatible => {
            if engine.config().loose_equality_errors {
                Err(Error::invalid_arguments(NAN_ERROR))
            } else {
                Ok(false)
            }
        }
    }
}

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

/// Tagged primitive view of an `DataValue`. Returns `None` for collections
/// (Array/Object) and DateTime/Duration which need the slow path.
enum PrimKind<'a> {
    Null,
    Bool(bool),
    Num(f64),
    Str(&'a str),
}

#[inline]
fn kind_of<'a>(av: &'a DataValue<'a>) -> Option<PrimKind<'a>> {
    match av {
        DataValue::Null => Some(PrimKind::Null),
        DataValue::Bool(b) => Some(PrimKind::Bool(*b)),
        DataValue::Number(n) => Some(PrimKind::Num(n.as_f64())),
        DataValue::String(s) => Some(PrimKind::Str(s)),
        _ => None,
    }
}

/// Arena-native equality. Mirrors `compare_equals` for primitive operands;
/// collections fall back to the legacy `Value`-based helper.
#[inline]
pub(crate) fn compare_equals(
    left: &DataValue<'_>,
    right: &DataValue<'_>,
    strict: bool,
    engine: &DataLogic,
) -> Result<bool> {
    // Datetime / duration takes precedence on string/object operands.
    #[cfg(feature = "datetime")]
    {
        use crate::operators::helpers::{extract_datetime, extract_duration};
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

    // Primitive arena-native fast path. Returns `None` only when one side is
    // a collection or both can't be compared without value-mode coercion.
    if let Some(eq) = compare_equals_primitive(left, right, strict, engine) {
        return Ok(eq);
    }

    // Collection-vs-collection (or other unhandled combo) — fall back to
    // the DataValue-based helpers.
    if strict {
        Ok(left == right)
    } else {
        loose_equals(left, right, engine)
    }
}

/// Arena-native primitive equality. `Some(eq)` when both operands are
/// non-collection (Number/Bool/String/Null variants); `None` when either
/// side is a collection or when loose-coercion needs the value-mode path.
#[inline]
fn compare_equals_primitive(
    left: &DataValue<'_>,
    right: &DataValue<'_>,
    strict: bool,
    engine: &DataLogic,
) -> Option<bool> {
    let lk = kind_of(left)?;
    let rk = kind_of(right)?;
    use PrimKind::*;
    match (lk, rk) {
        (Null, Null) => Some(true),
        (Bool(a), Bool(b)) => Some(a == b),
        (Str(a), Str(b)) => Some(a == b),
        (Num(a), Num(b)) => Some(a == b),
        _ if strict => Some(false),
        // Loose coercion table — mirrors `loose_equals_core` for primitive cases.
        // `None` from `s.parse()` defers to value-mode for Incompatible vs
        // NotEqual semantics.
        (Num(n), Str(s)) | (Str(s), Num(n)) => s.parse::<f64>().ok().map(|sf| sf == n),
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
fn compare_ordered(
    left: &DataValue<'_>,
    right: &DataValue<'_>,
    op: OrdOp,
    engine: &DataLogic,
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
    if let (Some(l), Some(r)) = (value_as_str_in_op(left), value_as_str_in_op(right))
        && (!could_be_datetime_or_duration(l) || !could_be_datetime_or_duration(r))
    {
        return Ok(op.apply_str(l, r));
    }
    #[cfg(not(feature = "datetime"))]
    if let (Some(l), Some(r)) = (value_as_str_in_op(left), value_as_str_in_op(right)) {
        return Ok(op.apply_str(l, r));
    }

    #[cfg(feature = "datetime")]
    {
        use crate::operators::helpers::{extract_datetime, extract_duration};
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
        return Err(crate::constants::nan_error());
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
        return Err(crate::constants::nan_error());
    }

    Ok(false)
}

#[inline]
pub(crate) fn evaluate_strict_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let first_av = engine.evaluate_node(&args[0], ctx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.evaluate_node(arg, ctx, arena)?;
        if !compare_equals(first_av, cur_av, true, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_strict_not_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let a = engine.evaluate_node(&args[0], ctx, arena)?;
    let b = engine.evaluate_node(&args[1], ctx, arena)?;
    let eq = compare_equals(a, b, true, engine)?;
    Ok(crate::arena::pool::singleton_bool(!eq))
}

#[inline]
pub(crate) fn evaluate_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let first_av = engine.evaluate_node(&args[0], ctx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.evaluate_node(arg, ctx, arena)?;
        if !compare_equals(first_av, cur_av, false, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_not_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let a = engine.evaluate_node(&args[0], ctx, arena)?;
    let b = engine.evaluate_node(&args[1], ctx, arena)?;
    let eq = compare_equals(a, b, false, engine)?;
    Ok(crate::arena::pool::singleton_bool(!eq))
}

#[inline]
fn evaluate_ord<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    op: OrdOp,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let mut prev_av = engine.evaluate_node(&args[0], ctx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.evaluate_node(arg, ctx, arena)?;
        if !compare_ordered(prev_av, cur_av, op, engine)? {
            return Ok(crate::arena::pool::singleton_false());
        }
        prev_av = cur_av;
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
pub(crate) fn evaluate_greater_than<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    evaluate_ord(args, ctx, engine, arena, OrdOp::Gt)
}

#[inline]
pub(crate) fn evaluate_greater_than_equal<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    evaluate_ord(args, ctx, engine, arena, OrdOp::Gte)
}

#[inline]
pub(crate) fn evaluate_less_than<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    evaluate_ord(args, ctx, engine, arena, OrdOp::Lt)
}

#[inline]
pub(crate) fn evaluate_less_than_equal<'a>(
    args: &'a [CompiledNode],
    ctx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    evaluate_ord(args, ctx, engine, arena, OrdOp::Lte)
}
