//! Datetime and duration utilities.
//!
//! This module provides functions for parsing and formatting datetime and duration values.

use chrono::Datelike;
use chrono::{DateTime, Duration, FixedOffset, ParseError};
use lazy_static::lazy_static;
use regex::Regex;
use std::error::Error;

/// Parses an RFC3339 datetime string into a chrono DateTime with preserved timezone offset.
///
/// This function preserves the original timezone information from the input string.
/// If the input has a timezone offset (like "+05:00"), it will be preserved.
/// If the input ends with "Z" (UTC), it will be treated as +00:00 offset.
pub fn parse_datetime(datetime_str: &str) -> Result<DateTime<FixedOffset>, ParseError> {
    DateTime::parse_from_rfc3339(datetime_str)
}

/// Parses a duration string into a `chrono::Duration`.
///
/// Accepts two formats:
/// - 1d:2h:3m:4s (custom format with days, hours, minutes, seconds)
/// - P1DT2H3M4S (ISO8601 duration format)
pub fn parse_duration(duration_str: &str) -> Result<Duration, Box<dyn Error>> {
    // First, try our custom format
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"(?:(\d+)d)?:?(?:(\d+)h)?:?(?:(\d+)m)?:?(?:(\d+)s)?").unwrap();
    }

    if let Some(caps) = RE.captures(duration_str) {
        let days = caps
            .get(1)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));
        let hours = caps
            .get(2)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));
        let minutes = caps
            .get(3)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));
        let seconds = caps
            .get(4)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));

        if days > 0 || hours > 0 || minutes > 0 || seconds > 0 {
            return Ok(Duration::days(days)
                + Duration::hours(hours)
                + Duration::minutes(minutes)
                + Duration::seconds(seconds));
        }
    }

    // Then try ISO8601 format
    parse_iso8601_duration(duration_str)
}

/// Parses an ISO8601 duration string like "P1DT2H3M4S".
fn parse_iso8601_duration(duration_str: &str) -> Result<Duration, Box<dyn Error>> {
    lazy_static! {
        static ref ISO_RE: Regex =
            Regex::new(r"P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?)?").unwrap();
    }

    if let Some(caps) = ISO_RE.captures(duration_str) {
        let days = caps
            .get(1)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));
        let hours = caps
            .get(2)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));
        let minutes = caps
            .get(3)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));
        let seconds = caps
            .get(4)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));

        if days > 0 || hours > 0 || minutes > 0 || seconds > 0 {
            return Ok(Duration::days(days)
                + Duration::hours(hours)
                + Duration::minutes(minutes)
                + Duration::seconds(seconds));
        }
    }

    Err("Invalid duration format".into())
}

