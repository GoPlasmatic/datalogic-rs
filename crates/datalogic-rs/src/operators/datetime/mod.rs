//! DateTime and Duration operators for temporal data handling.
//!
//! This module provides operators for working with dates, times, and durations in JSONLogic.
//! It supports ISO 8601 datetime strings and duration formats.
//!
//! # Submodules
//!
//! - [`arith`] — `+` / `-` / `*` / `/` / `%` cases that involve datetime or
//!   duration operands. Called from `operators::arithmetic::*` when the
//!   generic numeric path detects a datetime-shaped operand.
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

pub(crate) mod arith;

use chrono::Utc;

use crate::{CompiledNode, Engine, Error, Result};
use datavalue::{DataDateTime, DataDuration};

// =============================================================================
// Datetime operators.
// =============================================================================

use crate::arena::{ContextStack, DataValue};
use bumpalo::Bump;

// =============================================================================
// Sentinel-form extraction helpers (used by comparison + arithmetic ops too).
// =============================================================================

/// Arena-native datetime extraction — walks `String` / `Object` arena values
/// directly without `Value` materialization. Recognises both ISO datetime
/// strings and `{datetime: <iso>}` sentinel objects.
#[inline]
pub(crate) fn extract_datetime(av: &DataValue<'_>) -> Option<DataDateTime> {
    match av {
        DataValue::DateTime(dt) => Some(*dt),
        DataValue::String(s) => DataDateTime::parse(s),
        DataValue::Object(pairs) => {
            for (k, v) in *pairs {
                if *k == "datetime" {
                    if let DataValue::String(s) = v {
                        return DataDateTime::parse(s);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Arena-native duration extraction. See [`extract_datetime`].
#[inline]
pub(crate) fn extract_duration(av: &DataValue<'_>) -> Option<DataDuration> {
    match av {
        DataValue::Duration(d) => Some(*d),
        DataValue::String(s) => DataDuration::parse(s),
        DataValue::Object(pairs) => {
            for (k, v) in *pairs {
                if *k == "timestamp" {
                    if let DataValue::String(s) = v {
                        return DataDuration::parse(s);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Resolve an arg as an arena string. Returns `None` if not string-like.
#[inline]
fn arg_as_str<'a>(av: &'a DataValue<'a>) -> Option<&'a str> {
    match av {
        DataValue::String(s) => Some(*s),
        _ => None,
    }
}

/// True iff this arena Object has a `datetime` key (boundary form).
#[inline]
fn is_datetime_object(av: &DataValue<'_>) -> bool {
    matches!(av, DataValue::Object(pairs) if pairs.iter().any(|(k, _)| *k == "datetime"))
}

/// True iff this arena Object has a `timestamp` key (boundary form).
#[inline]
fn is_duration_object(av: &DataValue<'_>) -> bool {
    matches!(av, DataValue::Object(pairs) if pairs.iter().any(|(k, _)| *k == "timestamp"))
}

/// Native arena-mode `datetime`. Returns the input unchanged if it parses
/// as a datetime (object or ISO string); errors otherwise.
#[inline]
pub(crate) fn evaluate_datetime<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(Error::invalid_arguments("datetime requires an argument"));
    }
    let av = engine.dispatch_node(&args[0], ctx, arena)?;

    // Datetime object passthrough.
    if is_datetime_object(av) {
        return Ok(av);
    }

    // String parses as datetime → return as-is to preserve timezone info.
    if let Some(s) = arg_as_str(av) {
        if DataDateTime::parse(s).is_some() {
            return Ok(av);
        }
    }

    Err(Error::invalid_arguments("Invalid datetime format"))
}

/// Native arena-mode `timestamp`. Returns the input unchanged if it parses
/// as a duration (object or string); errors otherwise.
#[inline]
pub(crate) fn evaluate_timestamp<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(Error::invalid_arguments("timestamp requires an argument"));
    }
    let av = engine.dispatch_node(&args[0], ctx, arena)?;

    if is_duration_object(av) {
        return Ok(av);
    }

    if let Some(s) = arg_as_str(av) {
        if let Some(duration) = DataDuration::parse(s) {
            // `DataDuration` has a streaming `Display`, so render it straight
            // into the arena rather than through a heap `String`.
            return Ok(arith::write_into_arena(arena, duration));
        }
    }

    Err(Error::invalid_arguments("Invalid duration format"))
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
pub(crate) fn evaluate_parse_date<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(Error::invalid_arguments(
            "parse_date requires date string and format",
        ));
    }
    let date_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let fmt_av = engine.dispatch_node(&args[1], ctx, arena)?;
    if let (Some(date), Some(fmt)) = (arg_as_str(date_av), arg_as_str(fmt_av)) {
        let chrono_format = jsonlogic_to_chrono_format(fmt);
        if let Some(dt) = DataDateTime::parse_with_format(date, &chrono_format) {
            let iso = dt.to_iso_string();
            let s: &'a str = arena.alloc_str(&iso);
            return Ok(arena.alloc(DataValue::String(s)));
        }
    }
    Err(Error::invalid_arguments("Failed to parse date"))
}

/// Native arena-mode `format_date`.
#[inline]
pub(crate) fn evaluate_format_date<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(Error::invalid_arguments(
            "format_date requires datetime and format",
        ));
    }
    let dt_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let fmt_av = engine.dispatch_node(&args[1], ctx, arena)?;

    // Resolve the datetime — supports object form and string form.
    let dt: Option<DataDateTime> = extract_datetime(dt_av);

    let fmt: &'a str =
        arg_as_str(fmt_av).ok_or_else(|| Error::invalid_arguments("Failed to format date"))?;

    if let Some(datetime) = dt {
        let chrono_format = if fmt == "z" {
            fmt.to_string()
        } else {
            jsonlogic_to_chrono_format(fmt)
        };
        let formatted = datetime.format(&chrono_format);
        let s: &'a str = arena.alloc_str(&formatted);
        return Ok(arena.alloc(DataValue::String(s)));
    }

    Err(Error::invalid_arguments("Failed to format date"))
}

/// Native arena-mode `date_diff`.
#[inline]
pub(crate) fn evaluate_date_diff<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 3 {
        return Err(Error::invalid_arguments(
            "date_diff requires two dates and a unit",
        ));
    }
    let d1_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let d2_av = engine.dispatch_node(&args[1], ctx, arena)?;
    let unit_av = engine.dispatch_node(&args[2], ctx, arena)?;

    let dt1 = extract_datetime(d1_av);
    let dt2 = extract_datetime(d2_av);
    let unit = arg_as_str(unit_av);

    if let (Some(a), Some(b), Some(u)) = (dt1, dt2, unit) {
        let diff = a.diff_in_unit(&b, u);
        return Ok(arena.alloc(DataValue::from_i64(diff as i64)));
    }
    Err(Error::invalid_arguments(
        "Failed to calculate date difference",
    ))
}

/// Native arena-mode `now`. Allocates the ISO string in the arena.
#[inline]
pub(crate) fn evaluate_now<'a>(
    _args: &[CompiledNode],
    _ctx: &mut ContextStack<'a>,
    _engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let now = Utc::now();
    let data_dt = DataDateTime {
        dt: now,
        original_offset: Some(0),
    };
    let s: &'a str = arena.alloc_str(&data_dt.to_iso_string());
    Ok(arena.alloc(DataValue::String(s)))
}
