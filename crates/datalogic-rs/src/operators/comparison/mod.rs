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

/// Fractional-second digit count of a string in the strict normalized
/// ISO 8601 shape `YYYY-MM-DDTHH:MM:SS[.<1-9 digits>][Z | ±HH:MM]`:
/// uppercase `T`/`Z`, `-`/`:` separators at fixed positions, every field a
/// fixed-width run of ASCII digits. Character classes only; field *values*
/// are deliberately not range-checked (see [`iso_byte_compare_eligible`]).
///
/// Returns `None` for anything else: date-only, space or lowercase-`t`
/// separator, lowercase `z`, week/ordinal dates, colon-less offsets, more
/// than 9 fractional digits (chrono truncates past nanoseconds, so byte
/// order would diverge from instant order), or trailing garbage.
#[cfg(feature = "datetime")]
#[inline]
fn iso_datetime_shape(s: &str) -> Option<u8> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    // Core "YYYY-MM-DDTHH:MM:SS". The fixed-size reborrow erases the bounds
    // checks and the unrolled `&&` chain vectorizes; this scan sits ahead of
    // every datetime-shaped comparison, so it has to stay a few ns.
    let c: &[u8; 19] = b[..19].try_into().ok()?;
    let seps_ok = c[4] == b'-' && c[7] == b'-' && c[10] == b'T' && c[13] == b':' && c[16] == b':';
    let digits_ok = c[0].is_ascii_digit()
        && c[1].is_ascii_digit()
        && c[2].is_ascii_digit()
        && c[3].is_ascii_digit()
        && c[5].is_ascii_digit()
        && c[6].is_ascii_digit()
        && c[8].is_ascii_digit()
        && c[9].is_ascii_digit()
        && c[11].is_ascii_digit()
        && c[12].is_ascii_digit()
        && c[14].is_ascii_digit()
        && c[15].is_ascii_digit()
        && c[17].is_ascii_digit()
        && c[18].is_ascii_digit();
    if !(seps_ok && digits_ok) {
        return None;
    }
    // Optional fraction: '.' followed by 1..=9 digits.
    let mut i = 19;
    let mut frac_len = 0usize;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_len = i - start;
        if frac_len == 0 || frac_len > 9 {
            return None;
        }
    }
    // Optional timezone designator: nothing (naive), 'Z', or "±HH:MM".
    match b.len() - i {
        0 => {}
        1 if b[i] == b'Z' => {}
        6 if (b[i] == b'+' || b[i] == b'-')
            && b[i + 1].is_ascii_digit()
            && b[i + 2].is_ascii_digit()
            && b[i + 3] == b':'
            && b[i + 4].is_ascii_digit()
            && b[i + 5].is_ascii_digit() => {}
        _ => return None,
    }
    Some(frac_len as u8)
}

