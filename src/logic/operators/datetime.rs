//! DateTime operators for logic expressions.
//!
//! This module provides operators for working with datetime and duration values.

use chrono::{FixedOffset, Local, TimeZone};

use crate::arena::DataArena;
use crate::context::EvalContext;
use crate::logic::error::{LogicError, Result};
use crate::value::{DataValue, date_diff, parse_datetime, parse_duration};

/// Enumeration of datetime operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeOp {
    /// Direct datetime conversion
    DateTime,
    /// Duration/timestamp conversion
    Timestamp,
    /// Current date and time
    Now,
    /// Parse a date string with a format
    ParseDate,
    /// Format a date according to a specified format
    FormatDate,
    /// Calculate difference between two dates
    DateDiff,
}

/// Validates that exactly n arguments are provided
fn validate_argument_count(args: &[DataValue], expected: usize) -> Result<()> {
    if args.len() != expected {
        return Err(LogicError::InvalidArgumentsError);
    }
    Ok(())
}

/// Converts from a human-readable format string to a chrono format string.
fn convert_format_to_chrono(format: &str) -> String {
    // This is a simplified version. We could add more conversions as needed.
    format
        .replace("yyyy", "%Y")
        .replace("MM", "%m")
        .replace("dd", "%d")
        .replace("HH", "%H")
        .replace("mm", "%M")
        .replace("ss", "%S")
        .replace("z", "%z")
}

/// Extracts a datetime from a value, handling both direct and wrapped forms
fn extract_datetime<'a>(
    value: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a chrono::DateTime<FixedOffset>> {
    match value {
        DataValue::DateTime(dt) => Ok(dt),
        DataValue::Object(entries) => {
            // Look for a "datetime" entry
            entries
                .iter()
                .find(|(key, _)| *key == "datetime")
                .and_then(|(_, value)| {
                    if let DataValue::String(datetime_str) = value {
                        Some(datetime_str)
                    } else {
                        None
                    }
                })
                .ok_or(LogicError::InvalidArgumentsError)
                .and_then(|datetime_str| {
                    parse_datetime(datetime_str)
                        .map_err(|_e| LogicError::InvalidArgumentsError)
                        .map(|dt| arena.alloc(dt))
                })
        }
        DataValue::String(datetime_str) => parse_datetime(datetime_str)
            .map_err(|_e| LogicError::InvalidArgumentsError)
            .map(|dt| arena.alloc(dt)),
        _ => Err(LogicError::InvalidArgumentsError),
    }
}

/// Creates a duration value from a string.
pub fn eval_timestamp_operator<'a>(
    args: &'a [DataValue<'a>],
    _context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    validate_argument_count(args, 1)?;

    match &args[0] {
        DataValue::String(s) => {
            // Try to parse the string as a duration
            match parse_duration(s) {
                Ok(duration) => Ok(arena.alloc(DataValue::duration(duration))),
                Err(_) => Err(LogicError::InvalidArgumentsError),
            }
        }
        DataValue::Duration(dur) => {
            // If already a duration, wrap it in an object
            Ok(arena.alloc(DataValue::duration(*dur)))
        }
        _ => Err(LogicError::InvalidArgumentsError),
    }
}

/// Gets the current date and time.
pub fn eval_now<'a>(_context: &EvalContext<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    let now = Local::now().with_timezone(&FixedOffset::east_opt(0).unwrap());
    Ok(arena.alloc(DataValue::datetime(now)))
}

/// Formats a date according to the specified format string.
pub fn eval_format_date<'a>(
    args: &'a [DataValue<'a>],
    _context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Extract the datetime from the first argument
    let dt = extract_datetime(&args[0], arena)?;

    // Ensure the second argument is a format string
    let format_str = match &args[1] {
        DataValue::String(s) => s,
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    // Convert from human-readable format to chrono format
    let chrono_format = convert_format_to_chrono(format_str);

    // Format the datetime
    let formatted = dt.format(&chrono_format).to_string();

    // Note: Removed special case handling for yyyy-MM-dd format
    // All format_date operations should return strings, not DateTime objects

    // Return the formatted string
    Ok(arena.alloc(DataValue::String(arena.alloc_str(&formatted))))
}

