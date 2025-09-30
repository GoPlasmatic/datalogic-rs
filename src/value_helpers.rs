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

    // For simple paths without dots, use direct access
    if !path.contains('.') {
        if let Value::Object(obj) = value
            && let Some(val) = obj.get(path)
        {
            return Some(val.clone());
        }
        if let Ok(index) = path.parse::<usize>()
            && let Value::Array(arr) = value
        {
            return arr.get(index).cloned();
        }
        return None;
    }

    // Handle paths with dots manually to avoid JSON pointer issues with numeric property names
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts.iter() {
        match current {
            Value::Object(obj) => {
                // Try as object key first
                if let Some(val) = obj.get(*part) {
                    current = val;
                } else {
                    return None;
                }
            }
            Value::Array(arr) => {
                // Try as array index
                if let Ok(index) = part.parse::<usize>() {
                    if let Some(val) = arr.get(index) {
                        current = val;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(current.clone())
}

/// Coerce a value to a number using the engine's configuration
pub fn coerce_to_number(value: &Value, engine: &crate::DataLogic) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            if s.is_empty() && engine.config().numeric_coercion.empty_string_to_zero {
                Some(0.0)
            } else {
                s.parse().ok()
            }
        }
        Value::Bool(b) if engine.config().numeric_coercion.bool_to_number => {
            Some(if *b { 1.0 } else { 0.0 })
        }
        Value::Null if engine.config().numeric_coercion.null_to_zero => Some(0.0),
        _ => None,
    }
}

/// Compare two values with strict equality (JavaScript ===)
pub fn strict_equals(left: &Value, right: &Value) -> bool {
    left == right
}

/// Compare two values with loose equality, returning error for incompatible types
pub fn loose_equals_with_error(left: &Value, right: &Value) -> crate::Result<bool> {
    use crate::Error;

    // Helper to return NaN error
    let nan_error = || Error::InvalidArguments(NAN_ERROR.into());

    match (left, right) {
        // Same type comparisons
        (Value::Null, Value::Null) => Ok(true),
        (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
        (Value::String(a), Value::String(b)) => Ok(a == b),
        (Value::Number(a), Value::Number(b)) => {
            let a_f = a.as_f64().unwrap_or(f64::NAN);
            let b_f = b.as_f64().unwrap_or(f64::NAN);
            Ok(!a_f.is_nan() && !b_f.is_nan() && a_f == b_f)
        }

        // Number-String coercion
        (Value::Number(n), Value::String(s)) | (Value::String(s), Value::Number(n)) => n
            .as_f64()
            .and_then(|n_f| s.parse::<f64>().ok().map(|s_f| n_f == s_f))
            .ok_or_else(nan_error),

        // Number-Bool coercion
        (Value::Number(n), Value::Bool(b)) | (Value::Bool(b), Value::Number(n)) => {
            Ok(n.as_f64() == Some(if *b { 1.0 } else { 0.0 }))
        }

        // String-Bool coercion
        (Value::String(s), Value::Bool(b)) | (Value::Bool(b), Value::String(s)) => {
            Ok(s == if *b { "true" } else { "false" })
        }

        // Null coercions
        (Value::Null, Value::Number(n)) | (Value::Number(n), Value::Null) => {
            Ok(n.as_f64() == Some(0.0))
        }
        (Value::Null, Value::Bool(b)) | (Value::Bool(b), Value::Null) => Ok(!*b),
        (Value::Null, Value::String(s)) | (Value::String(s), Value::Null) => Ok(s.is_empty()),

        // Complex types compared to primitives - all error
        (Value::Array(_), _) | (_, Value::Array(_))
            if !matches!((left, right), (Value::Array(_), Value::Array(_))) =>
        {
            Err(nan_error())
        }
        (Value::Object(_), _) | (_, Value::Object(_))
            if !matches!((left, right), (Value::Object(_), Value::Object(_))) =>
        {
            Err(nan_error())
        }

        // Array to array comparison
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() || !a.iter().zip(b.iter()).all(|(av, bv)| av == bv) {
                Err(nan_error())
            } else {
                Ok(true)
            }
        }

        _ => Ok(false),
    }
}

/// Try to coerce a value to an integer using the engine's configuration
pub fn try_coerce_to_integer(value: &Value, engine: &crate::DataLogic) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => {
            if s.is_empty() && engine.config().numeric_coercion.empty_string_to_zero {
                Some(0)
            } else {
                s.parse().ok()
            }
        }
        Value::Bool(b) if engine.config().numeric_coercion.bool_to_number => {
            Some(if *b { 1 } else { 0 })
        }
        Value::Null if engine.config().numeric_coercion.null_to_zero => Some(0),
        _ => None,
    }
}

/// Compare two values with loose equality using the engine's configuration
pub fn loose_equals(left: &Value, right: &Value, engine: &crate::DataLogic) -> crate::Result<bool> {
    if engine.config().loose_equality_errors {
        loose_equals_with_error(left, right)
    } else {
        // Same logic but return false instead of error for incompatible types
        match (left, right) {
            // Same type comparisons
            (Value::Null, Value::Null) => Ok(true),
            (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
            (Value::String(a), Value::String(b)) => Ok(a == b),
            (Value::Number(a), Value::Number(b)) => {
                let a_f = a.as_f64().unwrap_or(f64::NAN);
                let b_f = b.as_f64().unwrap_or(f64::NAN);
                Ok(!a_f.is_nan() && !b_f.is_nan() && a_f == b_f)
            }

            // Number-String coercion
            (Value::Number(n), Value::String(s)) | (Value::String(s), Value::Number(n)) => Ok(n
                .as_f64()
                .and_then(|n_f| s.parse::<f64>().ok().map(|s_f| n_f == s_f))
                .unwrap_or(false)),

            // Number-Bool coercion
            (Value::Number(n), Value::Bool(b)) | (Value::Bool(b), Value::Number(n)) => {
                Ok(n.as_f64() == Some(if *b { 1.0 } else { 0.0 }))
            }

            // String-Bool coercion
            (Value::String(s), Value::Bool(b)) | (Value::Bool(b), Value::String(s)) => {
                Ok(s == if *b { "true" } else { "false" })
            }

            // Null coercions
            (Value::Null, Value::Number(n)) | (Value::Number(n), Value::Null) => {
                Ok(n.as_f64() == Some(0.0))
            }
            (Value::Null, Value::Bool(b)) | (Value::Bool(b), Value::Null) => Ok(!*b),
            (Value::Null, Value::String(s)) | (Value::String(s), Value::Null) => Ok(s.is_empty()),

            // Complex types compared to primitives - return false instead of error
            (Value::Array(_), _) | (_, Value::Array(_))
                if !matches!((left, right), (Value::Array(_), Value::Array(_))) =>
            {
                Ok(false)
            }
            (Value::Object(_), _) | (_, Value::Object(_))
                if !matches!((left, right), (Value::Object(_), Value::Object(_))) =>
            {
                Ok(false)
            }

            // Array to array comparison
            (Value::Array(a), Value::Array(b)) => {
                Ok(a.len() == b.len() && a.iter().zip(b.iter()).all(|(av, bv)| av == bv))
            }

            _ => Ok(false),
        }
    }
}
