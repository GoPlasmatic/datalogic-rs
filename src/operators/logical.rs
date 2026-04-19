use serde_json::Value;
use std::borrow::Cow;

use super::helpers::{check_invalid_args_marker, is_truthy};
use crate::eval_mode::Mode;
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// Logical NOT operator function (!)
#[inline(always)]
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
#[inline(always)]
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

/// Logical AND operator - returns first falsy or last value.
///
/// Generic over [`Mode`] so plain and traced dispatch share a single body.
#[inline]
pub fn evaluate_and<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Null);
    }

    check_invalid_args_marker(args)?;

    // Fast path: 2-arg case (the common shape). Avoids the loop and the
    // initial `Cow::Owned(Bool(true))` allocation that the variadic path
    // pays for every call.
    if args.len() == 2 {
        let a = engine.evaluate_node_cow_with_mode::<M>(&args[0], context, mode)?;
        if !is_truthy(&a, engine) {
            return Ok(a.into_owned());
        }
        return engine.evaluate_node_with_mode::<M>(&args[1], context, mode);
    }

    let mut last_value: Cow<'_, Value> = Cow::Owned(Value::Bool(true));

    for arg in args {
        let value = engine.evaluate_node_cow_with_mode::<M>(arg, context, mode)?;
        if !is_truthy(&value, engine) {
            return Ok(value.into_owned());
        }
        last_value = value;
    }

    Ok(last_value.into_owned())
}

/// Logical OR operator - returns first truthy or last value.
///
/// Generic over [`Mode`] so plain and traced dispatch share a single body.
#[inline]
pub fn evaluate_or<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Null);
    }

    check_invalid_args_marker(args)?;

    // Fast path: 2-arg case. Same rationale as `evaluate_and` above.
    if args.len() == 2 {
        let a = engine.evaluate_node_cow_with_mode::<M>(&args[0], context, mode)?;
        if is_truthy(&a, engine) {
            return Ok(a.into_owned());
        }
        return engine.evaluate_node_with_mode::<M>(&args[1], context, mode);
    }

    let mut last_value: Cow<'_, Value> = Cow::Owned(Value::Bool(false));

    for arg in args {
        let value = engine.evaluate_node_cow_with_mode::<M>(arg, context, mode)?;
        if is_truthy(&value, engine) {
            return Ok(value.into_owned());
        }
        last_value = value;
    }

    Ok(last_value.into_owned())
}
