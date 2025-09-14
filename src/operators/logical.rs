use serde_json::Value;

use super::helpers::is_truthy;
use crate::constants::INVALID_ARGS;
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// Logical NOT operator function (!)
#[inline]
pub fn evaluate_not(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let value = if args.is_empty() {
        Value::Null
    } else {
        engine.evaluate_node(&args[0], context)?
    };

    Ok(Value::Bool(!is_truthy(&value)))
}

/// Double NOT operator function (!!) - converts to boolean
#[inline]
pub fn evaluate_double_not(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let value = if args.is_empty() {
        Value::Null
    } else {
        engine.evaluate_node(&args[0], context)?
    };

    Ok(Value::Bool(is_truthy(&value)))
}

/// Logical AND operator function - returns first falsy or last value
#[inline]
pub fn evaluate_and(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Null);
    }

    // Check if we have the invalid args marker
    if args.len() == 1
        && let CompiledNode::Value { value, .. } = &args[0]
        && let Some(obj) = value.as_object()
        && obj.contains_key("__invalid_args__")
    {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let mut last_value = Value::Bool(true);

    for arg in args {
        let value = engine.evaluate_node(arg, context)?;
        if !is_truthy(&value) {
            return Ok(value);
        }
        last_value = value;
    }

    Ok(last_value)
}

/// Logical OR operator function - returns first truthy or last value
#[inline]
pub fn evaluate_or(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Null);
    }

    // Check if we have the invalid args marker
    if args.len() == 1
        && let CompiledNode::Value { value, .. } = &args[0]
        && let Some(obj) = value.as_object()
        && obj.contains_key("__invalid_args__")
    {
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let mut last_value = Value::Bool(false);

    for arg in args {
        let value = engine.evaluate_node(arg, context)?;
        if is_truthy(&value) {
            return Ok(value);
        }
        last_value = value;
    }

    Ok(last_value)
}