/// True when byte-wise comparison of `l` and `r` is guaranteed to return the
/// same verdict as the parse-based datetime comparison, so parsing can be
/// skipped entirely.
///
/// # Invariant (fast-path equivalence)
///
/// Both strings match the strict shape checked by [`iso_datetime_shape`]
/// with the **same** fractional-digit count and a **byte-identical**
/// timezone designator (both absent, both `Z`, or the same `±HH:MM`). Then:
///
/// 1. **Both parse** (`DataDateTime::parse` succeeds on each): the parse
///    path compares UTC instants. With an identical fixed offset, instant
///    order equals (year, month, day, hour, minute, second, fraction)
///    tuple order. Every field is fixed-width, zero-padded, at the same
///    byte position in both strings, ordered most-significant first, so
///    tuple order is exactly byte order. chrono's leap-second folding
///    (`:60` → second 59 + ≥1e9 ns) keeps the field→instant map strictly
///    monotonic and injective, and ≤ 9 fractional digits parse without
///    truncation, so byte equality is also instant equality.
/// 2. **At least one fails to parse** (e.g. month `13`): the existing path
///    finds no datetime, and no duration either (the shape admits no
///    `d`/`h`/`m`/`s` unit letters), and falls through to plain string
///    ordering / string equality, which is byte order again.
///
/// Everything outside the strict shape falls back to the parse path:
/// differing offsets, offset vs `Z`, naive vs `Z`, differing fractional
/// precision (`.5` vs `.50`), lowercase `t`/`z`, space separators,
/// date-only strings, week/ordinal dates, > 9 fractional digits.
#[cfg(feature = "datetime")]
#[inline]
fn iso_byte_compare_eligible(l: &str, r: &str) -> bool {
    // Identical shape implies identical length; checking it first makes the
    // common mismatched-shape fallback (e.g. `Z` vs `+02:00`) near-free.
    if l.len() != r.len() {
        return false;
    }
    let Some(frac_l) = iso_datetime_shape(l) else {
        return false;
    };
    let Some(frac_r) = iso_datetime_shape(r) else {
        return false;
    };
    // Equal lengths still allow differing splits (e.g. 8-digit fraction
    // plus `Z` vs 9-digit fraction, naive), so the counts must match too.
    if frac_l != frac_r {
        return false;
    }
    // Identical designator, byte-for-byte. Equal fraction lengths put the
    // designator at the same offset in both strings.
    let tz_start = 19 + if frac_l > 0 { 1 + frac_l as usize } else { 0 };
    l.as_bytes()[tz_start..] == r.as_bytes()[tz_start..]
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
            // Fast path: strings in the strict ISO shape with identical
            // designator and precision are temporally equal iff byte-equal;
            // skip parsing. See `iso_byte_compare_eligible` for the invariant.
            if let (DataValue::String(l), DataValue::String(r)) = (left, right) {
                if iso_byte_compare_eligible(l, r) {
                    return Ok(l == r);
                }
            }
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

/// Arena-native ordered comparison (`<`, `<=`, `>`, `>=`).
#[inline]
fn compare_ordered(
    left: &DataValue<'_>,
    right: &DataValue<'_>,
    op: OrdOp,
    engine: &Engine,
) -> Result<bool> {
    // Number vs Number — most common case. Bind the `NumberValue`s and use
    // the infallible `NumberValue::as_f64` (every variant converts losslessly
    // to f64), matching `compare_equals` and avoiding the `Option` + `.expect()`
    // panic-path codegen of the `DataValue::as_f64` round-trip.
    if let (DataValue::Number(a), DataValue::Number(b)) = (left, right) {
        return Ok(op.apply_f64(a.as_f64(), b.as_f64()));
    }

    // String vs String (non-datetime fast path). Datetime-shaped operands
    // also compare byte-wise when the strict-ISO gate proves byte order
    // equals the parse path's verdict; see `iso_byte_compare_eligible`.
    #[cfg(feature = "datetime")]
    if let (Some(l), Some(r)) = (value_as_str_in_op(left), value_as_str_in_op(right)) {
        if !could_be_datetime_or_duration(l)
            || !could_be_datetime_or_duration(r)
            || iso_byte_compare_eligible(l, r)
        {
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

/// Chained equality (`==` / `===`): every arg must equal the first.
/// `strict` selects strict vs loose `compare_equals`.
#[inline]
fn equals_chain<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    strict: bool,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let first_av = engine.dispatch_node(&args[0], ctx, arena)?;
    for arg in &args[1..] {
        let cur_av = engine.dispatch_node(arg, ctx, arena)?;
        if !compare_equals(first_av, cur_av, strict, engine)? {
            return Ok(crate::arena::singletons::singleton_false());
        }
    }
    Ok(crate::arena::singletons::singleton_true())
}

/// Pairwise inequality (`!=` / `!==`) on the first two args. `strict`
/// selects strict vs loose `compare_equals`.
#[inline]
fn not_equals_pair<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    strict: bool,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let a = engine.dispatch_node(&args[0], ctx, arena)?;
    let b = engine.dispatch_node(&args[1], ctx, arena)?;
    let eq = compare_equals(a, b, strict, engine)?;
    Ok(crate::arena::singletons::singleton_bool(!eq))
}

#[inline]
pub(crate) fn evaluate_strict_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    equals_chain(args, ctx, engine, arena, true)
}

#[inline]
pub(crate) fn evaluate_strict_not_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    not_equals_pair(args, ctx, engine, arena, true)
}

#[inline]
pub(crate) fn evaluate_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    equals_chain(args, ctx, engine, arena, false)
}

#[inline]
pub(crate) fn evaluate_not_equals<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    not_equals_pair(args, ctx, engine, arena, false)
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

#[cfg(all(test, feature = "datetime"))]
mod iso_fastpath_tests {
    use super::*;
    use crate::operators::datetime::{extract_datetime, extract_duration};

