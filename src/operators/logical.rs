use serde_json::Value;
use std::collections::HashMap;

use super::helpers::is_truthy;
use crate::constants::INVALID_ARGS;
use crate::trace::TraceCollector;
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

    Ok(Value::Bool(!is_truthy(&value, engine)))
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

    Ok(Value::Bool(is_truthy(&value, engine)))
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
        if !is_truthy(&value, engine) {
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
        if is_truthy(&value, engine) {
            return Ok(value);
        }
        last_value = value;
    }

    Ok(last_value)
}

// ============================================================================
// Traced versions of short-circuit logical operators
// ============================================================================

/// Traced version of `and` operator - only evaluates until first falsy value.
#[inline]
pub fn evaluate_and_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
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
        let value = engine.evaluate_node_traced(arg, context, collector, node_id_map)?;
        if !is_truthy(&value, engine) {
            // Short-circuit: stop here, don't evaluate remaining args
            return Ok(value);
        }
        last_value = value;
    }

    Ok(last_value)
}

/// Traced version of `or` operator - only evaluates until first truthy value.
#[inline]
pub fn evaluate_or_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
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
        let value = engine.evaluate_node_traced(arg, context, collector, node_id_map)?;
        if is_truthy(&value, engine) {
            // Short-circuit: stop here, don't evaluate remaining args
            return Ok(value);
        }
        last_value = value;
    }

    Ok(last_value)
}
