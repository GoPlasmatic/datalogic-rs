//! Numeric coercion for `DataValue` — config-aware variants gated on the
//! engine's `numeric_coercion` settings, plus a default-rules variant for
//! contexts that intentionally bypass the engine config.

use super::DataValue;

/// Config-aware arena-native f64 coercion. Mirrors
/// `value_helpers::coerce_to_number` exactly — same engine config gates.
#[inline]
pub(crate) fn coerce_arena_to_number_cfg(
    v: &DataValue<'_>,
    engine: &crate::DataLogic,
) -> Option<f64> {
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

/// Config-aware arena-native i64 coercion. Mirrors
/// `value_helpers::try_coerce_to_integer`.
#[inline]
pub(crate) fn try_coerce_arena_to_integer_cfg(
    v: &DataValue<'_>,
    engine: &crate::DataLogic,
) -> Option<i64> {
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
#[cfg(any(test, feature = "datetime"))]
pub(crate) fn coerce_arena_to_number(v: &DataValue<'_>) -> Option<f64> {
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
            1 => coerce_arena_to_number(&items[0]),
            _ => None,
        },
        DataValue::Object(_) => None,
        #[cfg(feature = "datetime")]
        DataValue::DateTime(_) | DataValue::Duration(_) => None,
    }
}