    const ORD_OPS: [OrdOp; 4] = [OrdOp::Gt, OrdOp::Gte, OrdOp::Lt, OrdOp::Lte];

    // ---- Gate classification ----

    #[test]
    fn shape_accepts_strict_iso_forms() {
        assert_eq!(iso_datetime_shape("2024-01-15T10:30:00Z"), Some(0));
        assert_eq!(iso_datetime_shape("2024-01-15T10:30:00"), Some(0));
        assert_eq!(iso_datetime_shape("2024-01-15T10:30:00.5Z"), Some(1));
        assert_eq!(iso_datetime_shape("2024-01-15T10:30:00.123Z"), Some(3));
        assert_eq!(
            iso_datetime_shape("2024-01-15T10:30:00.123456789+05:30"),
            Some(9)
        );
        assert_eq!(iso_datetime_shape("2024-01-15T10:30:00-00:00"), Some(0));
        // Shape only: calendar-invalid values still match (both fail to
        // parse, and the parse path then compares lexicographically anyway).
        assert_eq!(iso_datetime_shape("2024-99-99T99:99:99Z"), Some(0));
    }

    #[test]
    fn shape_rejects_non_strict_forms() {
        for s in [
            "2024-01-15",                      // date-only
            "2024-01-15t10:30:00z",            // lowercase designators
            "2024-01-15 10:30:00Z",            // space separator
            "2024-01-15T10:30:00.1234567890Z", // 10 fractional digits
            "2024-01-15T10:30:00.Z",           // empty fraction
            "2024-01-15T10:30:00+0530",        // colon-less offset
            "2024-01-15T10:30:00+05:3",        // truncated offset
            "2024-W03-1T10:00:00Z",            // week date
            "2024-015T10:00:00Z",              // ordinal date
            "2024-01-15T10:30:00ZZ",           // trailing garbage
            "2024-01-15T10:30",                // missing seconds
            "20240115T103000Z",                // basic format
        ] {
            assert_eq!(iso_datetime_shape(s), None, "should reject {s:?}");
        }
    }

    #[test]
    fn eligibility_requires_identical_designator_and_precision() {
        // Eligible: same designator, same precision.
        for (l, r) in [
            ("2024-01-15T10:30:00Z", "2025-06-01T08:00:00Z"),
            ("2024-01-15T10:30:00", "2025-06-01T08:00:00"),
            ("2024-01-15T10:30:00+05:30", "2025-06-01T08:00:00+05:30"),
            ("2024-01-15T10:30:00.123Z", "2025-06-01T08:00:00.456Z"),
        ] {
            assert!(iso_byte_compare_eligible(l, r), "{l:?} vs {r:?}");
        }
        // Not eligible: anything that could change the verdict.
        for (l, r) in [
            ("2024-01-15T10:30:00Z", "2024-01-15T10:30:00"), // Z vs naive
            ("2024-01-15T12:00:00+02:00", "2024-01-15T11:00:00Z"), // offset vs Z
            ("2024-01-15T10:30:00+05:30", "2024-01-15T10:30:00-05:30"), // differing offsets
            ("2024-01-15T10:30:00.5Z", "2024-01-15T10:30:00.50Z"), // differing precision
            ("2024-01-15T10:30:00Z", "2024-01-15T10:30:00.0Z"), // none vs some fraction
            ("2024-01-15T10:30:00Z", "2024-01-15t10:30:00z"), // lowercase
            ("2024-01-15T10:30:00Z", "2024-01-15"),          // datetime vs date-only
        ] {
            assert!(!iso_byte_compare_eligible(l, r), "{l:?} vs {r:?}");
        }
    }

    // ---- Verdict equivalence with the parse path ----

    /// Reference verdict: what the pre-fast-path code computes for two
    /// datetime-shaped strings, temporal when both parse and lexicographic
    /// otherwise. (Duration extraction never fires for the corpora here;
    /// asserted in `assert_matches_parse_path`.)
    fn parse_verdict_ord(l: &str, r: &str, op: OrdOp) -> bool {
        match (
            extract_datetime(&DataValue::String(l)),
            extract_datetime(&DataValue::String(r)),
        ) {
            (Some(a), Some(b)) => op.apply_datetime(&a, &b),
            _ => op.apply_str(l, r),
        }
    }

