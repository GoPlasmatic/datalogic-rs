use serde_json::Value;

use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

// Strict number extraction - only accepts actual numbers or numeric strings
#[inline]
fn get_number_strict(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Ceiling operator function (ceil)
#[inline]
pub fn evaluate_ceil(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
    }

    // Check if we have multiple arguments - if so, return array of ceil values
    if args.len() > 1 {
        let mut results = Vec::new();
        for arg in args {
            let value = engine.evaluate_node(arg, context)?;
            if let Some(num) = get_number_strict(&value) {
                let ceil_val = num.ceil();
                results.push(Value::Number((ceil_val as i64).into()));
            } else {
                return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
            }
        }
        return Ok(Value::Array(results));
    }

    // Single argument - evaluate and return ceil
    let value = engine.evaluate_node(&args[0], context)?;

    if let Some(num) = get_number_strict(&value) {
        let ceil_val = num.ceil();
        Ok(Value::Number((ceil_val as i64).into()))
    } else {
        Err(Error::InvalidArguments("Invalid Arguments".to_string()))
    }
}
