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

use crate::arena::{ContextStack, DataValue};
use crate::operators::{NanForm, nan_error};
use crate::{Engine, Result};

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
        // `NumberValue`'s equality: two integers exactly, as `===`
        // compares them.
        (DataValue::Number(a), DataValue::Number(b)) => {
            if a == b {
                Equal
            } else {
                NotEqual
            }
        }

        // Number-String coercion. An integer against an integer string
        // compares exactly, as two numbers do.
        (DataValue::Number(n), DataValue::String(s))
        | (DataValue::String(s), DataValue::Number(n)) => {
            if let (Some(i), Ok(si)) = (n.as_i64(), s.parse::<i64>()) {
                return if i == si { Equal } else { NotEqual };
            }
            match crate::arena::parse_finite(s) {
                Some(s_f) if n.as_f64() == s_f => Equal,
                Some(_) => NotEqual,
                None => Incompatible,
            }
        }

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

        // Tensor-tensor is datavalue's structural `PartialEq`: dtype,
        // shape, and payload bytes. Two tensors that differ are genuinely
        // `NotEqual` rather than `Incompatible` — unlike arrays, there is
        // no coercion left to try.
        #[cfg(feature = "tensor")]
        (DataValue::Tensor(a), DataValue::Tensor(b)) => {
            if a == b {
                Equal
            } else {
                NotEqual
            }
        }
        // A tensor against anything else is incompatible, the same answer
        // an object gets, so it follows the `loose_equality_errors` config
        // rather than silently reporting `false`.
        #[cfg(feature = "tensor")]
        (DataValue::Tensor(_), _) | (_, DataValue::Tensor(_)) => Incompatible,

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
    ctx: &mut ContextStack<'_>,
) -> Result<bool> {
    match loose_equals_core(left, right) {
        LooseEqualsResult::Equal => Ok(true),
        LooseEqualsResult::NotEqual => Ok(false),
        LooseEqualsResult::Incompatible => incompatible(engine, ctx),
    }
}

/// The answer for operands loose equality cannot compare (two unequal
/// arrays among them): an error under `loose_equality_errors`, else
/// `false`.
pub(super) fn incompatible(engine: &Engine, ctx: &mut ContextStack<'_>) -> Result<bool> {
    if engine.config().loose_equality_errors {
        Err(nan_error(NanForm::InvalidArguments, ctx))
    } else {
        Ok(false)
    }
}
