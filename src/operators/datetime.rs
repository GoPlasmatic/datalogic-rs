//! DateTime and Duration operators for temporal data handling.
//!
//! This module provides operators for working with dates, times, and durations in JSONLogic.
//! It supports ISO 8601 datetime strings and duration formats.
//!
//! # Supported Operators
//!
//! - `datetime` - Parse or validate a datetime value
//! - `timestamp` - Parse or validate a duration value
//! - `parse_date` - Parse a date string with a custom format
//! - `format_date` - Format a datetime with a custom format string
//! - `date_diff` - Calculate the difference between two dates
//! - `now` - Get the current UTC datetime
//!
//! # Format String Conversion
//!
//! Format strings use a simplified syntax that is converted to chrono format internally:
//!
//! | Input | Chrono | Description |
//! |-------|--------|-------------|
//! | `yyyy` | `%Y` | 4-digit year |
//! | `MM` | `%m` | 2-digit month |
//! | `dd` | `%d` | 2-digit day |
//! | `HH` | `%H` | 2-digit hour (24h) |
//! | `mm` | `%M` | 2-digit minute |
//! | `ss` | `%S` | 2-digit second |
//!
//! # Examples
//!
//! ```json
//! // Parse and validate a datetime
//! {"datetime": "2024-01-15T10:30:00Z"}
//!
//! // Format a datetime
//! {"format_date": [{"var": "date"}, "yyyy-MM-dd"]}
//!
//! // Calculate days between two dates
//! {"date_diff": [{"var": "start"}, {"var": "end"}, "days"]}
//! ```

use chrono::Utc;

use crate::datetime::{DataDateTime, DataDuration};
use crate::{CompiledNode, DataLogic, Error, Result};

// =============================================================================
// Datetime operators.
// =============================================================================

use crate::arena::{ArenaContextStack, ArenaValue};
use bumpalo::Bump;

/// Resolve an arg as an arena string. Returns `None` if not string-like.
#[inline]
fn arg_as_str_arena<'a>(av: &'a ArenaValue<'a>) -> Option<&'a str> {
    match av {
        ArenaValue::String(s) => Some(*s),
        _ => None,
    }
}

/// True iff this arena Object has a `datetime` key (boundary form).
#[inline]
fn is_datetime_object_arena(av: &ArenaValue<'_>) -> bool {
    matches!(av, ArenaValue::Object(pairs) if pairs.iter().any(|(k, _)| *k == "datetime"))
}

/// True iff this arena Object has a `timestamp` key (boundary form).
#[inline]
fn is_duration_object_arena(av: &ArenaValue<'_>) -> bool {
    matches!(av, ArenaValue::Object(pairs) if pairs.iter().any(|(k, _)| *k == "timestamp"))
}

/// Native arena-mode `datetime`. Returns the input unchanged if it parses
/// as a datetime (object or ISO string); errors otherwise.
#[inline]
pub(crate) fn evaluate_datetime_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(
            "datetime requires an argument".to_string(),
        ));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, arena)?;

    // Datetime object passthrough.
    if is_datetime_object_arena(av) {
        return Ok(av);
    }

    // String parses as datetime → return as-is to preserve timezone info.
    if let Some(s) = arg_as_str_arena(av)
        && DataDateTime::parse(s).is_some()
    {
        return Ok(av);
    }

    Err(Error::InvalidArguments(
        "Invalid datetime format".to_string(),
    ))
}

/// Native arena-mode `timestamp`. Returns the input unchanged if it parses
/// as a duration (object or string); errors otherwise.
#[inline]
pub(crate) fn evaluate_timestamp_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(
            "timestamp requires an argument".to_string(),
        ));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, arena)?;

    if is_duration_object_arena(av) {
        return Ok(av);
    }

    if let Some(s) = arg_as_str_arena(av)
        && let Some(duration) = DataDuration::parse(s)
    {
        let s_arena: &'a str = arena.alloc_str(&duration.to_string());
        return Ok(arena.alloc(ArenaValue::String(s_arena)));
    }

    Err(Error::InvalidArguments(
        "Invalid duration format".to_string(),
    ))
}

