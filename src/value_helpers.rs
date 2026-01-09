//! Helper functions for JSON value manipulation and type coercion.
//!
//! This module provides utilities for accessing nested values, comparing values,
//! and coercing between types according to JSONLogic semantics.
//!
//! # Path Access
//!
//! The `access_path` and `access_path_ref` functions support dot-notation paths
//! for navigating nested JSON structures:
//!
//! - `"field"` - Access object field
//! - `"0"` - Access array element by index
//! - `"field.nested"` - Nested object access
//! - `"field.0.nested"` - Mixed object/array access
//!
//! # Equality Comparison
//!
//! Two modes of equality are supported:
//!
//! - **Strict equality** (`===`): Values must have same type and value
//! - **Loose equality** (`==`): Type coercion is applied before comparison
//!
//! # Loose Equality Coercion Rules
//!
//! | Left Type | Right Type | Behavior |
//! |-----------|------------|----------|
//! | Number | String | Parse string as number |
//! | Number | Bool | `true` → `1`, `false` → `0` |
//! | String | Bool | Compare to `"true"`/`"false"` |
//! | Null | Number | `null` equals `0` |
//! | Null | Bool | `null` equals `false` |
//! | Null | String | `null` equals `""` |
//!
//! # Numeric Coercion
//!
//! Configurable through `EvaluationConfig`:
//! - Empty string to zero (optional)
//! - Boolean to number (`true` → `1`, `false` → `0`)
//! - Null to zero (optional)

use crate::constants::NAN_ERROR;
use serde_json::Value;

/// Access a path in a JSON value using dot notation (reference variant)
/// Supports:
/// - Object field access: "field" or "field.nested"
/// - Array index access: "0" or "field.0"
/// - Mixed: "field.0.nested"
///   Returns a reference to avoid unnecessary cloning
pub fn access_path_ref<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    // For simple paths without dots, use direct access
    if !path.contains('.') {
        if let Value::Object(obj) = value
            && let Some(val) = obj.get(path)
        {
            return Some(val);
        }
        if let Ok(index) = path.parse::<usize>()
            && let Value::Array(arr) = value
        {
            return arr.get(index);
        }
        return None;
    }

    // Handle paths with dots - use iterator directly to avoid Vec allocation
    let mut current = value;

    for part in path.split('.') {
        match current {
            Value::Object(obj) => {
                // Try as object key first
                if let Some(val) = obj.get(part) {
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

    Some(current)
}

/// Access a path in a JSON value using dot notation (cloning variant)
/// Supports:
/// - Object field access: "field" or "field.nested"
/// - Array index access: "0" or "field.0"
/// - Mixed: "field.0.nested"
pub fn access_path(value: &Value, path: &str) -> Option<Value> {
    access_path_ref(value, path).cloned()
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

/// Result of loose equality comparison for incompatible types
enum LooseEqualsResult {
    Equal,
    NotEqual,
    Incompatible,
}

/// Core implementation of loose equality comparison
fn loose_equals_core(left: &Value, right: &Value) -> LooseEqualsResult {
    use LooseEqualsResult::*;

    match (left, right) {
        // Same type comparisons
        (Value::Null, Value::Null) => Equal,
        (Value::Bool(a), Value::Bool(b)) => {
            if a == b {
                Equal
            } else {
                NotEqual
            }
        }
        (Value::String(a), Value::String(b)) => {
            if a == b {
                Equal
            } else {
                NotEqual
            }
        }
        (Value::Number(a), Value::Number(b)) => {
            let a_f = a.as_f64().unwrap_or(f64::NAN);
            let b_f = b.as_f64().unwrap_or(f64::NAN);
            if !a_f.is_nan() && !b_f.is_nan() && a_f == b_f {
                Equal
            } else {
                NotEqual
            }
        }

        // Number-String coercion
        (Value::Number(n), Value::String(s)) | (Value::String(s), Value::Number(n)) => {
            match (n.as_f64(), s.parse::<f64>().ok()) {
                (Some(n_f), Some(s_f)) if n_f == s_f => Equal,
                (Some(_), Some(_)) => NotEqual,
                _ => Incompatible,
            }
        }

        // Number-Bool coercion
        (Value::Number(n), Value::Bool(b)) | (Value::Bool(b), Value::Number(n)) => {
            if n.as_f64() == Some(if *b { 1.0 } else { 0.0 }) {
                Equal
            } else {
                NotEqual
            }
        }

        // String-Bool coercion
        (Value::String(s), Value::Bool(b)) | (Value::Bool(b), Value::String(s)) => {
            if s == if *b { "true" } else { "false" } {
                Equal
            } else {
                NotEqual
            }
        }

        // Null coercions
        (Value::Null, Value::Number(n)) | (Value::Number(n), Value::Null) => {
            if n.as_f64() == Some(0.0) {
                Equal
            } else {
                NotEqual
            }
        }
        (Value::Null, Value::Bool(b)) | (Value::Bool(b), Value::Null) => {
            if !*b {
                Equal
            } else {
                NotEqual
            }
        }
        (Value::Null, Value::String(s)) | (Value::String(s), Value::Null) => {
            if s.is_empty() {
                Equal
            } else {
                NotEqual
            }
        }

        // Complex types compared to primitives - incompatible
        (Value::Array(_), _) | (_, Value::Array(_))
            if !matches!((left, right), (Value::Array(_), Value::Array(_))) =>
        {
            Incompatible
        }
        (Value::Object(_), _) | (_, Value::Object(_))
            if !matches!((left, right), (Value::Object(_), Value::Object(_))) =>
        {
            Incompatible
        }

        // Array to array comparison
        (Value::Array(a), Value::Array(b)) => {
            if a.len() == b.len() && a.iter().zip(b.iter()).all(|(av, bv)| av == bv) {
                Equal
            } else {
                Incompatible
            }
        }

        _ => NotEqual,
    }
}

/// Compare two values with loose equality, returning error for incompatible types
pub fn loose_equals_with_error(left: &Value, right: &Value) -> crate::Result<bool> {
    use crate::Error;

    match loose_equals_core(left, right) {
        LooseEqualsResult::Equal => Ok(true),
        LooseEqualsResult::NotEqual => Ok(false),
        LooseEqualsResult::Incompatible => Err(Error::InvalidArguments(NAN_ERROR.into())),
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
        // Return false instead of error for incompatible types
        match loose_equals_core(left, right) {
            LooseEqualsResult::Equal => Ok(true),
            LooseEqualsResult::NotEqual | LooseEqualsResult::Incompatible => Ok(false),
        }
    }
}
