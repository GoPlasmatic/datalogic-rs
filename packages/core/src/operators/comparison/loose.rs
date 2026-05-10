//! Loose equality (`==` / `!=`) coercion table and dispatcher.
//!
//! Reached from the comparison-arena collection-fallback path (rare — array-vs-
//! array / object-vs-object) and from the primitive `==`/`!=` arms. Strict
//! equality (`===`) compares values directly without going through here.
//!
//! Loose coercion table:
//!
//! | Left Type | Right Type | Behavior                       |
//! |-----------|------------|--------------------------------|
//! | Number    | String     | Parse string as number         |
//! | Number    | Bool       | `true` → `1`, `false` → `0`    |
//! | String    | Bool       | Compare to `"true"`/`"false"`  |
//! | Null      | Number     | `null` equals `0`              |
//! | Null      | Bool       | `null` equals `false`          |
//! | Null      | String     | `null` equals `""`             |

use crate::arena::DataValue;
use crate::error::NAN_ERROR;
use crate::{Engine, Error, Result};

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
            if a == b {
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
pub(super) fn loose_equals(
    left: &DataValue<'_>,
    right: &DataValue<'_>,
    engine: &Engine,
) -> Result<bool> {
    match loose_equals_core(left, right) {
        LooseEqualsResult::Equal => Ok(true),
        LooseEqualsResult::NotEqual => Ok(false),
        LooseEqualsResult::Incompatible => {
            if engine.config().loose_equality_errors {
                Err(Error::invalid_arguments(NAN_ERROR))
            } else {
                Ok(false)
            }
        }
    }
}