    fn parse_verdict_eq(l: &str, r: &str) -> bool {
        match (
            extract_datetime(&DataValue::String(l)),
            extract_datetime(&DataValue::String(r)),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => l == r,
        }
    }

    fn assert_matches_parse_path(engine: &Engine, l: &str, r: &str) {
        // Corpus sanity: these shapes must never parse as durations, or the
        // reference above would diverge from the real fallback chain.
        assert!(extract_duration(&DataValue::String(l)).is_none());
        assert!(extract_duration(&DataValue::String(r)).is_none());

        let lv = DataValue::String(l);
        let rv = DataValue::String(r);
        for op in ORD_OPS {
            assert_eq!(
                compare_ordered(&lv, &rv, op, engine).unwrap(),
                parse_verdict_ord(l, r, op),
                "ordered mismatch for {l:?} vs {r:?}"
            );
        }
        for strict in [false, true] {
            assert_eq!(
                compare_equals(&lv, &rv, strict, engine).unwrap(),
                parse_verdict_eq(l, r),
                "equality mismatch for {l:?} vs {r:?} (strict={strict})"
            );
        }
    }

    #[test]
    fn in_shape_pairs_compare_correctly() {
        let engine = Engine::new();
        let lt = |l: &str, r: &str| {
            let (lv, rv) = (DataValue::String(l), DataValue::String(r));
            compare_ordered(&lv, &rv, OrdOp::Lt, &engine).unwrap()
        };
        let eq = |l: &str, r: &str| {
            let (lv, rv) = (DataValue::String(l), DataValue::String(r));
            compare_equals(&lv, &rv, false, &engine).unwrap()
        };

        // Equal / less / greater on the plain Z form.
        assert!(eq("2024-01-15T10:30:00Z", "2024-01-15T10:30:00Z"));
        assert!(lt("2024-01-15T10:30:00Z", "2024-06-01T08:00:00Z"));
        assert!(!lt("2024-06-01T08:00:00Z", "2024-01-15T10:30:00Z"));
        // Fractional seconds.
        assert!(lt("2024-01-15T10:30:00.123Z", "2024-01-15T10:30:00.124Z"));
        assert!(eq("2024-01-15T10:30:00.123Z", "2024-01-15T10:30:00.123Z"));
        // Naive pair.
        assert!(lt("2024-01-15T10:30:00", "2024-01-15T11:30:00"));
        // Identical explicit offsets.
        assert!(lt("2024-01-15T10:30:00+05:30", "2024-01-15T11:30:00+05:30"));
        // Shape-valid but calendar-invalid pair: both fail to parse, both
        // paths compare lexicographically.
        assert!(!lt("2024-99-99T99:99:99Z", "2024-98-99T99:99:99Z"));
        // Mixed valid/invalid: parse path degrades to lexicographic too.
        assert!(!lt("2024-13-01T00:00:00Z", "2024-02-01T00:00:00Z"));
    }

    /// Pairs whose byte order and temporal order *disagree*; passing these
    /// proves the gate excluded them and the parse path decided the verdict.
    #[test]
    fn out_of_shape_pairs_fall_back_to_parse_path() {
        let engine = Engine::new();
        let lt = |l: &str, r: &str| {
            let (lv, rv) = (DataValue::String(l), DataValue::String(r));
            compare_ordered(&lv, &rv, OrdOp::Lt, &engine).unwrap()
        };
        let eq = |l: &str, r: &str| {
            let (lv, rv) = (DataValue::String(l), DataValue::String(r));
            compare_equals(&lv, &rv, false, &engine).unwrap()
        };

        // Mixed offsets: byte order says Greater, instants say Less.
        assert!(lt("2024-01-15T12:00:00+02:00", "2024-01-15T11:00:00Z"));
        // Differing fractional precision: byte-unequal, temporally equal.
        assert!(eq("2024-01-15T10:30:00.5Z", "2024-01-15T10:30:00.50Z"));
        assert!(eq("2024-01-15T10:30:00Z", "2024-01-15T10:30:00.0Z"));
        // Naive vs Z: byte-unequal, temporally equal (naive is assumed UTC).
        assert!(eq("2024-01-15T10:30:00", "2024-01-15T10:30:00Z"));
        // Lowercase designators parse via chrono: byte order says Greater,
        // instants say Less.
        assert!(lt("2024-01-15t10:00:00z", "2024-01-15T11:00:00Z"));
        // Date-only never parses as a datetime; both paths are lexicographic.
        assert!(lt("2024-01-15", "2024-01-15T00:00:00"));
        assert!(!eq("2024-01-15", "2024-01-15T00:00:00"));
        // Garbage that merely looks datish: one side never parses, so both
        // paths compare lexicographically ('1' < 'x').
        assert!(lt("2024-01-15T10:30:00Z", "2024-01-15Txx:yy:zzZ"));
    }

