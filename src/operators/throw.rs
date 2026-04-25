use serde_json::Value;

use crate::eval_mode::Mode;
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Throw operator — throws an error built from its argument.
///
/// Generic over [`Mode`] so plain and traced dispatch share the same body.
#[inline]
pub fn evaluate_throw<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    let error_value = if args.is_empty() {
        Value::Null
    } else if let CompiledNode::Value { value, .. } = &args[0] {
        // Fast path: access literal directly without evaluate_node dispatch
        value.clone()
    } else {
        engine.evaluate_node_with_mode::<M>(&args[0], context, mode)?
    };

    // If the error value is an object with a "type" field, use that as the error.
    // Otherwise, convert the value to a string and use it as the error type.
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

// =============================================================================
// Arena-mode throw
// =============================================================================

use crate::arena::{ArenaContextStack, ArenaValue, arena_to_value};
use bumpalo::Bump;

/// Native arena-mode `throw`. Builds the error object directly from the
/// argument's arena-resolved form — no value-mode round-trip.
#[inline]
pub(crate) fn evaluate_throw_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let error_obj: Value = if args.is_empty() {
        Value::Null
    } else if let CompiledNode::Value { value, .. } = &args[0] {
        // Literal fast path — skip arena dispatch.
        value.clone()
    } else {
        let av = engine.evaluate_arena_node(&args[0], actx, context, arena)?;
        arena_to_value(av)
    };

    let error_obj = match error_obj {
        Value::Object(_) => error_obj,
        Value::String(s) => serde_json::json!({"type": s}),
        other => serde_json::json!({"type": other.to_string()}),
    };

    Err(Error::Thrown(error_obj))
}
