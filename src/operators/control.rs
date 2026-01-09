use serde_json::Value;

use super::helpers::is_truthy;
use crate::constants::INVALID_ARGS;
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// If operator function - supports if/then/else and if/elseif/else chains
#[inline]
pub fn evaluate_if(
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
        return Err(crate::Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    // Support variadic if/elseif/else chains
    let mut i = 0;
    while i < args.len() {
        if i == args.len() - 1 {
            // Final else clause
            return engine.evaluate_node(&args[i], context);
        }

        // Evaluate condition
        let condition = engine.evaluate_node(&args[i], context)?;
        if is_truthy(&condition, engine) {
            // Evaluate then branch
            if i + 1 < args.len() {
                return engine.evaluate_node(&args[i + 1], context);
            } else {
                return Ok(condition);
            }
        }

        // Move to next if/elseif pair
        i += 2;
    }

    Ok(Value::Null)
}

/// Ternary operator function (?:)
#[inline]
pub fn evaluate_ternary(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 3 {
        return Ok(Value::Null);
    }

    let condition = engine.evaluate_node(&args[0], context)?;

    if is_truthy(&condition, engine) {
        engine.evaluate_node(&args[1], context)
    } else {
        engine.evaluate_node(&args[2], context)
    }
}

/// Coalesce operator function (??) - returns first non-null value
#[inline]
pub fn evaluate_coalesce(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // Empty args returns null
    if args.is_empty() {
        return Ok(Value::Null);
    }

    // Return the first non-null value
    for arg in args {
        let value = engine.evaluate_node(arg, context)?;
        if value != Value::Null {
            return Ok(value);
        }
    }

    // All values were null
    Ok(Value::Null)
}