    // ---- Randomized corpus: fast-path verdict == parse-path verdict ----

    /// SplitMix64: deterministic pseudo-random stream, no dev-deps.
    struct Rng(u64);

    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next_u64() % n
        }
    }

    const SEPS: [char; 3] = ['T', 't', ' '];
    const SUFFIXES: [&str; 8] = [
        "", "Z", "z", "+02:00", "+05:30", "-05:30", "+23:59", "+0530",
    ];

    /// Random datetime-ish string. Field values intentionally overshoot the
    /// valid calendar ranges (month 0/13+, second 60/61, hour 24) so the
    /// corpus covers parse failures and chrono's leap-second folding.
    fn gen_datetime(rng: &mut Rng, sep: char, frac_len: usize, suffix: &str) -> String {
        let year = 1900 + rng.below(200);
        let month = rng.below(15);
        let day = rng.below(34);
        let hour = rng.below(25);
        let min = rng.below(61);
        let sec = rng.below(62);
        let mut s = format!("{year:04}-{month:02}-{day:02}{sep}{hour:02}:{min:02}:{sec:02}");
        if frac_len > 0 {
            s.push('.');
            for _ in 0..frac_len {
                s.push(char::from(b'0' + rng.below(10) as u8));
            }
        }
        s.push_str(suffix);
        s
    }

    #[test]
    fn randomized_corpus_matches_parse_path() {
        let engine = Engine::new();
        let mut rng = Rng(0x00C0_FFEE);

        for _ in 0..4096 {
            let sep = SEPS[rng.below(SEPS.len() as u64) as usize];
            let frac = rng.below(11) as usize; // 0..=10; 10 exceeds the gate
            let suffix = SUFFIXES[rng.below(SUFFIXES.len() as u64) as usize];
            let l = gen_datetime(&mut rng, sep, frac, suffix);

            let r = match rng.below(10) {
                // Same shape as `l` (exercises the eligible branch), often
                // with equal instants thanks to the narrow field ranges.
                0..=4 => gen_datetime(&mut rng, sep, frac, suffix),
                // Identical string.
                5 => l.clone(),
                // Independent shape (mixed designators/precision fall back).
                6..=8 => {
                    let sep = SEPS[rng.below(SEPS.len() as u64) as usize];
                    let frac = rng.below(11) as usize;
                    let suffix = SUFFIXES[rng.below(SUFFIXES.len() as u64) as usize];
                    gen_datetime(&mut rng, sep, frac, suffix)
                }
                // Date-only / week / ordinal specials.
                _ => match rng.below(3) {
                    0 => format!("{:04}-{:02}-{:02}", 1900 + rng.below(200), 1, 15),
                    1 => "2024-W03-1T10:00:00Z".to_string(),
                    _ => "2024-015T10:00:00Z".to_string(),
                },
            };

            assert_matches_parse_path(&engine, &l, &r);
        }
    }

    /// Focused corpus: valid instants only, same-shape pairs around field
    /// boundaries (leap seconds, midnight rollover, fraction edges).
    #[test]
    fn boundary_corpus_matches_parse_path() {
        let engine = Engine::new();
        let cases = [
            "2024-06-30T23:59:59Z",
            "2024-06-30T23:59:60Z",
            "2024-07-01T00:00:00Z",
            "2024-06-30T23:59:59.999999999Z",
            "2024-06-30T23:59:60.000000000Z",
            "2024-06-30T23:59:60.999999999Z",
            "2024-07-01T00:00:00.000000000Z",
            "2024-12-31T23:59:59Z",
            "2025-01-01T00:00:00Z",
            "2024-02-29T00:00:00Z",
            "2024-03-01T00:00:00Z",
            "0000-01-01T00:00:00Z",
            "9999-12-31T23:59:59Z",
        ];
        for l in cases {
            for r in cases {
                assert_matches_parse_path(&engine, l, r);
            }
        }
    }
}
