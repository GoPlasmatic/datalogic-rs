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
//! Format strings use a simplified syntax that is converted to chrono format
//! internally (longest token wins; raw `%X` chrono specifiers pass through):
//!
//! | Input | Chrono | Description |
//! |-------|--------|-------------|
//! | `yyyy` | `%Y` | 4-digit year |
//! | `MMMM` | `%B` | full month name |
//! | `MMM` | `%b` | abbreviated month name |
//! | `MM` | `%m` | 2-digit month |
//! | `dd` | `%d` | 2-digit day |
//! | `HH` | `%H` | 2-digit hour (24h) |
//! | `mm` | `%M` | 2-digit minute |
//! | `ss` | `%S` | 2-digit second |
//! | `EEEE` | `%A` | full weekday name |
//! | `EEE` | `%a` | abbreviated weekday name |
//!
//! # Timezones
//!
//! `format_date` and `parse_date` accept an optional trailing IANA zone name
//! (chrono-tz's compiled-in table — no tzdata I/O). `format_date` renders the
//! instant as wall-clock time in that zone; `parse_date` reads a naive input
//! as wall-clock time *in* that zone and resolves it to the UTC instant.
//! DST policy for `parse_date`: an ambiguous local time (clocks rolled back)
//! resolves to the earlier instant; a nonexistent one (spring-forward gap)
//! is an error.
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
//! // Format an instant as an Asia/Kolkata calendar date
//! {"format_date": [{"var": "date"}, "dd MMM yyyy", "Asia/Kolkata"]}
//!
//! // Read a naive local time as America/New_York wall clock
//! {"parse_date": ["2026-08-17 09:30", "yyyy-MM-dd HH:mm", "America/New_York"]}
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
    if let Some(s) = av.as_str() {
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

    if let Some(s) = av.as_str() {
        if let Some(duration) = DataDuration::parse(s) {
            // `DataDuration` has a streaming `Display`, so render it straight
            // into the arena rather than through a heap `String`.
            return Ok(arith::write_into_arena(arena, duration));
        }
    }

    Err(Error::invalid_arguments("Invalid duration format"))
}

/// Convert a JSONLogic format spec ("yyyy-MM-dd HH:mm:ss") to a chrono format.
///
/// Single left-to-right pass, longest token first, so `MMMM`/`MMM`/`MM`
/// disambiguate by length and a replacement can never be re-matched by a
/// later token (the old sequential `String::replace` corrupted `"MMM"` into
/// `"%mM"`). A literal `%` starts a raw chrono specifier: it and the
/// following character pass through verbatim, keeping the documented
/// "raw `%X` works too" behavior.
fn jsonlogic_to_chrono_format(format: &str) -> String {
    const TOKENS: &[(&str, &str)] = &[
        ("yyyy", "%Y"),
        ("MMMM", "%B"),
        ("MMM", "%b"),
        ("MM", "%m"),
        ("dd", "%d"),
        ("HH", "%H"),
        ("mm", "%M"),
        ("ss", "%S"),
        ("EEEE", "%A"),
        ("EEE", "%a"),
    ];

    let mut out = String::with_capacity(format.len() + 8);
    let mut rest = format;
    'scan: while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('%') {
            out.push('%');
            let mut chars = after.chars();
            if let Some(ch) = chars.next() {
                out.push(ch);
            }
            rest = chars.as_str();
            continue;
        }
        for (tok, repl) in TOKENS {
            if let Some(after) = rest.strip_prefix(tok) {
                out.push_str(repl);
                rest = after;
                continue 'scan;
            }
        }
        let mut chars = rest.chars();
        // Loop guard ensures at least one char remains.
        out.push(chars.next().expect("non-empty rest"));
        rest = chars.as_str();
    }
    out
}

/// Resolve the optional trailing timezone argument to a chrono-tz zone.
#[inline]
fn resolve_tz(av: &DataValue<'_>) -> Result<chrono_tz::Tz> {
    let s = av
        .as_str()
        .ok_or_else(|| Error::invalid_arguments("Timezone must be a string"))?;
    s.parse::<chrono_tz::Tz>()
        .map_err(|_| Error::invalid_arguments(format!("Unknown timezone: {s}")))
}

/// Render an offset in seconds as the colonless `"+0530"` form that
/// `DataDateTime::format("z")` produces for source offsets — the two `"z"`
/// paths must stay byte-compatible.
fn offset_to_z_string(secs: i32) -> String {
    let sign = if secs < 0 { '-' } else { '+' };
    let abs = secs.abs();
    format!("{sign}{:02}{:02}", abs / 3600, (abs % 3600) / 60)
}