/// Parses a string into a date using a specified format.
pub fn eval_parse_date<'a>(
    args: &'a [DataValue<'a>],
    _context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    validate_argument_count(args, 2)?;

    let date_str = match &args[0] {
        DataValue::String(s) => s,
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    let format_str = match &args[1] {
        DataValue::String(s) => s,
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    let chrono_format = convert_format_to_chrono(format_str);

    // Use the non-deprecated method
    match chrono::NaiveDateTime::parse_from_str(date_str, &chrono_format) {
        Ok(naive_dt) => {
            let dt = FixedOffset::east_opt(0)
                .unwrap()
                .from_utc_datetime(&naive_dt);
            // Return formatted string instead of datetime object
            let formatted = if dt.offset().local_minus_utc() == 0 {
                dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
            } else {
                dt.to_rfc3339()
            };
            Ok(arena.alloc(DataValue::string(arena, &formatted)))
        }
        Err(_) => {
            // Try as date only
            match chrono::NaiveDate::parse_from_str(date_str, &chrono_format) {
                Ok(date) => {
                    let naive_dt = date.and_hms_opt(0, 0, 0).unwrap();
                    let dt = FixedOffset::east_opt(0)
                        .unwrap()
                        .from_utc_datetime(&naive_dt);
                    // Return formatted string instead of datetime object
                    let formatted = if dt.offset().local_minus_utc() == 0 {
                        dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                    } else {
                        dt.to_rfc3339()
                    };
                    Ok(arena.alloc(DataValue::string(arena, &formatted)))
                }
                Err(_e) => Err(LogicError::InvalidArgumentsError),
            }
        }
    }
}

/// Calculates the difference between two dates.
pub fn eval_date_diff<'a>(
    args: &'a [DataValue<'a>],
    _context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    validate_argument_count(args, 3)?;

    // Extract datetime from the first and second arguments
    let dt1 = extract_datetime(&args[0], arena)?;
    let dt2 = extract_datetime(&args[1], arena)?;

    let unit = match &args[2] {
        DataValue::String(s) => s,
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    let diff = date_diff(dt2, dt1, unit);
    Ok(arena.alloc(DataValue::integer(diff)))
}

