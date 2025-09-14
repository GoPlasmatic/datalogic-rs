use serde_json::Value;
use std::borrow::Cow;

/// Checks if a value is truthy according to JSONLogic rules
pub fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                f != 0.0 && !f.is_nan()
            } else {
                n.as_i64() != Some(0) && n.as_u64() != Some(0)
            }
        }
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
    }
}

/// Converts a value to a string
pub fn to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

/// Safe arithmetic operations with overflow protection
pub fn safe_add(a: f64, b: f64) -> f64 {
    let result = a + b;
    if result.is_infinite() {
        if result.is_sign_positive() {
            f64::MAX
        } else {
            f64::MIN
        }
    } else if result.is_nan() {
        0.0
    } else {
        result
    }
}

pub fn safe_subtract(a: f64, b: f64) -> f64 {
    let result = a - b;
    if result.is_infinite() {
        if result.is_sign_positive() {
            f64::MAX
        } else {
            f64::MIN
        }
    } else if result.is_nan() {
        0.0
    } else {
        result
    }
}

pub fn safe_multiply(a: f64, b: f64) -> f64 {
    let result = a * b;
    if result.is_infinite() {
        if (a > 0.0 && b > 0.0) || (a < 0.0 && b < 0.0) {
            f64::MAX
        } else {
            f64::MIN
        }
    } else if result.is_nan() {
        0.0
    } else {
        result
    }
}

pub fn safe_divide(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        if a > 0.0 {
            f64::MAX
        } else if a < 0.0 {
            f64::MIN
        } else {
            0.0
        }
    } else {
        let result = a / b;
        if result.is_infinite() {
            if result.is_sign_positive() {
                f64::MAX
            } else {
                f64::MIN
            }
        } else if result.is_nan() {
            0.0
        } else {
            result
        }
    }
}

pub fn safe_modulo(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        0.0
    } else {
        let result = a % b;
        if result.is_nan() { 0.0 } else { result }
    }
}

/// Extracts a string from a value
pub fn extract_string(value: &Value) -> Cow<'_, str> {
    match value {
        Value::String(s) => Cow::Borrowed(s),
        _ => Cow::Owned(to_string(value)),
    }
}

/// Creates a JSON number value with proper overflow handling
pub fn create_number_value(n: f64) -> Value {
    if n.is_finite() {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    } else if n.is_infinite() {
        if n.is_sign_positive() {
            serde_json::Number::from_f64(f64::MAX)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        } else {
            serde_json::Number::from_f64(f64::MIN)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
    } else {
        Value::Null
    }
}

/// Extracts a datetime from a value (either datetime object or string)
pub fn extract_datetime_value(value: &Value) -> Option<crate::datetime::DataDateTime> {
    if crate::datetime::is_datetime_object(value) {
        crate::datetime::extract_datetime(value)
    } else if let Value::String(s) = value {
        crate::datetime::DataDateTime::parse(s)
    } else {
        None
    }
}

/// Extracts a duration from a value (either duration object or string)
pub fn extract_duration_value(value: &Value) -> Option<crate::datetime::DataDuration> {
    if crate::datetime::is_duration_object(value) {
        crate::datetime::extract_duration(value)
    } else if let Value::String(s) = value {
        crate::datetime::DataDuration::parse(s)
    } else {
        None
    }
}
