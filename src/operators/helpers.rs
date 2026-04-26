use crate::CompiledNode;
use crate::config::TruthyEvaluator;
use crate::constants::INVALID_ARGS;
use serde_json::Value;

/// Checks if a value is truthy using the engine's configuration
#[inline(always)]
pub fn is_truthy(value: &Value, engine: &crate::DataLogic) -> bool {
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript => is_truthy_js(value),
        TruthyEvaluator::Python => is_truthy_js(value),
        TruthyEvaluator::StrictBoolean => match value {
            Value::Null => false,
            Value::Bool(b) => *b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => f(value),
    }
}

/// Checks if a value is truthy according to JavaScript rules.
#[inline(always)]
pub fn is_truthy_js(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                f != 0.0 && !f.is_nan()
            } else {
                n.as_i64() != Some(0) && n.as_u64() != Some(0)
            }
        }
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
    }
}

/// Extracts a datetime from a value (either datetime object or string).
/// Single map lookup for objects — avoids the double lookup of is_datetime_object + extract_datetime.
#[cfg(feature = "datetime")]
#[inline]
pub fn extract_datetime_value(value: &Value) -> Option<crate::datetime::DataDateTime> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(s)) = map.get("datetime") {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            }
        }
        Value::String(s) => crate::datetime::DataDateTime::parse(s),
        _ => None,
    }
}

/// Extracts a duration from a value (either duration object or string).
/// Single map lookup for objects — avoids the double lookup of is_duration_object + extract_duration.
#[cfg(feature = "datetime")]
#[inline]
pub fn extract_duration_value(value: &Value) -> Option<crate::datetime::DataDuration> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(s)) = map.get("timestamp") {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            }
        }
        Value::String(s) => crate::datetime::DataDuration::parse(s),
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
        && obj.contains_key("__invalid_args__")
    {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }
    Ok(())
}
