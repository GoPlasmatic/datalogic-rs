//! Boundary equality helpers for [`DataValue`].
//!
//! All evaluation-hot-path operators dispatch on `DataValue`. The functions
//! here are reached from the comparison-arena collection-fallback path
//! (rare — array-vs-array / object-vs-object) in `operators/comparison.rs`.
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

use crate::arena::DataValue;
use crate::constants::NAN_ERROR;

/// Compare two values with strict equality (JavaScript ===). Walks both
/// trees structurally; arena and number type tags must match exactly for
/// equality.
pub(crate) fn strict_equals(left: &DataValue<'_>, right: &DataValue<'_>) -> bool {
    match (left, right) {
        (DataValue::Null, DataValue::Null) => true,
        (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
        (DataValue::Number(a), DataValue::Number(b)) => a == b,
        (DataValue::String(a), DataValue::String(b)) => a == b,
        (DataValue::Array(a), DataValue::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| strict_equals(x, y))
        }
        (DataValue::Object(a), DataValue::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| *ka == *kb && strict_equals(va, vb))
        }
        #[cfg(feature = "datetime")]
        (DataValue::DateTime(a), DataValue::DateTime(b)) => a == b,
        #[cfg(feature = "datetime")]
        (DataValue::Duration(a), DataValue::Duration(b)) => a == b,
        _ => false,
    }
}

/// Result of loose equality comparison for incompatible types.
enum LooseEqualsResult {
    Equal,
    NotEqual,
    Incompatible,
}

fn loose_equals_core(left: &DataValue<'_>, right: &DataValue<'_>) -> LooseEqualsResult {
    use LooseEqualsResult::*;

    match (left, right) {
        // Same-type cases
        (DataValue::Null, DataValue::Null) => Equal,
        (DataValue::Bool(a), DataValue::Bool(b)) => {
            if a == b {
                Equal
            } else {
                NotEqual
            }
        }
        (DataValue::String(a), DataValue::String(b)) => {
            if a == b {
                Equal
            } else {
                NotEqual
            }
        }
        (DataValue::Number(a), DataValue::Number(b)) => {
            let a_f = a.as_f64();
            let b_f = b.as_f64();
            if !a_f.is_nan() && !b_f.is_nan() && a_f == b_f {
                Equal
            } else {
                NotEqual
            }
        }

        // Number-String coercion
        (DataValue::Number(n), DataValue::String(s))
        | (DataValue::String(s), DataValue::Number(n)) => match s.parse::<f64>().ok() {
            Some(s_f) if n.as_f64() == s_f => Equal,
            Some(_) => NotEqual,
            None => Incompatible,
        },

        // Number-Bool coercion
        (DataValue::Number(n), DataValue::Bool(b)) | (DataValue::Bool(b), DataValue::Number(n)) => {
            if n.as_f64() == (if *b { 1.0 } else { 0.0 }) {
                Equal
            } else {
                NotEqual
            }
        }

        // String-Bool coercion
        (DataValue::String(s), DataValue::Bool(b)) | (DataValue::Bool(b), DataValue::String(s)) => {
            if *s == (if *b { "true" } else { "false" }) {
                Equal
            } else {
                NotEqual
            }
        }

        // Null coercions
        (DataValue::Null, DataValue::Number(n)) | (DataValue::Number(n), DataValue::Null) => {
            if n.as_f64() == 0.0 { Equal } else { NotEqual }
        }
        (DataValue::Null, DataValue::Bool(b)) | (DataValue::Bool(b), DataValue::Null) => {
            if !*b {
                Equal
            } else {
                NotEqual
            }
        }
        (DataValue::Null, DataValue::String(s)) | (DataValue::String(s), DataValue::Null) => {
            if s.is_empty() { Equal } else { NotEqual }
        }

        // Composite mixed with primitive: incompatible
        (DataValue::Array(_), _) | (_, DataValue::Array(_))
            if !matches!((left, right), (DataValue::Array(_), DataValue::Array(_))) =>
        {
            Incompatible
        }
        (DataValue::Object(_), _) | (_, DataValue::Object(_))
            if !matches!((left, right), (DataValue::Object(_), DataValue::Object(_))) =>
        {
            Incompatible
        }

        // Array-array structural compare
        (DataValue::Array(a), DataValue::Array(b)) => {
            if a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| strict_equals(x, y)) {
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
    left: &DataValue<'_>,
    right: &DataValue<'_>,
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
