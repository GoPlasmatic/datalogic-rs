use crate::CompiledNode;
use crate::config::TruthyEvaluator;
use datavalue::{NumberValue, OwnedDataValue};

/// Truthiness for an [`OwnedDataValue`] (compile-time literal form). The
/// runtime arena-resident form has its own [`crate::arena::truthy_arena`]
/// in the arena helpers module.
#[inline]
pub fn truthy_owned(value: &OwnedDataValue, engine: &crate::Engine) -> bool {
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript | TruthyEvaluator::Python => truthy_js_owned(value),
        TruthyEvaluator::StrictBoolean => match value {
            OwnedDataValue::Null => false,
            OwnedDataValue::Bool(b) => *b,
            _ => true,
        },
        #[cfg(feature = "compat")]
        TruthyEvaluator::Custom(f) => f(&crate::value::owned_to_serde(value)),
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
) -> Option<crate::datetime::DataDateTime> {
    use crate::arena::DataValue;
    match av {
        DataValue::DateTime(dt) => Some(*dt),
        DataValue::String(s) => crate::datetime::DataDateTime::parse(s),
        DataValue::Object(pairs) => {
            for (k, v) in *pairs {
                if *k == "datetime"
                    && let DataValue::String(s) = v
                {
                    return crate::datetime::DataDateTime::parse(s);
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
) -> Option<crate::datetime::DataDuration> {
    use crate::arena::DataValue;
    match av {
        DataValue::Duration(d) => Some(*d),
        DataValue::String(s) => crate::datetime::DataDuration::parse(s),
        DataValue::Object(pairs) => {
            for (k, v) in *pairs {
                if *k == "timestamp"
                    && let DataValue::String(s) = v
                {
                    return crate::datetime::DataDuration::parse(s);
                }
            }
            None
        }
        _ => None,
    }
}

/// Checks if args contain the `__invalid_args__` sentinel marker from compilation.
/// Returns an error if the marker is present, Ok(()) otherwise.
#[inline]
pub fn check_invalid_args_marker(args: &[CompiledNode]) -> crate::Result<()> {
    if args.len() == 1
        && let CompiledNode::Value { value, .. } = &args[0]
        && let Some(obj) = value.as_object()
        && obj.iter().any(|(k, _)| k == "__invalid_args__")
    {
        return Err(crate::constants::invalid_args());
    }
    Ok(())
}
