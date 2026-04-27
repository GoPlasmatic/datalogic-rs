//! Native arena datetime/duration arithmetic. Returns `None` when neither
//! operand is a datetime/duration form so the caller falls through to the
//! generic numeric path.

use crate::arena::DataValue;
use crate::arena::value::coerce_arena_to_number;
use bumpalo::Bump;

/// Wrap an owned string into an arena-resident `DataValue::String`.
#[inline]
fn alloc_string_av<'a>(arena: &'a Bump, s: &str) -> &'a DataValue<'a> {
    arena.alloc(DataValue::String(arena.alloc_str(s)))
}

/// Extract `(DateTime, Duration)` slots from an arena value. The two slots
/// are mutually exclusive — a value parsed as `DateTime` is not also probed
/// for `Duration`.
#[inline]
fn arena_extract_dt_dur(
    av: &DataValue<'_>,
) -> (
    Option<crate::datetime::DataDateTime>,
    Option<crate::datetime::DataDuration>,
) {
    use crate::operators::helpers::{extract_datetime_arena, extract_duration_arena};
    let dt = extract_datetime_arena(av);
    let dur = if dt.is_none() {
        extract_duration_arena(av)
    } else {
        None
    };
    (dt, dur)
}

/// Native arena datetime/duration subtract.
/// - DateTime − DateTime → Duration string.
/// - DateTime − Duration → DateTime ISO string.
/// - Duration − Duration → Duration string.
#[inline]
pub(super) fn arena_datetime_subtract<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let (a_dt, a_dur) = arena_extract_dt_dur(a_av);
    let (b_dt, b_dur) = arena_extract_dt_dur(b_av);

    if let (Some(d1), Some(d2)) = (&a_dt, &b_dt) {
        return Some(alloc_string_av(arena, &d1.diff(d2).to_string()));
    }
    if let (Some(d), Some(dur)) = (&a_dt, &b_dur) {
        return Some(alloc_string_av(arena, &d.sub_duration(dur).to_iso_string()));
    }
    if let (Some(d1), Some(d2)) = (&a_dur, &b_dur) {
        return Some(alloc_string_av(arena, &d1.sub(d2).to_string()));
    }
    None
}

/// Native arena datetime/duration add.
/// - DateTime + Duration → DateTime ISO string.
/// - Duration + Duration → Duration string.
#[inline]
pub(super) fn arena_datetime_add<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let (a_dt, a_dur) = arena_extract_dt_dur(a_av);
    let (_b_dt, b_dur) = arena_extract_dt_dur(b_av);

    if let (Some(dt), Some(dur)) = (&a_dt, &b_dur) {
        return Some(alloc_string_av(
            arena,
            &dt.add_duration(dur).to_iso_string(),
        ));
    }
    if let (Some(d1), Some(d2)) = (&a_dur, &b_dur) {
        return Some(alloc_string_av(arena, &d1.add(d2).to_string()));
    }
    None
}

/// Native arena duration/scalar multiply.
/// - Duration × scalar → Duration string.
/// - scalar × Duration → Duration string.
#[inline]
pub(super) fn arena_datetime_multiply<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let (_, a_dur) = arena_extract_dt_dur(a_av);
    let (_, b_dur) = arena_extract_dt_dur(b_av);

    if let (Some(dur), None) = (&a_dur, &b_dur)
        && let Some(factor) = coerce_arena_to_number(b_av)
    {
        return Some(alloc_string_av(arena, &dur.multiply(factor).to_string()));
    }
    if let (None, Some(dur)) = (&a_dur, &b_dur)
        && let Some(factor) = coerce_arena_to_number(a_av)
    {
        return Some(alloc_string_av(arena, &dur.multiply(factor).to_string()));
    }
    None
}

/// `Duration / Number` → scaled `Duration`. Returns `None` for non-duration
/// LHS so the generic numeric path handles regular division.
#[inline]
pub(super) fn arena_datetime_divide<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<crate::Result<&'a DataValue<'a>>> {
    let (_, a_dur) = arena_extract_dt_dur(a_av);
    let a_dur = a_dur?;
    let divisor = coerce_arena_to_number(b_av)?;
    if divisor == 0.0 {
        return Some(Err(crate::constants::nan_error()));
    }
    Some(Ok(alloc_string_av(
        arena,
        &a_dur.divide(divisor).to_string(),
    )))
}
