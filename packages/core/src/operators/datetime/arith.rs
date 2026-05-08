//! Native arena datetime/duration arithmetic. Returns `None` when neither
//! operand is a datetime/duration form so the caller falls through to the
//! generic numeric path.

use crate::arena::DataValue;
use crate::arena::value::coerce_to_number;
use bumpalo::Bump;
use std::fmt::Display;

/// Format a `Display` value directly into an arena-backed `DataValue::String`,
/// skipping the intermediate heap `String` that `value.to_string()` would
/// allocate. The `bumpalo::collections::String` writes through the arena, so
/// the only allocation is the destination buffer plus the `DataValue` wrapper.
///
/// `Display` impls that internally allocate (e.g. `DataDateTime::fmt` calls
/// `to_iso_string`) still pay that intermediate string upstream — the savings
/// only land for streaming `Display` impls like `DataDuration::fmt`.
#[inline]
fn write_into_arena<'a>(arena: &'a Bump, value: impl Display) -> &'a DataValue<'a> {
    use std::fmt::Write;
    let mut buf = bumpalo::collections::String::new_in(arena);
    // `bumpalo::collections::String` writes never fail; `expect` rather than
    // `unwrap_or_default` so a future bug in bumpalo surfaces loudly.
    write!(&mut buf, "{}", value).expect("bumpalo String write is infallible");
    arena.alloc(DataValue::String(buf.into_bump_str()))
}

/// Extract `(DateTime, Duration)` slots from an arena value. The two slots
/// are mutually exclusive — a value parsed as `DateTime` is not also probed
/// for `Duration`.
#[inline]
fn extract_dt_dur(
    av: &DataValue<'_>,
) -> (
    Option<datavalue::DataDateTime>,
    Option<datavalue::DataDuration>,
) {
    use super::{extract_datetime, extract_duration};
    let dt = extract_datetime(av);
    let dur = if dt.is_none() {
        extract_duration(av)
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
pub(crate) fn datetime_subtract<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let (a_dt, a_dur) = extract_dt_dur(a_av);
    let (b_dt, b_dur) = extract_dt_dur(b_av);

    if let (Some(d1), Some(d2)) = (&a_dt, &b_dt) {
        return Some(write_into_arena(arena, d1.diff(d2)));
    }
    if let (Some(d), Some(dur)) = (&a_dt, &b_dur) {
        return Some(write_into_arena(arena, d.sub_duration(dur)));
    }
    if let (Some(d1), Some(d2)) = (&a_dur, &b_dur) {
        return Some(write_into_arena(arena, d1.sub(d2)));
    }
    None
}

/// Native arena datetime/duration add.
/// - DateTime + Duration → DateTime ISO string.
/// - Duration + Duration → Duration string.
#[inline]
pub(crate) fn datetime_add<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let (a_dt, a_dur) = extract_dt_dur(a_av);
    let (_b_dt, b_dur) = extract_dt_dur(b_av);

    if let (Some(dt), Some(dur)) = (&a_dt, &b_dur) {
        return Some(write_into_arena(arena, dt.add_duration(dur)));
    }
    if let (Some(d1), Some(d2)) = (&a_dur, &b_dur) {
        return Some(write_into_arena(arena, d1.add(d2)));
    }
    None
}

/// Native arena duration/scalar multiply.
/// - Duration × scalar → Duration string.
/// - scalar × Duration → Duration string.
#[inline]
pub(crate) fn datetime_multiply<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let (_, a_dur) = extract_dt_dur(a_av);
    let (_, b_dur) = extract_dt_dur(b_av);

    if let (Some(dur), None) = (&a_dur, &b_dur)
        && let Some(factor) = coerce_to_number(b_av)
    {
        return Some(write_into_arena(arena, dur.multiply(factor)));
    }
    if let (None, Some(dur)) = (&a_dur, &b_dur)
        && let Some(factor) = coerce_to_number(a_av)
    {
        return Some(write_into_arena(arena, dur.multiply(factor)));
    }
    None
}

/// `Duration / Number` → scaled `Duration`. Returns `None` for non-duration
/// LHS so the generic numeric path handles regular division.
#[inline]
pub(crate) fn datetime_divide<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<crate::Result<&'a DataValue<'a>>> {
    let (_, a_dur) = extract_dt_dur(a_av);
    let a_dur = a_dur?;
    let divisor = coerce_to_number(b_av)?;
    if divisor == 0.0 {
        return Some(Err(crate::Error::nan()));
    }
    Some(Ok(write_into_arena(arena, a_dur.divide(divisor))))
}
