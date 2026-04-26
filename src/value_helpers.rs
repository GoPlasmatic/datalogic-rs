//! Boundary equality helpers for `serde_json::Value`.
//!
//! All evaluation-hot-path operators dispatch on `ArenaValue`. The functions
//! here are reached only from the comparison-arena collection-fallback path
//! (rare — array-vs-array / object-vs-object), via the `arena_to_value_cow`
//! materialisation in `operators/comparison.rs`.
//!
//! # Equality modes
//!
//! - **Strict** (`===`): same type and value (`left == right`).
//! - **Loose** (`==`): coerce per the table below before comparing.
//!
//! # Loose coercion table
//!
//! | Left Type | Right Type | Behavior |
//! |-----------|------------|----------|
//! | Number    | String     | Parse string as number |
//! | Number    | Bool       | `true` → `1`, `false` → `0` |
//! | String    | Bool       | Compare to `"true"`/`"false"` |
//! | Null      | Number     | `null` equals `0` |
//! | Null      | Bool       | `null` equals `false` |
//! | Null      | String     | `null` equals `""` |

use crate::constants::NAN_ERROR;
use serde_json::Value;

/// Compare two values with strict equality (JavaScript ===).
pub(crate) fn strict_equals(left: &Value, right: &Value) -> bool {
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

/// Compare two values with loose equality. When the engine config has
/// `loose_equality_errors` enabled, type-incompatible operands return an
/// error; otherwise they compare as not-equal.
pub(crate) fn loose_equals(
    left: &Value,
    right: &Value,
    engine: &crate::DataLogic,
) -> crate::Result<bool> {
    use crate::Error;
    match loose_equals_core(left, right) {
        LooseEqualsResult::Equal => Ok(true),
        LooseEqualsResult::NotEqual => Ok(false),
        LooseEqualsResult::Incompatible => {
            if engine.config().loose_equality_errors {
                Err(Error::InvalidArguments(NAN_ERROR.into()))
            } else {
                Ok(false)
            }
        }
    }
}
