use crate::constants::NAN_ERROR;
use serde_json::Value;

/// Access a path in a JSON value using dot notation
/// Supports:
/// - Object field access: "field" or "field.nested"
/// - Array index access: "0" or "field.0"
/// - Mixed: "field.0.nested"
pub fn access_path(value: &Value, path: &str) -> Option<Value> {
    if path.is_empty() {
        return Some(value.clone());
    }

    // If the path doesn't contain dots, try direct key access first
    // This handles special keys like "../" or keys with slashes
    if !path.contains('.') {
        if let Value::Object(obj) = value
            && let Some(val) = obj.get(path)
        {
            return Some(val.clone());
        }
        // Also try array index access for simple numeric paths
        if let Ok(index) = path.parse::<usize>()
            && let Value::Array(arr) = value
        {
            return arr.get(index).cloned();
        }
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

    value.pointer(&pointer).cloned()
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
        Value::Object(o) => !o.is_empty(),
    }
}

/// Coerce a value to a number
pub fn coerce_to_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            if s.is_empty() {
                Some(0.0)
            } else {
                s.parse().ok()
            }
        }
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
        Value::String(s) => {
            if s.is_empty() {
                Some(0)
            } else {
                s.parse().ok()
            }
        }
        Value::Bool(true) => Some(1),
        Value::Bool(false) => Some(0),
        Value::Null => Some(0),
        _ => None,
    }
}

/// Compare two values with strict equality (JavaScript ===)
pub fn strict_equals(left: &Value, right: &Value) -> bool {
    left == right
}

/// Compare two values with loose equality, returning error for incompatible types
pub fn loose_equals_with_error(left: &Value, right: &Value) -> crate::Result<bool> {
    match (left, right) {
        (Value::Null, Value::Null) => Ok(true),
        (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
        (Value::Number(a), Value::Number(b)) => {
            let a_f = a.as_f64().unwrap_or(f64::NAN);
            let b_f = b.as_f64().unwrap_or(f64::NAN);
            if a_f.is_nan() && b_f.is_nan() {
                Ok(false) // NaN != NaN in JavaScript
            } else {
                Ok(a_f == b_f)
            }
        }
        (Value::String(a), Value::String(b)) => Ok(a == b),
        // Type coercion cases
        (Value::Number(n), Value::String(s)) | (Value::String(s), Value::Number(n)) => {
            if let Some(n_f) = n.as_f64() {
                if let Ok(s_f) = s.parse::<f64>() {
                    Ok(n_f == s_f)
                } else {
                    // Non-numeric string compared with number
                    Err(crate::Error::InvalidArguments(NAN_ERROR.into()))
                }
            } else {
                Ok(false)
            }
        }
        (Value::Number(n), Value::Bool(b)) | (Value::Bool(b), Value::Number(n)) => {
            let b_val = if *b { 1.0 } else { 0.0 };
            Ok(n.as_f64() == Some(b_val))
        }
        (Value::String(s), Value::Bool(b)) | (Value::Bool(b), Value::String(s)) => {
            let b_str = if *b { "true" } else { "false" };
            Ok(s == b_str)
        }
        // Null coerces to 0 in loose equality
        (Value::Null, Value::Number(n)) | (Value::Number(n), Value::Null) => {
            Ok(n.as_f64() == Some(0.0))
        }
        // Null coerces to false in loose equality
        (Value::Null, Value::Bool(b)) | (Value::Bool(b), Value::Null) => Ok(!*b),
        // Null coerces to empty string in loose equality
        (Value::Null, Value::String(s)) | (Value::String(s), Value::Null) => Ok(s.is_empty()),
        // Arrays compared to primitives
        (Value::Array(_), Value::Number(_))
        | (Value::Number(_), Value::Array(_))
        | (Value::Array(_), Value::String(_))
        | (Value::String(_), Value::Array(_))
        | (Value::Array(_), Value::Bool(_))
        | (Value::Bool(_), Value::Array(_)) => {
            Err(crate::Error::InvalidArguments(NAN_ERROR.into()))
        }
        // Objects compared to primitives
        (Value::Object(_), Value::Number(_))
        | (Value::Number(_), Value::Object(_))
        | (Value::Object(_), Value::String(_))
        | (Value::String(_), Value::Object(_))
        | (Value::Object(_), Value::Bool(_))
        | (Value::Bool(_), Value::Object(_)) => {
            Err(crate::Error::InvalidArguments(NAN_ERROR.into()))
        }
        // Arrays/objects to arrays/objects (different instances)
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                // Different arrays should throw NaN error
                Err(crate::Error::InvalidArguments(NAN_ERROR.into()))
            } else {
                // Check if contents are equal
                for (av, bv) in a.iter().zip(b.iter()) {
                    if av != bv {
                        return Err(crate::Error::InvalidArguments(NAN_ERROR.into()));
                    }
                }
                Ok(true)
            }
        }
        _ => Ok(false),
    }
}
