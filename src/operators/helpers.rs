use std::borrow::Cow;

use crate::CompiledNode;
use crate::config::TruthyEvaluator;
use crate::constants::INVALID_ARGS;
use serde_json::Value;

/// Checks if a value is truthy using the engine's configuration
#[inline(always)]
pub fn is_truthy(value: &Value, engine: &crate::DataLogic) -> bool {
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript => is_truthy_js(value),
        TruthyEvaluator::Python => {
            // Python-style truthiness (same as JavaScript for these types)
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
        TruthyEvaluator::StrictBoolean => {
            // Strict boolean truthiness
            match value {
                Value::Null => false,
                Value::Bool(b) => *b,
                _ => true,
            }
        }
        TruthyEvaluator::Custom(f) => f(value),
    }
}

/// Checks if a value is truthy according to JavaScript rules (for backward compatibility)
#[inline(always)]
pub fn is_truthy_js(value: &Value) -> bool {
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

/// Converts a value to a string, borrowing when possible to avoid allocation
#[inline]
pub fn to_string_cow(value: &Value) -> Cow<'_, str> {
    match value {
        Value::String(s) => Cow::Borrowed(s.as_str()),
        Value::Null => Cow::Borrowed(""),
        _ => Cow::Owned(value.to_string()),
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

/// Strict number extraction - only accepts actual numbers or numeric strings.
/// Used by abs, floor, ceil operators.
#[inline]
pub fn get_number_strict(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
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

/// Extracts a datetime from a value (either datetime object or string).
/// Single map lookup for objects — avoids the double lookup of is_datetime_object + extract_datetime.
#[inline]
pub fn extract_datetime_value(value: &Value) -> Option<crate::datetime::DataDateTime> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(s)) = map.get("datetime") {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            }
        }
        Value::String(s) => crate::datetime::DataDateTime::parse(s),
        _ => None,
    }
}

/// Extracts a duration from a value (either duration object or string).
/// Single map lookup for objects — avoids the double lookup of is_duration_object + extract_duration.
#[inline]
pub fn extract_duration_value(value: &Value) -> Option<crate::datetime::DataDuration> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(s)) = map.get("timestamp") {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            }
        }
        Value::String(s) => crate::datetime::DataDuration::parse(s),
        _ => None,
    }
}

/// Checks if args contain the `__invalid_args__` sentinel marker from compilation.
/// Returns an error if the marker is present, Ok(()) otherwise.
#[inline]
pub fn check_invalid_args_marker(args: &[CompiledNode]) -> crate::Result<()> {
    if args.len() == 1
        && let CompiledNode::Value { value, .. } = &args[0]
        && let Some(obj) = value.as_object()
        && obj.contains_key("__invalid_args__")
    {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }
    Ok(())
}
