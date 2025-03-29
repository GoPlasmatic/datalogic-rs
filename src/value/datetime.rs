//! Datetime and duration utilities.
//!
//! This module provides functions for parsing and formatting datetime and duration values.

use chrono::Datelike;
use chrono::{DateTime, Duration, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use std::error::Error;

/// Parses a datetime string into a `chrono::DateTime<Utc>`.
pub fn parse_datetime(datetime_str: &str) -> Result<DateTime<Utc>, Box<dyn Error>> {
    // Try to parse as RFC3339/ISO8601 format
    DateTime::parse_from_rfc3339(datetime_str)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Try as ISO8601 without time
            chrono::NaiveDate::parse_from_str(datetime_str, "%Y-%m-%d")
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        })
        .or_else(|_| {
            // Try other common formats (could add more as needed)
            chrono::NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.and_utc())
        })
        .map_err(|e| e.into())
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
        format!("{}d:{}h:{}m:{}s", days, hours, minutes, seconds)
    } else if hours > 0 {
        format!("{}h:{}m:{}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m:{}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Calculates the difference between two datetimes in the specified unit.
///
/// Supported units:
/// - "years", "year", "y"
/// - "months", "month", "M"
/// - "days", "day", "d"
/// - "hours", "hour", "h"
/// - "minutes", "minute", "m"
/// - "seconds", "second", "s"
pub fn date_diff(dt1: &DateTime<Utc>, dt2: &DateTime<Utc>, unit: &str) -> i64 {
    let duration = *dt1 - *dt2;

    match unit.to_lowercase().as_str() {
        "years" | "year" | "y" => {
            let years = dt1.year() - dt2.year();
            // Adjust for not having completed a full year
            if dt1.month() < dt2.month() || (dt1.month() == dt2.month() && dt1.day() < dt2.day()) {
                (years - 1) as i64
            } else {
                years as i64
            }
        }
        "months" | "month" | "M" => {
            let year_diff = dt1.year() - dt2.year();
            let month_diff = dt1.month() as i32 - dt2.month() as i32;
            let total_months = year_diff * 12 + month_diff;

            // Adjust for not having completed a full month
            if dt1.day() < dt2.day() {
                (total_months - 1) as i64
            } else {
                total_months as i64
            }
        }
        "days" | "day" | "d" => duration.num_days(),
        "hours" | "hour" | "h" => duration.num_hours(),
        "minutes" | "minute" | "m" => duration.num_minutes(),
        "seconds" | "second" | "s" => duration.num_seconds(),
        _ => duration.num_seconds(), // Default to seconds
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

        // Test date-only ISO8601
        let dt = parse_datetime("2022-07-06").unwrap();
        assert_eq!(dt.year(), 2022);
        assert_eq!(dt.month(), 7);
        assert_eq!(dt.day(), 6);
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);
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
        let dt1 = Utc.with_ymd_and_hms(2022, 7, 6, 0, 0, 0).unwrap();
        let dt2 = Utc.with_ymd_and_hms(2022, 7, 7, 0, 0, 0).unwrap();

        assert_eq!(date_diff(&dt2, &dt1, "days"), 1);
        assert_eq!(date_diff(&dt1, &dt2, "days"), -1);

        // Test hours difference
        let dt1 = Utc.with_ymd_and_hms(2022, 7, 6, 10, 0, 0).unwrap();
        let dt2 = Utc.with_ymd_and_hms(2022, 7, 6, 15, 0, 0).unwrap();

        assert_eq!(date_diff(&dt2, &dt1, "hours"), 5);

        // Test months difference
        let dt1 = Utc.with_ymd_and_hms(2022, 7, 15, 0, 0, 0).unwrap();
        let dt2 = Utc.with_ymd_and_hms(2022, 10, 15, 0, 0, 0).unwrap();

        assert_eq!(date_diff(&dt2, &dt1, "months"), 3);

        // Test years difference
        let dt1 = Utc.with_ymd_and_hms(2020, 7, 6, 0, 0, 0).unwrap();
        let dt2 = Utc.with_ymd_and_hms(2022, 7, 6, 0, 0, 0).unwrap();

        assert_eq!(date_diff(&dt2, &dt1, "years"), 2);
    }
}
