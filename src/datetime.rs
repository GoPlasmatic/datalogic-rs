use chrono::{DateTime, Datelike, Duration, NaiveDateTime, Timelike, Utc};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DataDateTime {
    pub dt: DateTime<Utc>,
    pub original_offset: Option<i32>, // Store original timezone offset in seconds
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DataDuration(pub Duration);

impl DataDateTime {
    /// Saturate datetime to max/min bounds on overflow
    fn saturate_datetime(dt: Option<DateTime<Utc>>, is_positive: bool) -> DateTime<Utc> {
        dt.unwrap_or(if is_positive {
            DateTime::<Utc>::MAX_UTC
        } else {
            DateTime::<Utc>::MIN_UTC
        })
    }

    pub fn parse(s: &str) -> Option<Self> {
        // Try parsing as RFC3339/ISO8601
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            let offset = dt.offset().local_minus_utc();
            return Some(DataDateTime {
                dt: dt.with_timezone(&Utc),
                original_offset: Some(offset),
            });
        }

        // Try parsing without timezone (assume UTC)
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            return Some(DataDateTime {
                dt: DateTime::from_naive_utc_and_offset(naive, Utc),
                original_offset: None,
            });
        }

        None
    }

    pub fn parse_with_format(s: &str, format: &str) -> Option<Self> {
        // Parse with custom format
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, format) {
            return Some(DataDateTime {
                dt: DateTime::from_naive_utc_and_offset(naive, Utc),
                original_offset: None,
            });
        }

        // Try date-only formats
        if let Ok(date) = chrono::NaiveDate::parse_from_str(s, format) {
            let datetime = date.and_hms_opt(0, 0, 0)?;
            return Some(DataDateTime {
                dt: DateTime::from_naive_utc_and_offset(datetime, Utc),
                original_offset: None,
            });
        }

        None
    }

    pub fn format(&self, format: &str) -> String {
        // Handle special format codes for timezone
        if format == "z" {
            // Return original timezone offset if we have it
            if let Some(offset_secs) = self.original_offset {
                let hours = offset_secs / 3600;
                let minutes = (offset_secs % 3600) / 60;
                return format!("{:+03}{:02}", hours, minutes);
            }
            return "+0000".to_string();
        }

        self.dt.format(format).to_string()
    }

    pub fn to_iso_string(&self) -> String {
        self.dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    pub fn year(&self) -> i32 {
        self.dt.year()
    }

    pub fn month(&self) -> u32 {
        self.dt.month()
    }

    pub fn day(&self) -> u32 {
        self.dt.day()
    }

    pub fn hour(&self) -> u32 {
        self.dt.hour()
    }

    pub fn minute(&self) -> u32 {
        self.dt.minute()
    }

    pub fn second(&self) -> u32 {
        self.dt.second()
    }

    pub fn timestamp(&self) -> i64 {
        self.dt.timestamp()
    }

    pub fn add_duration(&self, duration: &DataDuration) -> DataDateTime {
        let dt = Self::saturate_datetime(
            self.dt.checked_add_signed(duration.0),
            duration.0.num_seconds() > 0,
        );
        DataDateTime {
            dt,
            original_offset: self.original_offset,
        }
    }

    pub fn sub_duration(&self, duration: &DataDuration) -> DataDateTime {
        let dt = Self::saturate_datetime(
            self.dt.checked_sub_signed(duration.0),
            duration.0.num_seconds() < 0,
        );
        DataDateTime {
            dt,
            original_offset: self.original_offset,
        }
    }

    pub fn diff(&self, other: &DataDateTime) -> DataDuration {
        DataDuration(self.dt - other.dt)
    }

    pub fn diff_in_unit(&self, other: &DataDateTime, unit: &str) -> f64 {
        let duration = self.dt - other.dt;
        match unit {
            "days" => duration.num_days() as f64,
            "hours" => duration.num_hours() as f64,
            "minutes" => duration.num_minutes() as f64,
            "seconds" => duration.num_seconds() as f64,
            "milliseconds" => duration.num_milliseconds() as f64,
            _ => 0.0,
        }
    }
}