/// Native arena-mode `parse_date`.
///
/// `[string, format]` reads a naive input as UTC (unchanged pre-5.2
/// behavior). The optional third argument names an IANA zone: the naive
/// input is then read as wall-clock time in that zone and resolved to the
/// corresponding UTC instant (see [`parse_date_in_zone`] for the DST
/// policy).
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
    if let (Some(date), Some(fmt)) = (date_av.as_str(), fmt_av.as_str()) {
        let chrono_format = jsonlogic_to_chrono_format(fmt);
        if args.len() >= 3 {
            let tz_av = engine.dispatch_node(&args[2], ctx, arena)?;
            let tz = resolve_tz(tz_av)?;
            return parse_date_in_zone(date, &chrono_format, tz, arena);
        }
        if let Some(dt) = DataDateTime::parse_with_format(date, &chrono_format) {
            let iso = dt.to_iso_string();
            let s: &'a str = arena.alloc_str(&iso);
            return Ok(arena.alloc(DataValue::String(s)));
        }
    }
    Err(Error::invalid_arguments("Failed to parse date"))
}

/// Zone-aware `parse_date` tail: naive parse, then wall-clock → instant
/// through the zone's offset table.
///
/// DST policy: an ambiguous local time (clocks rolled back, two instants
/// share the wall-clock) resolves to the **earlier** instant; a
/// nonexistent one (spring-forward gap) is an error rather than a silent
/// shift.
fn parse_date_in_zone<'a>(
    date: &str,
    chrono_format: &str,
    tz: chrono_tz::Tz,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    use chrono::offset::LocalResult;
    use chrono::{NaiveDate, NaiveDateTime, Offset, TimeZone};

    // Mirror `DataDateTime::parse_with_format`: full datetime first, then
    // date-only read as midnight.
    let naive = NaiveDateTime::parse_from_str(date, chrono_format)
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(date, chrono_format)
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        })
        .ok_or_else(|| Error::invalid_arguments("Failed to parse date"))?;

    let zoned = match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(earliest, _) => earliest,
        LocalResult::None => {
            return Err(Error::invalid_arguments(
                "Nonexistent local time for timezone",
            ));
        }
    };
    let data_dt = DataDateTime {
        dt: zoned.with_timezone(&Utc),
        original_offset: Some(zoned.offset().fix().local_minus_utc()),
    };
    let s: &'a str = arena.alloc_str(&data_dt.to_iso_string());
    Ok(arena.alloc(DataValue::String(s)))
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

    let fmt: &'a str = fmt_av
        .as_str()
        .ok_or_else(|| Error::invalid_arguments("Failed to format date"))?;

    if let Some(datetime) = dt {
        // Optional trailing IANA zone: render the instant as wall-clock
        // time in that zone. This path bypasses `DataDateTime::format`
        // entirely — its `"z"` branch reports the *source* offset, but
        // with an explicit zone the offset reported must be the target
        // zone's at that instant (DST-correct via chrono-tz's table).
        if args.len() >= 3 {
            use chrono::Offset;
            let tz_av = engine.dispatch_node(&args[2], ctx, arena)?;
            let tz = resolve_tz(tz_av)?;
            let zoned = datetime.dt.with_timezone(&tz);
            let formatted = if fmt == "z" {
                offset_to_z_string(zoned.offset().fix().local_minus_utc())
            } else {
                zoned.format(&jsonlogic_to_chrono_format(fmt)).to_string()
            };
            let s: &'a str = arena.alloc_str(&formatted);
            return Ok(arena.alloc(DataValue::String(s)));
        }

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
    let unit = unit_av.as_str();

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

#[cfg(test)]
mod tests {
    use super::jsonlogic_to_chrono_format;

    #[test]
    fn format_tokens_translate_longest_first() {
        assert_eq!(
            jsonlogic_to_chrono_format("yyyy-MM-dd HH:mm:ss"),
            "%Y-%m-%d %H:%M:%S"
        );
        // The month-name family disambiguates by length — the old
        // sequential replace corrupted "MMM" into "%mM".
        assert_eq!(jsonlogic_to_chrono_format("dd MMM yyyy"), "%d %b %Y");
        assert_eq!(jsonlogic_to_chrono_format("MMMM"), "%B");
        assert_eq!(jsonlogic_to_chrono_format("EEEE, dd MMMM"), "%A, %d %B");
        assert_eq!(jsonlogic_to_chrono_format("EEE"), "%a");
        // Raw chrono specifiers pass through untouched.
        assert_eq!(jsonlogic_to_chrono_format("%Y-%m-%d"), "%Y-%m-%d");
        assert_eq!(jsonlogic_to_chrono_format("%Z"), "%Z");
        // Literal text between tokens survives.
        assert_eq!(jsonlogic_to_chrono_format("at HH:mm"), "at %H:%M");
    }
}
