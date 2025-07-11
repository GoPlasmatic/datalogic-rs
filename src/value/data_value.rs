//! Core DataValue implementation.
//!
//! This module provides the DataValue enum, which is a memory-efficient
//! representation of data values that leverages arena allocation.

use super::number::NumberValue;
use crate::arena::DataArena;
use chrono::{DateTime, Duration, FixedOffset};
use std::cmp::Ordering;
use std::fmt;

/// A memory-efficient value type that leverages arena allocation.
///
/// This replaces the direct dependency on `serde_json::Value` with a custom
/// implementation optimized for rule evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue<'a> {
    /// Represents a null value
    Null,

    /// Represents a boolean value
    Bool(bool),

    /// Represents a numeric value (integer or floating point)
    Number(NumberValue),

    /// Represents a string value (arena-allocated)
    String(&'a str),

    /// Represents an array of values (arena-allocated)
    Array(&'a [DataValue<'a>]),

    /// Represents an object with key-value pairs (arena-allocated)
    Object(&'a [(&'a str, DataValue<'a>)]),

    /// Represents a datetime value with timezone information
    DateTime(DateTime<FixedOffset>),

    /// Represents a duration value
    Duration(Duration),
}

impl<'a> DataValue<'a> {
    /// Creates a null value.
    pub fn null() -> Self {
        DataValue::Null
    }

    /// Creates a boolean value.
    pub fn bool(value: bool) -> Self {
        DataValue::Bool(value)
    }

    /// Creates an integer value.
    pub fn integer(value: i64) -> Self {
        DataValue::Number(NumberValue::Integer(value))
    }

    /// Creates a floating-point value.
    pub fn float(value: f64) -> Self {
        DataValue::Number(NumberValue::from_f64(value))
    }

    /// Creates a string value.
    ///
    /// If the string is empty, returns a string value with the preallocated empty string.
    pub fn string(arena: &'a DataArena, value: &str) -> Self {
        if value.is_empty() {
            // Use the preallocated empty string
            DataValue::String(arena.empty_string())
        } else {
            DataValue::String(arena.alloc_str(value))
        }
    }

    /// Creates a datetime value from a `chrono::DateTime<FixedOffset>`.
    pub fn datetime(value: DateTime<FixedOffset>) -> Self {
        DataValue::DateTime(value)
    }

    /// Creates a duration value from a `chrono::Duration`.
    pub fn duration(value: Duration) -> Self {
        DataValue::Duration(value)
    }

