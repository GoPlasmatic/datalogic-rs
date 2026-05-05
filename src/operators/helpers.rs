use crate::config::TruthyEvaluator;
use datavalue::{NumberValue, OwnedDataValue};

/// Truthiness for an [`OwnedDataValue`] (compile-time literal form). The
/// runtime arena-resident form has its own [`crate::arena::truthy_arena`]
/// in the arena helpers module.
#[inline]
pub(crate) fn truthy_owned(value: &OwnedDataValue, engine: &crate::Engine) -> bool {
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript | TruthyEvaluator::Python => truthy_js_owned(value),
        TruthyEvaluator::StrictBoolean => match value {
            OwnedDataValue::Null => false,
            OwnedDataValue::Bool(b) => *b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => f(value),
    }
}

#[inline]
fn truthy_js_owned(value: &OwnedDataValue) -> bool {
    match value {
        OwnedDataValue::Null => false,
        OwnedDataValue::Bool(b) => *b,
        OwnedDataValue::Number(NumberValue::Integer(i)) => *i != 0,
        OwnedDataValue::Number(NumberValue::Float(f)) => *f != 0.0 && !f.is_nan(),
        OwnedDataValue::String(s) => !s.is_empty(),
        OwnedDataValue::Array(items) => !items.is_empty(),
        OwnedDataValue::Object(pairs) => !pairs.is_empty(),
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(_) | OwnedDataValue::Duration(_) => true,
    }
}

/// Arena-native datetime extraction — walks `String` / `Object` arena values
/// directly without `Value` materialization.
#[cfg(feature = "datetime")]
#[inline]
pub(crate) fn extract_datetime(
    av: &crate::arena::DataValue<'_>,
) -> Option<datavalue::DataDateTime> {
    use crate::arena::DataValue;
    match av {
        DataValue::DateTime(dt) => Some(*dt),
        DataValue::String(s) => datavalue::DataDateTime::parse(s),
        DataValue::Object(pairs) => {
            for (k, v) in *pairs {
                if *k == "datetime"
                    && let DataValue::String(s) = v
                {
                    return datavalue::DataDateTime::parse(s);
                }
            }
            None
        }
        _ => None,
    }
}

/// Arena-native duration extraction. See [`extract_datetime`].
#[cfg(feature = "datetime")]
#[inline]
pub(crate) fn extract_duration(
    av: &crate::arena::DataValue<'_>,
) -> Option<datavalue::DataDuration> {
    use crate::arena::DataValue;
    match av {
        DataValue::Duration(d) => Some(*d),
        DataValue::String(s) => datavalue::DataDuration::parse(s),
        DataValue::Object(pairs) => {
            for (k, v) in *pairs {
                if *k == "timestamp"
                    && let DataValue::String(s) = v
                {
                    return datavalue::DataDuration::parse(s);
                }
            }
            None
        }
        _ => None,
    }
}