/// Creates a datetime directly from a string without requiring a format.
pub fn eval_datetime_operator<'a>(
    args: &'a [DataValue<'a>],
    _context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    validate_argument_count(args, 1)?;
    match &args[0] {
        DataValue::String(s) => {
            // Try to parse the string as a datetime
            match parse_datetime(s) {
                Ok(dt) => {
                    // Return formatted string instead of datetime object
                    let formatted = if dt.offset().local_minus_utc() == 0 {
                        dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                    } else {
                        dt.to_rfc3339()
                    };
                    Ok(arena.alloc(DataValue::string(arena, &formatted)))
                }
                Err(_) => Err(LogicError::InvalidArgumentsError),
            }
        }
        DataValue::DateTime(dt) => {
            // If already a datetime, return it as a formatted string
            let formatted = if dt.offset().local_minus_utc() == 0 {
                dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
            } else {
                dt.to_rfc3339()
            };
            Ok(arena.alloc(DataValue::string(arena, &formatted)))
        }
        _ => Err(LogicError::InvalidArgumentsError),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_eval_timestamp() {
        let arena = DataArena::new();

        // Test with valid duration string
        let args = [DataValue::string(&arena, "1d:2h:3m:4s")];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_timestamp_operator(&args, &dummy_context, &arena).unwrap();

        // Check that it's a duration directly
        assert!(result.is_duration());
        let dur = result.as_duration().unwrap();
        assert_eq!(dur.num_days(), 1);
        assert_eq!(dur.num_hours() % 24, 2);
        assert_eq!(dur.num_minutes() % 60, 3);
        assert_eq!(dur.num_seconds() % 60, 4);

        // Test with invalid duration string
        let args = [DataValue::string(&arena, "invalid")];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_timestamp_operator(&args, &dummy_context, &arena);
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_now() {
        let arena = DataArena::new();
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);

        let result = eval_now(&dummy_context, &arena).unwrap();
        assert!(result.is_datetime());
    }

    #[test]
    fn test_eval_format_date() {
        let arena = DataArena::new();

        let dt = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 13, 20, 6)
            .unwrap();

        // Test standard format that returns a string
        let args = [
            DataValue::datetime(dt),
            DataValue::string(&arena, "yyyy-MM-dd"),
        ];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_format_date(&args, &dummy_context, &arena).unwrap();

        // format_date should always return a string
        assert!(result.is_string());
        assert_eq!(result.as_str().unwrap(), "2022-07-06");

        // Test a different format that should return a string
        let args2 = [
            DataValue::datetime(dt),
            DataValue::string(&arena, "yyyy/MM/dd HH:mm"),
        ];
        let dummy_data2 = arena.alloc(DataValue::Null);
        let dummy_context2 = crate::context::EvalContext::new(dummy_data2);
        let result2 = eval_format_date(&args2, &dummy_context2, &arena).unwrap();
        assert!(result2.is_string());
        assert_eq!(result2.as_str().unwrap(), "2022/07/06 13:20");
    }

    #[test]
    fn test_eval_parse_date() {
        let arena = DataArena::new();

        let args = [
            DataValue::string(&arena, "2022-07-06"),
            DataValue::string(&arena, "yyyy-MM-dd"),
        ];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_parse_date(&args, &dummy_context, &arena).unwrap();
        assert!(result.is_string());

        let formatted = result.as_str().unwrap();
        assert_eq!(formatted, "2022-07-06T00:00:00Z");
    }

    #[test]
    fn test_eval_date_diff() {
        let arena = DataArena::new();

        // Testing positive date difference
        let dt1 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 0, 0, 0)
            .unwrap();
        let dt2 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 7, 0, 0, 0)
            .unwrap();

        let args = [
            DataValue::datetime(dt1),
            DataValue::datetime(dt2),
            DataValue::string(&arena, "days"),
        ];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_date_diff(&args, &dummy_context, &arena).unwrap();
        assert_eq!(result.as_i64().unwrap(), -1); // dt1 - dt2 = -1 day (from dt2 to dt1)

        // Testing with reversed dates
        let args = [
            DataValue::datetime(dt2),
            DataValue::datetime(dt1),
            DataValue::string(&arena, "days"),
        ];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_date_diff(&args, &dummy_context, &arena).unwrap();
        assert_eq!(result.as_i64().unwrap(), 1); // dt2 - dt1 = 1 day (from dt1 to dt2)
    }

    #[test]
    fn test_eval_datetime_operator() {
        let arena = DataArena::new();

        // Test with valid datetime string
        let args = [DataValue::string(&arena, "2022-07-06T13:20:06Z")];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_datetime_operator(&args, &dummy_context, &arena).unwrap();

        // Check that it's a datetime directly
        assert!(result.is_string());
        let formatted = result.as_str().unwrap();
        assert_eq!(formatted, "2022-07-06T13:20:06Z");

        // Test with invalid datetime string
        let args = [DataValue::string(&arena, "invalid")];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_datetime_operator(&args, &dummy_context, &arena);
        assert!(result.is_err());

        // Test with already a datetime
        let dt = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 13, 20, 6)
            .unwrap();
        let args = [DataValue::datetime(dt)];
        let dummy_data = arena.alloc(DataValue::Null);
        let dummy_context = crate::context::EvalContext::new(dummy_data);
        let result = eval_datetime_operator(&args, &dummy_context, &arena).unwrap();

        // Check that it returns a formatted string
        assert!(result.is_string());
        let formatted = result.as_str().unwrap();
        assert_eq!(formatted, "2022-07-06T13:20:06Z");
    }
}
