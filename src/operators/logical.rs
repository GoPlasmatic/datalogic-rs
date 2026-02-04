use serde_json::Value;
use std::collections::HashMap;

use super::helpers::{check_invalid_args_marker, is_truthy};
use crate::trace::TraceCollector;
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// Logical NOT operator function (!)
#[inline]
pub fn evaluate_not(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Bool(true)); // !null = true
    }
    let value = engine.evaluate_node_cow(&args[0], context)?;
    Ok(Value::Bool(!is_truthy(&value, engine)))
}

/// Double NOT operator function (!!) - converts to boolean
#[inline]
pub fn evaluate_double_not(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Bool(false)); // !!null = false
    }
    let value = engine.evaluate_node_cow(&args[0], context)?;
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

    check_invalid_args_marker(args)?;

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

    check_invalid_args_marker(args)?;

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

    check_invalid_args_marker(args)?;

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

    check_invalid_args_marker(args)?;

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