impl DataDuration {
    /// Saturate duration to max/min bounds on overflow
    fn saturate_duration(seconds: f64) -> DataDuration {
        if !seconds.is_finite() || seconds > i64::MAX as f64 / 1000.0 {
            DataDuration(Duration::MAX)
        } else if seconds < i64::MIN as f64 / 1000.0 {
            DataDuration(Duration::MIN)
        } else {
            Duration::try_seconds(seconds as i64)
                .map(DataDuration)
                .unwrap_or(DataDuration(Duration::MAX))
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        // Parse duration format like "1d:2h:3m:4s" or "1d" or "2h30m"
        let mut days = 0i64;
        let mut hours = 0i64;
        let mut minutes = 0i64;
        let mut seconds = 0i64;

        // Check for full format "1d:2h:3m:4s"
        if s.contains(':') {
            let parts: Vec<&str> = s.split(':').collect();
            for part in parts {
                if let Some(stripped) = part.strip_suffix('d') {
                    days = stripped.parse().ok()?;
                } else if let Some(stripped) = part.strip_suffix('h') {
                    hours = stripped.parse().ok()?;
                } else if let Some(stripped) = part.strip_suffix('m') {
                    minutes = stripped.parse().ok()?;
                } else if let Some(stripped) = part.strip_suffix('s') {
                    seconds = stripped.parse().ok()?;
                }
            }
        } else {
            // Parse compact format like "1d2h30m"
            let mut current_num = String::new();
            for ch in s.chars() {
                if ch.is_ascii_digit() {
                    current_num.push(ch);
                } else if ch == 'd' {
                    days = current_num.parse().ok()?;
                    current_num.clear();
                } else if ch == 'h' {
                    hours = current_num.parse().ok()?;
                    current_num.clear();
                } else if ch == 'm' {
                    minutes = current_num.parse().ok()?;
                    current_num.clear();
                } else if ch == 's' {
                    seconds = current_num.parse().ok()?;
                    current_num.clear();
                }
            }
        }

        // Only return a duration if we found at least one unit
        if days == 0
            && hours == 0
            && minutes == 0
            && seconds == 0
            && !s.contains(['d', 'h', 'm', 's'])
        {
            return None;
        }

        // Use checked arithmetic to prevent overflow
        let total_seconds = days
            .checked_mul(86400)?
            .checked_add(hours.checked_mul(3600)?)?
            .checked_add(minutes.checked_mul(60)?)?
            .checked_add(seconds)?;

        // Chrono's Duration::seconds will panic if value is out of bounds
        // Check bounds before creating Duration
        if !(i64::MIN / 1000..=i64::MAX / 1000).contains(&total_seconds) {
            // Saturate at max/min duration that chrono can handle
            if total_seconds > 0 {
                Some(DataDuration(Duration::MAX))
            } else {
                Some(DataDuration(Duration::MIN))
            }
        } else {
            Duration::try_seconds(total_seconds).map(DataDuration)
        }
    }

    pub fn days(&self) -> i64 {
        self.0.num_days()
    }

    pub fn hours(&self) -> i64 {
        (self.0.num_seconds() % 86400) / 3600
    }

    pub fn minutes(&self) -> i64 {
        (self.0.num_seconds() % 3600) / 60
    }

    pub fn seconds(&self) -> i64 {
        self.0.num_seconds() % 60
    }

    pub fn total_seconds(&self) -> i64 {
        self.0.num_seconds()
    }

    pub fn multiply(&self, factor: f64) -> DataDuration {
        let result = self.0.num_seconds() as f64 * factor;
        if !result.is_finite() {
            DataDuration(self.0)
        } else {
            Self::saturate_duration(result)
        }
    }

    pub fn divide(&self, divisor: f64) -> DataDuration {
        if divisor == 0.0 || divisor.abs() < f64::EPSILON {
            return DataDuration(Duration::MAX);
        }

        let result = self.0.num_seconds() as f64 / divisor;
        if !result.is_finite() {
            DataDuration(self.0)
        } else {
            Self::saturate_duration(result)
        }
    }

    pub fn add(&self, other: &DataDuration) -> DataDuration {
        self.0
            .checked_add(&other.0)
            .map(DataDuration)
            .unwrap_or_else(|| {
                if self.0.num_seconds() > 0 || other.0.num_seconds() > 0 {
                    DataDuration(Duration::MAX)
                } else {
                    DataDuration(Duration::MIN)
                }
            })
    }

    pub fn sub(&self, other: &DataDuration) -> DataDuration {
        self.0
            .checked_sub(&other.0)
            .map(DataDuration)
            .unwrap_or_else(|| {
                if self.0.num_seconds() > other.0.num_seconds() {
                    DataDuration(Duration::MAX)
                } else {
                    DataDuration(Duration::MIN)
                }
            })
    }
}

impl fmt::Display for DataDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total_seconds = self.0.num_seconds();
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        write!(f, "{}d:{}h:{}m:{}s", days, hours, minutes, seconds)
    }
}

// Helper to check if a Value is a datetime object
pub fn is_datetime_object(value: &Value) -> bool {
    if let Value::Object(map) = value {
        map.contains_key("datetime")
    } else {
        false
    }
}

// Helper to check if a Value is a duration/timestamp object
pub fn is_duration_object(value: &Value) -> bool {
    if let Value::Object(map) = value {
        map.contains_key("timestamp")
    } else {
        false
    }
}

// Extract datetime from object
pub fn extract_datetime(value: &Value) -> Option<DataDateTime> {
    if let Value::Object(map) = value
        && let Some(Value::String(s)) = map.get("datetime")
    {
        return DataDateTime::parse(s);
    }
    None
}

// Extract duration from object
pub fn extract_duration(value: &Value) -> Option<DataDuration> {
    if let Value::Object(map) = value
        && let Some(Value::String(s)) = map.get("timestamp")
    {
        return DataDuration::parse(s);
    }
    None
}