/// Convert a JSONLogic format spec ("yyyy-MM-dd HH:mm:ss") to a chrono format.
#[inline]
fn jsonlogic_to_chrono_format(format: &str) -> String {
    format
        .replace("yyyy", "%Y")
        .replace("MM", "%m")
        .replace("dd", "%d")
        .replace("HH", "%H")
        .replace("mm", "%M")
        .replace("ss", "%S")
}

/// Native arena-mode `parse_date`.
#[inline]
pub(crate) fn evaluate_parse_date_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(
            "parse_date requires date string and format".to_string(),
        ));
    }
    let date_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let fmt_av = engine.evaluate_arena_node(&args[1], actx, arena)?;
    if let (Some(date), Some(fmt)) = (arg_as_str_arena(date_av), arg_as_str_arena(fmt_av)) {
        let chrono_format = jsonlogic_to_chrono_format(fmt);
        if let Some(dt) = DataDateTime::parse_with_format(date, &chrono_format) {
            let iso = dt.to_iso_string();
            let s: &'a str = arena.alloc_str(&iso);
            return Ok(arena.alloc(ArenaValue::String(s)));
        }
    }
    Err(Error::InvalidArguments("Failed to parse date".to_string()))
}

/// Native arena-mode `format_date`.
#[inline]
pub(crate) fn evaluate_format_date_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(
            "format_date requires datetime and format".to_string(),
        ));
    }
    let dt_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let fmt_av = engine.evaluate_arena_node(&args[1], actx, arena)?;

    // Resolve the datetime — supports object form and string form.
    let dt: Option<DataDateTime> = crate::operators::helpers::extract_datetime_arena(dt_av);

    let fmt: &'a str = arg_as_str_arena(fmt_av)
        .ok_or_else(|| Error::InvalidArguments("Failed to format date".to_string()))?;

    if let Some(datetime) = dt {
        let chrono_format = if fmt == "z" {
            fmt.to_string()
        } else {
            jsonlogic_to_chrono_format(fmt)
        };
        let formatted = datetime.format(&chrono_format);
        let s: &'a str = arena.alloc_str(&formatted);
        return Ok(arena.alloc(ArenaValue::String(s)));
    }

    Err(Error::InvalidArguments("Failed to format date".to_string()))
}

/// Native arena-mode `date_diff`.
#[inline]
pub(crate) fn evaluate_date_diff_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 3 {
        return Err(Error::InvalidArguments(
            "date_diff requires two dates and a unit".to_string(),
        ));
    }
    let d1_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let d2_av = engine.evaluate_arena_node(&args[1], actx, arena)?;
    let unit_av = engine.evaluate_arena_node(&args[2], actx, arena)?;

    let resolve_dt = |av: &'a ArenaValue<'a>| -> Option<DataDateTime> {
        crate::operators::helpers::extract_datetime_arena(av)
    };
    let dt1 = resolve_dt(d1_av);
    let dt2 = resolve_dt(d2_av);
    let unit = arg_as_str_arena(unit_av);

    if let (Some(a), Some(b), Some(u)) = (dt1, dt2, unit) {
        let diff = a.diff_in_unit(&b, u);
        return Ok(arena.alloc(ArenaValue::from_i64(diff as i64)));
    }
    Err(Error::InvalidArguments(
        "Failed to calculate date difference".to_string(),
    ))
}

/// Native arena-mode `now`. Allocates the ISO string in the arena.
#[inline]
pub(crate) fn evaluate_now_arena<'a>(
    _args: &[CompiledNode],
    _actx: &mut ArenaContextStack<'a>,
    _engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let now = Utc::now();
    let data_dt = DataDateTime {
        dt: now,
        original_offset: Some(0),
    };
    let s: &'a str = arena.alloc_str(&data_dt.to_iso_string());
    Ok(arena.alloc(ArenaValue::String(s)))
}