/// Formats a Duration into a string like "1d:2h:3m:4s".
pub fn format_duration(duration: &Duration) -> String {
    let total_seconds = duration.num_seconds();
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if days > 0 {
        format!("{days}d:{hours}h:{minutes}m:{seconds}s")
    } else if hours > 0 {
        format!("{hours}h:{minutes}m:{seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m:{seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// Calculates the difference between two datetimes in the specified unit.
pub fn date_diff(dt1: &DateTime<FixedOffset>, dt2: &DateTime<FixedOffset>, unit: &str) -> i64 {
    match unit {
        "year" | "years" => {
            let years_diff = dt2.year() - dt1.year();
            years_diff as i64
        }
        "month" | "months" => {
            let years_diff = dt2.year() - dt1.year();
            let months_diff = dt2.month() as i32 - dt1.month() as i32;
            (years_diff * 12 + months_diff) as i64
        }
        "day" | "days" => {
            let duration = dt2.signed_duration_since(*dt1);
            duration.num_days()
        }
        "hour" | "hours" => {
            let duration = dt2.signed_duration_since(*dt1);
            duration.num_hours()
        }
        "minute" | "minutes" => {
            let duration = dt2.signed_duration_since(*dt1);
            duration.num_minutes()
        }
        "second" | "seconds" => {
            let duration = dt2.signed_duration_since(*dt1);
            duration.num_seconds()
        }
        _ => 0, // Unknown unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono::Timelike;

    #[test]
    fn test_parse_datetime() {
        // Test ISO8601 format
        let dt = parse_datetime("2022-07-06T13:20:06Z").unwrap();
        assert_eq!(dt.year(), 2022);
        assert_eq!(dt.month(), 7);
        assert_eq!(dt.day(), 6);
        assert_eq!(dt.hour(), 13);
        assert_eq!(dt.minute(), 20);
        assert_eq!(dt.second(), 6);

        // Test with timezone offset
        let dt = parse_datetime("2022-07-06T13:20:06+05:00").unwrap();
        assert_eq!(dt.year(), 2022);
        assert_eq!(dt.month(), 7);
        assert_eq!(dt.day(), 6);
        assert_eq!(dt.hour(), 13);
        assert_eq!(dt.minute(), 20);
        assert_eq!(dt.second(), 6);
        assert_eq!(dt.offset().local_minus_utc(), 5 * 3600); // +5 hours in seconds
    }

    #[test]
    fn test_parse_duration() {
        // Test our custom format
        let duration = parse_duration("1d:2h:3m:4s").unwrap();
        assert_eq!(duration.num_days(), 1);
        assert_eq!(duration.num_hours() % 24, 2);
        assert_eq!(duration.num_minutes() % 60, 3);
        assert_eq!(duration.num_seconds() % 60, 4);

        // Test partial format
        let duration = parse_duration("2h:30m").unwrap();
        assert_eq!(duration.num_hours(), 2);
        assert_eq!(duration.num_minutes() % 60, 30);

        // Test ISO8601 format
        let duration = parse_duration("P1DT2H3M4S").unwrap();
        assert_eq!(duration.num_days(), 1);
        assert_eq!(duration.num_hours() % 24, 2);
        assert_eq!(duration.num_minutes() % 60, 3);
        assert_eq!(duration.num_seconds() % 60, 4);
    }

    #[test]
    fn test_format_duration() {
        let duration =
            Duration::days(1) + Duration::hours(2) + Duration::minutes(3) + Duration::seconds(4);

        let formatted = format_duration(&duration);
        assert_eq!(formatted, "1d:2h:3m:4s");

        // Test with only hours
        let duration = Duration::hours(5) + Duration::minutes(30);
        let formatted = format_duration(&duration);
        assert_eq!(formatted, "5h:30m:0s");

        // Test with only minutes
        let duration = Duration::minutes(45);
        let formatted = format_duration(&duration);
        assert_eq!(formatted, "45m:0s");

        // Test with only seconds
        let duration = Duration::seconds(20);
        let formatted = format_duration(&duration);
        assert_eq!(formatted, "20s");
    }

    #[test]
    fn test_date_diff() {
        // Test days difference
        let dt1 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 0, 0, 0)
            .unwrap();
        let dt2 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 7, 0, 0, 0)
            .unwrap();

        assert_eq!(date_diff(&dt1, &dt2, "days"), 1);
        assert_eq!(date_diff(&dt2, &dt1, "days"), -1);

        // Test hours difference
        let dt1 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 10, 0, 0)
            .unwrap();
        let dt2 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 15, 0, 0)
            .unwrap();

        assert_eq!(date_diff(&dt1, &dt2, "hours"), 5);

        // Test months difference
        let dt1 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 15, 0, 0, 0)
            .unwrap();
        let dt2 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 10, 15, 0, 0, 0)
            .unwrap();

        assert_eq!(date_diff(&dt1, &dt2, "months"), 3);

        // Test years difference
        let dt1 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2020, 7, 6, 0, 0, 0)
            .unwrap();
        let dt2 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 0, 0, 0)
            .unwrap();

        assert_eq!(date_diff(&dt1, &dt2, "years"), 2);
    }
}