    /// Creates an array value.
    ///
    /// If the array is empty, returns a value with the preallocated empty array.
    /// For small arrays (up to 8 elements), uses an optimized allocation method.
    pub fn array(arena: &'a DataArena, values: &[DataValue<'a>]) -> Self {
        if values.is_empty() {
            // Use the preallocated empty array
            DataValue::Array(arena.empty_array())
        } else {
            // Use the standard allocation for larger arrays
            DataValue::Array(arena.alloc_data_value_slice(values))
        }
    }

    /// Creates an object value.
    ///
    /// If the entries array is empty, returns an object with an empty entries array.
    pub fn object(arena: &'a DataArena, entries: &[(&'a str, DataValue<'a>)]) -> Self {
        DataValue::Object(arena.alloc_object_entries(entries))
    }

    /// Returns true if the value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, DataValue::Null)
    }

    /// Returns true if the value is a boolean.
    pub fn is_bool(&self) -> bool {
        matches!(self, DataValue::Bool(_))
    }

    /// Returns true if the value is a number.
    pub fn is_number(&self) -> bool {
        matches!(self, DataValue::Number(_))
    }

    /// Returns true if the value is a string.
    pub fn is_string(&self) -> bool {
        matches!(self, DataValue::String(_))
    }

    /// Returns true if the value is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, DataValue::Array(_))
    }

    /// Returns true if the value is an object.
    pub fn is_object(&self) -> bool {
        matches!(self, DataValue::Object(_))
    }

    /// Returns true if the value is a datetime.
    pub fn is_datetime(&self) -> bool {
        matches!(self, DataValue::DateTime(_))
    }

    /// Returns true if the value is a duration.
    pub fn is_duration(&self) -> bool {
        matches!(self, DataValue::Duration(_))
    }

    /// Returns the value as a boolean, if it is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            DataValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the value as an i64, if it is a number that can be represented as an i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            DataValue::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    /// Returns the value as an f64, if it is a number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DataValue::Number(n) => Some(n.as_f64()),
            _ => None,
        }
    }

    /// Returns the value as a string slice, if it is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            DataValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the value as an array slice, if it is an array.
    pub fn as_array(&self) -> Option<&[DataValue<'a>]> {
        match self {
            DataValue::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Returns the value as a datetime, if it is a datetime.
    pub fn as_datetime(&self) -> Option<&DateTime<FixedOffset>> {
        match self {
            DataValue::DateTime(dt) => Some(dt),
            _ => None,
        }
    }

    /// Returns the value as a duration, if it is a duration.
    pub fn as_duration(&self) -> Option<&Duration> {
        match self {
            DataValue::Duration(d) => Some(d),
            _ => None,
        }
    }

    /// Returns the value as an object slice, if it is an object.
    pub fn as_object(&self) -> Option<&[(&'a str, DataValue<'a>)]> {
        match self {
            DataValue::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Coerces the value to a boolean.
    ///
    /// The coercion follows JSON Logic rules:
    /// - `null` is `false`
    /// - `false` is `false`
    /// - Empty string is `false`
    /// - Empty array is `false`
    /// - Empty object is `false`
    /// - `0` is `false`
    /// - Everything else is `true`
    #[inline]
    pub fn coerce_to_bool(&self) -> bool {
        match self {
            // Fast path for common cases
            DataValue::Bool(b) => *b,
            DataValue::Null => false,

            // Number case - only 0 is false
            DataValue::Number(n) => {
                // Fast path for integers
                if let NumberValue::Integer(i) = n {
                    *i != 0
                } else {
                    n.as_f64() != 0.0
                }
            }

            // String case - only empty string is false
            DataValue::String(s) => !s.is_empty(),

            // Array case - only empty array is false
            DataValue::Array(items) => !items.is_empty(),

            // Object case - only empty object is false
            DataValue::Object(items) => !items.is_empty(),

            // DateTime always coerces to true
            DataValue::DateTime(_) => true,

            // Duration is false if zero
            DataValue::Duration(d) => !d.is_zero(),
        }
    }

    /// Coerces the value to a number according to JSONLogic rules.
    #[inline]
    pub fn coerce_to_number(&self) -> Option<NumberValue> {
        match self {
            // Fast paths for common cases
            DataValue::Number(n) => Some(*n),
            DataValue::Bool(b) => Some(NumberValue::Integer(if *b { 1 } else { 0 })),
            DataValue::Null => Some(NumberValue::Integer(0)),

            DataValue::String(s) => {
                // Fast path for empty strings
                if s.is_empty() {
                    return Some(NumberValue::Integer(0));
                }

                // Fast path for simple integers
                let mut is_integer = true;
                let mut value: i64 = 0;
                let mut negative = false;
                let bytes = s.as_bytes();

                // Check for negative sign
                let mut i = 0;
                if !bytes.is_empty() && bytes[0] == b'-' {
                    negative = true;
                    i = 1;
                }

                // Parse digits
                while i < bytes.len() {
                    let b = bytes[i];
                    if b.is_ascii_digit() {
                        // Check for overflow
                        if value > i64::MAX / 10 {
                            is_integer = false;
                            break;
                        }
                        value = value * 10 + (b - b'0') as i64;
                    } else {
                        is_integer = false;
                        break;
                    }
                    i += 1;
                }

                if is_integer {
                    if negative {
                        value = -value;
                    }
                    return Some(NumberValue::Integer(value));
                }

                // Fall back to standard parsing for more complex cases
                if let Ok(i) = s.parse::<i64>() {
                    Some(NumberValue::Integer(i))
                } else if let Ok(f) = s.parse::<f64>() {
                    Some(NumberValue::Float(f))
                } else {
                    None
                }
            }

            // DateTime can be converted to Unix timestamp
            DataValue::DateTime(dt) => {
                let timestamp = dt.timestamp_millis() as f64 / 1000.0;
                Some(NumberValue::Float(timestamp))
            }

            // Duration can be converted to total seconds
            DataValue::Duration(d) => {
                let total_seconds = d.num_seconds();
                Some(NumberValue::Integer(total_seconds))
            }

            DataValue::Array(_) => None,

            DataValue::Object(_) => None,
        }
    }

    /// Coerces the value to a string according to JSONLogic rules.
    pub fn coerce_to_string(&self, arena: &'a DataArena) -> DataValue<'a> {
        match self {
            DataValue::Null => DataValue::String(arena.alloc_str("null")),
            DataValue::Bool(b) => {
                DataValue::String(arena.alloc_str(if *b { "true" } else { "false" }))
            }
            DataValue::Number(n) => DataValue::String(arena.alloc_str(&n.to_string())),
            DataValue::String(s) => DataValue::String(s),
            DataValue::Array(a) => {
                let mut result = String::new();
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    if let DataValue::String(s) = v.coerce_to_string(arena) {
                        result.push_str(s);
                    }
                }
                DataValue::String(arena.alloc_str(&result))
            }
            DataValue::Object(_) => DataValue::String(arena.alloc_str("[object Object]")),
            DataValue::DateTime(dt) => {
                // Format with Z suffix for UTC, otherwise preserve timezone offset
                let formatted = if dt.offset().local_minus_utc() == 0 {
                    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                } else {
                    dt.to_rfc3339()
                };
                DataValue::String(arena.alloc_str(&formatted))
            }
            DataValue::Duration(d) => {
                // Format duration as 1d:2h:3m:4s
                let days = d.num_days();
                let hours = d.num_hours() % 24;
                let minutes = d.num_minutes() % 60;
                let seconds = d.num_seconds() % 60;
                let formatted = format!("{days}d:{hours}h:{minutes}m:{seconds}s");
                DataValue::String(arena.alloc_str(&formatted))
            }
        }
    }

    /// Gets a value from an object by key.
    pub fn get(&self, key: &str) -> Option<&DataValue<'a>> {
        match self {
            DataValue::Object(entries) => entries
                .binary_search_by_key(&key, |&(k, _)| k)
                .ok()
                .map(|idx| &entries[idx].1),
            _ => None,
        }
    }

    /// Gets a value from an array by index.
    pub fn get_index(&self, index: usize) -> Option<&DataValue<'a>> {
        match self {
            DataValue::Array(elements) => elements.get(index),
            _ => None,
        }
    }

    /// Returns a string representation of the type of this value.
    pub fn type_name(&self) -> &'static str {
        match self {
            DataValue::Null => "null",
            DataValue::Bool(_) => "boolean",
            DataValue::Number(_) => "number",
            DataValue::String(_) => "string",
            DataValue::Array(_) => "array",
            DataValue::Object(_) => "object",
            DataValue::DateTime(_) => "datetime",
            DataValue::Duration(_) => "duration",
        }
    }

    /// Checks if this value equals another value, with type coercion.
    pub fn equals(&self, other: &DataValue<'a>) -> bool {
        match (self, other) {
            // Same types use direct comparison
            (DataValue::Null, DataValue::Null) => true,
            (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
            (DataValue::Number(a), DataValue::Number(b)) => a == b,

            // Fast path for string comparison - avoid unnecessary allocations
            (DataValue::String(a), DataValue::String(b)) => {
                // First check if the pointers are the same (interned strings)
                if std::ptr::eq(*a as *const str, *b as *const str) {
                    return true;
                }

                // Then check if the lengths are different (quick rejection)
                if a.len() != b.len() {
                    return false;
                }

                // Finally, compare the actual strings
                a == b
            }

            // DateTime and Duration direct comparisons
            (DataValue::DateTime(a), DataValue::DateTime(b)) => a == b,
            (DataValue::Duration(a), DataValue::Duration(b)) => a == b,

            // DateTime to String coercion
            (DataValue::DateTime(dt), DataValue::String(s)) => {
                let formatted = dt.to_rfc3339();
                &formatted == s
            }
            (DataValue::String(s), DataValue::DateTime(dt)) => {
                let formatted = dt.to_rfc3339();
                s == &formatted
            }

            // Duration to String coercion
            (DataValue::Duration(dur), DataValue::String(s)) => {
                let days = dur.num_days();
                let hours = dur.num_hours() % 24;
                let minutes = dur.num_minutes() % 60;
                let seconds = dur.num_seconds() % 60;
                let formatted = format!("{days}d:{hours}h:{minutes}m:{seconds}s");
                &formatted == s
            }
            (DataValue::String(s), DataValue::Duration(dur)) => {
                let days = dur.num_days();
                let hours = dur.num_hours() % 24;
                let minutes = dur.num_minutes() % 60;
                let seconds = dur.num_seconds() % 60;
                let formatted = format!("{days}d:{hours}h:{minutes}m:{seconds}s");
                s == &formatted
            }

            // Different types with coercion
            (DataValue::Null, DataValue::Bool(b)) => !b,
            (DataValue::Bool(a), DataValue::Null) => !a,

            (DataValue::Number(a), DataValue::String(_)) => {
                match other.coerce_to_number() {
                    Some(b_value) => {
                        let b_num = b_value.as_f64();
                        let a_num = a.as_f64();
                        // Compare with small epsilon for floating point
                        (a_num - b_num).abs() < f64::EPSILON
                    }
                    None => false, // If we can't convert string to number, they're not equal
                }
            }
            (DataValue::String(_), DataValue::Number(b)) => {
                match self.coerce_to_number() {
                    Some(a_value) => {
                        let a_num = a_value.as_f64();
                        let b_num = b.as_f64();
                        // Compare with small epsilon for floating point
                        (a_num - b_num).abs() < f64::EPSILON
                    }
                    None => false, // If we can't convert string to number, they're not equal
                }
            }

            // Arrays and objects are compared by reference
            (DataValue::Array(a), DataValue::Array(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                a.iter()
                    .zip(b.iter())
                    .all(|(a_item, b_item)| a_item.equals(b_item))
            }
            (DataValue::Object(a), DataValue::Object(b)) => {
                // Quick length check first
                if a.len() != b.len() {
                    return false;
                }

                // Check all key-value pairs in both directions
                for (a_key, a_val) in *a {
                    let found_pair = b
                        .iter()
                        .any(|(b_key, b_val)| *a_key == *b_key && a_val.equals(b_val));
                    if !found_pair {
                        return false;
                    }
                }

                // All key-value pairs matched
                true
            }

            // Other combinations are not equal
            _ => false,
        }
    }

    /// Checks if this value strictly equals another value, without type coercion.
    pub fn strict_equals(&self, other: &DataValue<'a>) -> bool {
        match (self, other) {
            (DataValue::Null, DataValue::Null) => true,
            (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
            (DataValue::Number(a), DataValue::Number(b)) => a == b,
            (DataValue::String(a), DataValue::String(b)) => a == b,
            (DataValue::DateTime(a), DataValue::DateTime(b)) => {
                // Strict equality requires same timezone AND same instant
                a.offset() == b.offset() && a.timestamp() == b.timestamp()
            }
            (DataValue::Duration(a), DataValue::Duration(b)) => a == b,
            (DataValue::Array(a), DataValue::Array(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                a.iter()
                    .zip(b.iter())
                    .all(|(a_item, b_item)| a_item.strict_equals(b_item))
            }
            (DataValue::Object(a), DataValue::Object(b)) => {
                if a.len() != b.len() {
                    return false;
                }

                // Check that all keys in a exist in b with strictly equal values
                for (a_key, a_value) in *a {
                    let mut found = false;
                    for (b_key, b_value) in *b {
                        if a_key == b_key {
                            if !a_value.strict_equals(b_value) {
                                return false;
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return false;
                    }
                }
                true
            }
            _ => false, // Different types are never strictly equal
        }
    }
}

impl PartialOrd for DataValue<'_> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            // Fast paths for common cases
            (DataValue::Number(a), DataValue::Number(b)) => a.partial_cmp(b),
            (DataValue::String(a), DataValue::String(b)) => {
                // First check if the pointers are the same (interned strings)
                if std::ptr::eq(*a as *const str, *b as *const str) {
                    return Some(Ordering::Equal);
                }

                // Then do the standard comparison
                a.partial_cmp(b)
            }
            (DataValue::Bool(a), DataValue::Bool(b)) => a.partial_cmp(b),
            (DataValue::Null, DataValue::Null) => Some(Ordering::Equal),

            // DateTime and Duration comparisons
            (DataValue::DateTime(a), DataValue::DateTime(b)) => {
                // Compare instants in time regardless of timezone
                a.timestamp().partial_cmp(&b.timestamp())
            }
            (DataValue::Duration(a), DataValue::Duration(b)) => a.partial_cmp(b),

            (DataValue::Array(a), DataValue::Array(b)) => {
                // Fast path for empty arrays
                if a.is_empty() && b.is_empty() {
                    return Some(Ordering::Equal);
                }

                // Fast path for different length arrays
                if a.len() != b.len() {
                    return a.len().partial_cmp(&b.len());
                }

                // Compare arrays lexicographically
                for i in 0..a.len() {
                    match a[i].partial_cmp(&b[i]) {
                        Some(Ordering::Equal) => continue,
                        other => return other,
                    }
                }
                Some(Ordering::Equal)
            }

            // Mixed types: convert to common type for comparison
            (DataValue::Number(a), DataValue::String(b)) => {
                if let Ok(b_num) = b.parse::<f64>() {
                    let a_f64 = match a {
                        NumberValue::Integer(i) => *i as f64,
                        NumberValue::Float(f) => *f,
                    };

                    if a_f64 > b_num {
                        Some(Ordering::Greater)
                    } else if a_f64 < b_num {
                        Some(Ordering::Less)
                    } else {
                        Some(Ordering::Equal)
                    }
                } else {
                    None
                }
            }
            (DataValue::String(a), DataValue::Number(b)) => {
                if let Ok(a_num) = a.parse::<f64>() {
                    let b_f64 = match b {
                        NumberValue::Integer(i) => *i as f64,
                        NumberValue::Float(f) => *f,
                    };

                    if a_num > b_f64 {
                        Some(Ordering::Greater)
                    } else if a_num < b_f64 {
                        Some(Ordering::Less)
                    } else {
                        Some(Ordering::Equal)
                    }
                } else {
                    None
                }
            }

            // Other combinations are not comparable
            _ => None,
        }
    }
}

impl fmt::Display for DataValue<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataValue::Null => write!(f, "null"),
            DataValue::Bool(b) => write!(f, "{b}"),
            DataValue::Number(n) => write!(f, "{n}"),
            DataValue::String(s) => write!(f, "\"{}\"", s.replace('"', "\\\"")),
            DataValue::Array(a) => {
                write!(f, "[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            DataValue::Object(o) => {
                write!(f, "{{")?;
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{k}\": {v}")?;
                }
                write!(f, "}}")
            }
            DataValue::DateTime(dt) => {
                // Format with Z suffix for UTC, otherwise preserve timezone offset
                let formatted = if dt.offset().local_minus_utc() == 0 {
                    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                } else {
                    dt.to_rfc3339()
                };
                write!(f, "{formatted}")
            }
            DataValue::Duration(d) => {
                let days = d.num_days();
                let hours = d.num_hours() % 24;
                let minutes = d.num_minutes() % 60;
                let seconds = d.num_seconds() % 60;
                write!(f, "\"{days}d:{hours}h:{minutes}m:{seconds}s\"")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;
    use chrono::TimeZone;

    #[test]
    fn test_data_value_creation() {
        let arena = DataArena::new();

        let null = DataValue::null();
        let boolean = DataValue::bool(true);
        let integer = DataValue::integer(42);
        let float = DataValue::float(3.14);
        let string = DataValue::string(&arena, "hello");

        assert!(null.is_null());
        assert!(boolean.is_bool());
        assert!(integer.is_number());
        assert!(float.is_number());
        assert!(string.is_string());

        assert_eq!(boolean.as_bool(), Some(true));
        assert_eq!(integer.as_i64(), Some(42));
        assert_eq!(float.as_f64(), Some(3.14));
        assert_eq!(string.as_str(), Some("hello"));
    }

    #[test]
    fn test_datetime_and_duration() {
        // Test datetime creation and access
        let dt = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 13, 20, 6)
            .unwrap();
        let dt_value = DataValue::datetime(dt);

        assert!(dt_value.is_datetime());
        assert_eq!(dt_value.as_datetime(), Some(&dt));

        // Test duration creation and access
        let duration =
            Duration::days(1) + Duration::hours(2) + Duration::minutes(3) + Duration::seconds(4);
        let duration_value = DataValue::duration(duration);

        assert!(duration_value.is_duration());
        assert_eq!(duration_value.as_duration(), Some(&duration));
    }

    #[test]
    fn test_array_and_object() {
        let arena = DataArena::new();

        // Create array
        let array = DataValue::array(
            &arena,
            &[
                DataValue::integer(1),
                DataValue::integer(2),
                DataValue::integer(3),
            ],
        );

        assert!(array.is_array());
        assert_eq!(array.as_array().unwrap().len(), 3);
        assert_eq!(array.get_index(1).unwrap().as_i64(), Some(2));

        // Create object
        let key1 = arena.intern_str("a");
        let key2 = arena.intern_str("b");

        let object = DataValue::object(
            &arena,
            &[(key1, DataValue::integer(1)), (key2, DataValue::integer(2))],
        );

        assert!(object.is_object());
        assert_eq!(object.as_object().unwrap().len(), 2);
        assert_eq!(object.get("a").unwrap().as_i64(), Some(1));
    }

    #[test]
    fn test_coercion() {
        let arena = DataArena::new();

        // Boolean coercion
        assert!(!DataValue::null().coerce_to_bool());
        assert!(DataValue::bool(true).coerce_to_bool());
        assert!(!DataValue::integer(0).coerce_to_bool());
        assert!(DataValue::integer(1).coerce_to_bool());
        assert!(!DataValue::string(&arena, "").coerce_to_bool());
        assert!(DataValue::string(&arena, "hello").coerce_to_bool());

        // Test datetime and duration boolean coercion
        let dt = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 13, 20, 6)
            .unwrap();
        assert!(DataValue::datetime(dt).coerce_to_bool());

        let duration = Duration::seconds(0);
        assert!(!DataValue::duration(duration).coerce_to_bool());

        let non_zero_duration = Duration::seconds(10);
        assert!(DataValue::duration(non_zero_duration).coerce_to_bool());

        // Number coercion
        assert_eq!(
            DataValue::null().coerce_to_number(),
            Some(NumberValue::Integer(0))
        );
        assert_eq!(
            DataValue::bool(true).coerce_to_number(),
            Some(NumberValue::Integer(1))
        );
        assert_eq!(
            DataValue::string(&arena, "42").coerce_to_number(),
            Some(NumberValue::Integer(42))
        );
        assert_eq!(
            DataValue::string(&arena, "3.14").coerce_to_number(),
            Some(NumberValue::Float(3.14))
        );

        // Test datetime and duration number coercion
        let dt = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 13, 20, 6)
            .unwrap();
        let dt_num = DataValue::datetime(dt).coerce_to_number();
        assert!(dt_num.is_some());

        let duration = Duration::seconds(93784); // 1d:2h:3m:4s
        assert_eq!(
            DataValue::duration(duration).coerce_to_number(),
            Some(NumberValue::Integer(93784))
        );

        // String coercion
        assert_eq!(
            DataValue::null().coerce_to_string(&arena).as_str(),
            Some("null")
        );
        assert_eq!(
            DataValue::bool(true).coerce_to_string(&arena).as_str(),
            Some("true")
        );
        assert_eq!(
            DataValue::integer(42).coerce_to_string(&arena).as_str(),
            Some("42")
        );

        // Test datetime and duration string coercion
        let dt = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 13, 20, 6)
            .unwrap();
        let dt_value = DataValue::datetime(dt).coerce_to_string(&arena);
        let dt_str = dt_value.as_str();
        assert!(dt_str.is_some());
        assert!(dt_str.unwrap().contains("2022-07-06T13:20:06"));

        let duration =
            Duration::days(1) + Duration::hours(2) + Duration::minutes(3) + Duration::seconds(4);
        let dur_value = DataValue::duration(duration).coerce_to_string(&arena);
        let dur_str = dur_value.as_str();
        assert_eq!(dur_str, Some("1d:2h:3m:4s"));
    }

    #[test]
    fn test_comparison() {
        let arena = DataArena::new();

        // Same types
        assert!(DataValue::null() == DataValue::null());
        assert!(DataValue::bool(true) > DataValue::bool(false));
        assert!(DataValue::integer(5) > DataValue::integer(3));
        assert!(DataValue::float(3.14) > DataValue::float(2.71));
        assert!(DataValue::string(&arena, "hello") == DataValue::string(&arena, "hello"));
        assert!(DataValue::string(&arena, "world") > DataValue::string(&arena, "hello"));

        // Test datetime comparison
        let dt1 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 6, 13, 20, 6)
            .unwrap();
        let dt2 = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 7, 7, 13, 20, 6)
            .unwrap();

        assert!(DataValue::datetime(dt1) < DataValue::datetime(dt2));
        assert!(DataValue::datetime(dt2) > DataValue::datetime(dt1));
        assert!(DataValue::datetime(dt1) == DataValue::datetime(dt1));

        // Test duration comparison
        let dur1 = Duration::days(1);
        let dur2 = Duration::days(2);

        assert!(DataValue::duration(dur1) < DataValue::duration(dur2));
        assert!(DataValue::duration(dur2) > DataValue::duration(dur1));
        assert!(DataValue::duration(dur1) == DataValue::duration(dur1));

        // Mixed types
        assert!(DataValue::integer(42) == DataValue::float(42.0));

        // Array comparison
        let array1 = DataValue::array(&arena, &[DataValue::integer(1), DataValue::integer(2)]);
        let array2 = DataValue::array(&arena, &[DataValue::integer(1), DataValue::integer(3)]);
        assert!(array1 < array2);
    }
}
