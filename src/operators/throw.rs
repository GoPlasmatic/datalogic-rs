use serde_json::Value;

use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};
use std::collections::HashMap;

/// Throw operator function - throws an error with a type
#[inline]
pub fn evaluate_throw(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let error_value = if args.is_empty() {
        Value::Null
    } else if let CompiledNode::Value { value } = &args[0] {
        // Fast path: access literal directly without evaluate_node dispatch
        value.clone()
    } else {
        engine.evaluate_node(&args[0], context)?
    };

    // If the error value is an object with a "type" field, use that as the error
    // Otherwise, convert the value to a string and use it as the error type
    let error_obj = if let Value::Object(_) = &error_value {
        error_value
    } else if let Value::String(s) = &error_value {
        // Create an error object with the string as the type
        serde_json::json!({"type": s})
    } else {
        // For other types, convert to string and use as type
        serde_json::json!({"type": error_value.to_string()})
    };

    Err(Error::Thrown(error_obj))
}

/// Traced version of throw - evaluates argument with tracing before throwing
#[inline]
pub fn evaluate_throw_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut crate::trace::TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    let error_value = if args.is_empty() {
        Value::Null
    } else {
        engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?
    };

    let error_obj = if let Value::Object(_) = &error_value {
        error_value
    } else if let Value::String(s) = &error_value {
        serde_json::json!({"type": s})
    } else {
        serde_json::json!({"type": error_value.to_string()})
    };

    Err(Error::Thrown(error_obj))
}
