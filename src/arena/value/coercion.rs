//! Numeric coercion for `DataValue`.
//!
//! There are four distinct coercion policies in the crate; this module owns
//! the *general-purpose* one. The others have intentionally different
//! semantics and live next to their callers:
//!
//! | Policy            | Where                                          | Used by                          |
//! |-------------------|------------------------------------------------|----------------------------------|
//! | Numeric (config)  | this module — `coerce_to_number_cfg`           | comparison, arithmetic helpers   |
//! | Numeric (default) | this module — `coerce_to_number` (datetime)    | datetime arithmetic              |
//! | Equality          | `operators/comparison.rs::loose_equals_core`   | `==` / `!=` (typed coercion table) |
//! | Arithmetic pair   | `operators/arithmetic/helpers.rs::coerce_pair_*` | `+`/`-`/`*`/`/`/`%` (delegate to `_cfg`) |
//!
//! Equality coercion is structural (NaN-strict, type-table) and does not
//! reuse the f64 path; arithmetic-pair helpers are thin wrappers over
//! [`coerce_to_number_cfg`] / [`try_coerce_to_integer_cfg`].

use super::DataValue;

/// Config-aware arena-native f64 coercion. Honours the engine's
/// [`crate::EvaluationConfig::numeric_coercion`] flags
/// (`empty_string_to_zero`, `bool_to_number`, `null_to_zero`).
#[inline]
pub(crate) fn coerce_to_number_cfg(v: &DataValue<'_>, engine: &crate::Engine) -> Option<f64> {
    match v {
        DataValue::Number(n) => Some(n.as_f64()),
        DataValue::String(s) => {
            if s.is_empty() && engine.config().numeric_coercion.empty_string_to_zero {
                Some(0.0)
            } else {
                s.parse().ok()
            }
        }
        DataValue::Bool(b) if engine.config().numeric_coercion.bool_to_number => {
            Some(if *b { 1.0 } else { 0.0 })
        }
        DataValue::Null if engine.config().numeric_coercion.null_to_zero => Some(0.0),
        _ => None,
    }
}

/// Config-aware arena-native i64 coercion. Honours the same
/// `numeric_coercion` flags as [`coerce_to_number_cfg`] but bails for
/// fractional values.
#[inline]
pub(crate) fn try_coerce_to_integer_cfg(v: &DataValue<'_>, engine: &crate::Engine) -> Option<i64> {
    match v {
        DataValue::Number(n) => n.as_i64(),
        DataValue::String(s) => {
            if s.is_empty() && engine.config().numeric_coercion.empty_string_to_zero {
                Some(0)
            } else {
                s.parse().ok()
            }
        }
        DataValue::Bool(b) if engine.config().numeric_coercion.bool_to_number => {
            Some(if *b { 1 } else { 0 })
        }
        DataValue::Null if engine.config().numeric_coercion.null_to_zero => Some(0),
        _ => None,
    }
}

/// Coerce an `DataValue` to f64 using default JSON Logic coercion rules
/// (no engine config consulted). Used by datetime arithmetic where the
/// duration/scalar-multiply path runs before user config can intervene.
#[cfg(feature = "datetime")]
pub(crate) fn coerce_to_number(v: &DataValue<'_>) -> Option<f64> {
    match v {
        DataValue::Number(n) => Some(n.as_f64()),
        DataValue::Bool(true) => Some(1.0),
        DataValue::Bool(false) => Some(0.0),
        DataValue::Null => Some(0.0),
        DataValue::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Some(0.0)
            } else {
                t.parse().ok()
            }
        }
        DataValue::Array(items) => match items.len() {
            0 => Some(0.0),
            1 => coerce_to_number(&items[0]),
            _ => None,
        },
        DataValue::Object(_) => None,
        #[cfg(feature = "datetime")]
        DataValue::DateTime(_) | DataValue::Duration(_) => None,
    }
}
