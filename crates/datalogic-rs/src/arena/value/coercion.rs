//! Numeric coercion for `DataValue`.
//!
//! There are four distinct coercion policies in the crate; this module owns
//! the *general-purpose* one. The others have intentionally different
//! semantics and live next to their callers:
//!
//! | Policy            | Where                                          | Used by                          |
//! |-------------------|------------------------------------------------|----------------------------------|
//! | Numeric (config)  | this module — `coerce_to_number_cfg`           | comparison, arithmetic, datetime |
//! | Equality          | `operators/comparison.rs::loose_equals_core`   | `==` / `!=` (typed coercion table) |
//! | Arithmetic pair   | `operators/arithmetic/helpers.rs::coerce_pair_*` | `+`/`-`/`*`/`/`/`%` (delegate to `_cfg`) |
//!
//! Equality coercion is structural (NaN-strict, type-table) and does not
//! reuse the f64 path; arithmetic-pair helpers are thin wrappers over
//! [`coerce_to_number_cfg`] / [`try_coerce_to_integer_cfg`].

use super::DataValue;

/// A string's number, for every coercion that reads one: the `f64` it
/// parses as, when finite. `str::parse` also accepts `"NaN"`, `"inf"`,
/// `"infinity"` (any case, either sign) and overflowing literals such as
/// `"1e400"`; none of those is a number a rule can compute with (they
/// serialise as `null`), so they coerce like any other non-numeric string.
#[inline]
pub(crate) fn parse_finite(s: &str) -> Option<f64> {
    s.parse::<f64>().ok().filter(|f| f.is_finite())
}

/// Config-aware arena-native f64 coercion. Honours the engine's
/// [`crate::EvaluationConfig::numeric_coercion`] flags
/// (`reject_non_numeric`, `empty_string_to_zero`, `bool_to_number`,
/// `null_to_zero`).
#[inline]
pub(crate) fn coerce_to_number_cfg(v: &DataValue<'_>, engine: &crate::Engine) -> Option<f64> {
    let coercion = &engine.config().numeric_coercion;
    // `reject_non_numeric` overrides the fabricated coercions: only real
    // numbers and numeric-looking strings are accepted; empty strings,
    // nulls, and booleans become type errors.
    if coercion.reject_non_numeric {
        return match v {
            DataValue::Number(n) => Some(n.as_f64()),
            DataValue::String(s) => parse_finite(s),
            _ => None,
        };
    }
    match v {
        DataValue::Number(n) => Some(n.as_f64()),
        DataValue::String(s) => {
            if s.is_empty() && coercion.empty_string_to_zero {
                Some(0.0)
            } else {
                parse_finite(s)
            }
        }
        DataValue::Bool(b) if coercion.bool_to_number => Some(if *b { 1.0 } else { 0.0 }),
        DataValue::Null if coercion.null_to_zero => Some(0.0),
        _ => None,
    }
}

/// Config-aware arena-native i64 coercion. Honours the same
/// `numeric_coercion` flags as [`coerce_to_number_cfg`] but bails for
/// fractional values.
#[inline]
pub(crate) fn try_coerce_to_integer_cfg(v: &DataValue<'_>, engine: &crate::Engine) -> Option<i64> {
    let coercion = &engine.config().numeric_coercion;
    // `reject_non_numeric` overrides the fabricated coercions (see
    // `coerce_to_number_cfg`): only real numbers and numeric-looking strings
    // are accepted.
    if coercion.reject_non_numeric {
        return match v {
            DataValue::Number(n) => n.as_i64(),
            DataValue::String(s) => s.parse().ok(),
            _ => None,
        };
    }
    match v {
        DataValue::Number(n) => n.as_i64(),
        DataValue::String(s) => {
            if s.is_empty() && coercion.empty_string_to_zero {
                Some(0)
            } else {
                s.parse().ok()
            }
        }
        DataValue::Bool(b) if coercion.bool_to_number => Some(if *b { 1 } else { 0 }),
        DataValue::Null if coercion.null_to_zero => Some(0),
        _ => None,
    }
}
