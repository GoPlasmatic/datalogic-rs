use serde_json::Value;
use std::borrow::Cow;

/// Access a path in a JSON value using dot notation
/// Supports:
/// - Object field access: "field" or "field.nested"
/// - Array index access: "0" or "field.0"
/// - Mixed: "field.0.nested"
pub fn access_path<'a>(value: &'a Value, path: &str) -> Option<Cow<'a, Value>> {
    if path.is_empty() || path.is_empty() {
        return Some(Cow::Borrowed(value));
    }

    // Use serde_json's pointer method for JSON pointer syntax
    // Convert dot notation to JSON pointer format
    let pointer = if path.starts_with('/') {
        // Already in pointer format
        path.to_string()
    } else {
        // Convert dot notation to pointer format
        let mut pointer = String::from("/");
        pointer.push_str(&path.replace('.', "/"));
        pointer
    };

    value.pointer(&pointer).map(Cow::Borrowed)
}

/// Coerce a value to boolean (JavaScript-like truthiness)
pub fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                f != 0.0 && !f.is_nan()
            } else {
                n.as_i64().unwrap_or(0) != 0
            }
        }
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

/// Coerce a value to a number
pub fn coerce_to_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        Value::Bool(true) => Some(1.0),
        Value::Bool(false) => Some(0.0),
        Value::Null => Some(0.0),
        _ => None,
    }
}

/// Try to coerce a value to an integer
pub fn try_coerce_to_integer(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        Value::Bool(true) => Some(1),
        Value::Bool(false) => Some(0),
        Value::Null => Some(0),
        _ => None,
    }
}

/// Compare two values with loose equality (JavaScript ==)
pub fn loose_equals(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => {
            let a_f = a.as_f64().unwrap_or(f64::NAN);
            let b_f = b.as_f64().unwrap_or(f64::NAN);
            if a_f.is_nan() && b_f.is_nan() {
                false // NaN != NaN in JavaScript
            } else {
                a_f == b_f
            }
        }
        (Value::String(a), Value::String(b)) => a == b,
        // Type coercion cases
        (Value::Number(n), Value::String(s)) | (Value::String(s), Value::Number(n)) => {
            if let (Some(n_f), Ok(s_f)) = (n.as_f64(), s.parse::<f64>()) {
                n_f == s_f
            } else {
                false
            }
        }
        (Value::Number(n), Value::Bool(b)) | (Value::Bool(b), Value::Number(n)) => {
            let b_val = if *b { 1.0 } else { 0.0 };
            n.as_f64() == Some(b_val)
        }
        (Value::String(s), Value::Bool(b)) | (Value::Bool(b), Value::String(s)) => {
            let b_str = if *b { "true" } else { "false" };
            s == b_str
        }
        _ => false,
    }
}

/// Compare two values with strict equality (JavaScript ===)
pub fn strict_equals(left: &Value, right: &Value) -> bool {
    left == right
}
