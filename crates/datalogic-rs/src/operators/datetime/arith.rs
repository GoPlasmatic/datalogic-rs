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
pub(super) fn write_into_arena<'a>(arena: &'a Bump, value: impl Display) -> &'a DataValue<'a> {
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

/// A datetime or a duration operand of `+` / `-`.
#[derive(Clone, Copy)]
pub(crate) enum Temporal {
    DateTime(datavalue::DataDateTime),
    Duration(datavalue::DataDuration),
}

impl Temporal {
    /// The value as a datetime, else as a duration (a value that parses as
    /// a datetime is not also probed as a duration).
    #[inline]
    pub(crate) fn of(av: &DataValue<'_>) -> Option<Self> {
        match extract_dt_dur(av) {
            (Some(dt), _) => Some(Temporal::DateTime(dt)),
            (None, Some(dur)) => Some(Temporal::Duration(dur)),
            (None, None) => None,
        }
    }

    /// `self + other`: a datetime plus a duration, in either order, is a
    /// datetime; two durations add. Two datetimes have no sum.
    #[inline]
    pub(crate) fn add(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Temporal::DateTime(dt), Temporal::Duration(dur))
            | (Temporal::Duration(dur), Temporal::DateTime(dt)) => {
                Some(Temporal::DateTime(dt.add_duration(&dur)))
            }
            (Temporal::Duration(d1), Temporal::Duration(d2)) => {
                Some(Temporal::Duration(d1.add(&d2)))
            }
            (Temporal::DateTime(_), Temporal::DateTime(_)) => None,
        }
    }

    /// `self - other`: datetime − datetime is a duration, datetime −
    /// duration a datetime, duration − duration a duration. A duration
    /// minus a datetime has no value.
    #[inline]
    pub(crate) fn sub(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Temporal::DateTime(d1), Temporal::DateTime(d2)) => {
                Some(Temporal::Duration(d1.diff(&d2)))
            }
            (Temporal::DateTime(dt), Temporal::Duration(dur)) => {
                Some(Temporal::DateTime(dt.sub_duration(&dur)))
            }
            (Temporal::Duration(d1), Temporal::Duration(d2)) => {
                Some(Temporal::Duration(d1.sub(&d2)))
            }
            (Temporal::Duration(_), Temporal::DateTime(_)) => None,
        }
    }

    /// The result as arithmetic returns it: the ISO string of a datetime,
    /// the `1d:0h:0m:0s` string of a duration.
    #[inline]
    pub(crate) fn into_value<'a>(self, arena: &'a Bump) -> &'a DataValue<'a> {
        match self {
            Temporal::DateTime(dt) => write_into_arena(arena, dt),
            Temporal::Duration(dur) => write_into_arena(arena, dur),
        }
    }
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
    let a = Temporal::of(a_av)?;
    let b = Temporal::of(b_av)?;
    Some(a.sub(b)?.into_value(arena))
}

/// Native arena datetime/duration add.
/// - DateTime + Duration, in either order → DateTime ISO string.
/// - Duration + Duration → Duration string.
#[inline]
pub(crate) fn datetime_add<'a>(
    a_av: &'a DataValue<'a>,
    b_av: &'a DataValue<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let a = Temporal::of(a_av)?;
    let b = Temporal::of(b_av)?;
    Some(a.add(b)?.into_value(arena))
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
