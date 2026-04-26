//! Numeric coercion for `ArenaValue` — config-aware variants gated on the
//! engine's `numeric_coercion` settings, plus a default-rules variant for
//! contexts that intentionally bypass the engine config.

use super::ArenaValue;

/// Config-aware arena-native f64 coercion. Mirrors
/// `value_helpers::coerce_to_number` exactly — same engine config gates.
#[inline]
pub(crate) fn coerce_arena_to_number_cfg(
    v: &ArenaValue<'_>,
    engine: &crate::DataLogic,
) -> Option<f64> {
    match v {
        ArenaValue::Number(n) => Some(n.as_f64()),
        ArenaValue::String(s) => {
            if s.is_empty() && engine.config().numeric_coercion.empty_string_to_zero {
                Some(0.0)
            } else {
                s.parse().ok()
            }
        }
        ArenaValue::Bool(b) if engine.config().numeric_coercion.bool_to_number => {
            Some(if *b { 1.0 } else { 0.0 })
        }
        ArenaValue::Null if engine.config().numeric_coercion.null_to_zero => Some(0.0),
        _ => None,
    }
}

/// Config-aware arena-native i64 coercion. Mirrors
/// `value_helpers::try_coerce_to_integer`.
#[inline]
pub(crate) fn try_coerce_arena_to_integer_cfg(
    v: &ArenaValue<'_>,
    engine: &crate::DataLogic,
) -> Option<i64> {
    match v {
        ArenaValue::Number(n) => n.as_i64(),
        ArenaValue::String(s) => {
            if s.is_empty() && engine.config().numeric_coercion.empty_string_to_zero {
                Some(0)
            } else {
                s.parse().ok()
            }
        }
        ArenaValue::Bool(b) if engine.config().numeric_coercion.bool_to_number => {
            Some(if *b { 1 } else { 0 })
        }
        ArenaValue::Null if engine.config().numeric_coercion.null_to_zero => Some(0),
        _ => None,
    }
}

/// Coerce an `ArenaValue` to f64 using default JSON Logic coercion rules
/// (no engine config consulted). Used by datetime arithmetic where the
/// duration/scalar-multiply path runs before user config can intervene.
#[cfg(any(test, feature = "datetime"))]
pub(crate) fn coerce_arena_to_number(v: &ArenaValue<'_>) -> Option<f64> {
    match v {
        ArenaValue::Number(n) => Some(n.as_f64()),
        ArenaValue::Bool(true) => Some(1.0),
        ArenaValue::Bool(false) => Some(0.0),
        ArenaValue::Null => Some(0.0),
        ArenaValue::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Some(0.0)
            } else {
                t.parse().ok()
            }
        }
        ArenaValue::Array(items) => match items.len() {
            0 => Some(0.0),
            1 => coerce_arena_to_number(&items[0]),
            _ => None,
        },
        ArenaValue::Object(_) => None,
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(_) | ArenaValue::Duration(_) => None,
    }
}
