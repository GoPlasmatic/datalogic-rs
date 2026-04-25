use serde_json::Value;

use super::helpers::{check_invalid_args_marker, is_truthy};
use crate::eval_mode::Mode;
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// If operator — supports `if/then/else` and `if/elseif/.../else` chains.
///
/// Generic over [`Mode`] so plain and traced dispatch share a single body.
/// Only the selected branch is evaluated (and therefore traced).
#[inline]
pub fn evaluate_if<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Null);
    }

    check_invalid_args_marker(args)?;

    // Fast path: the overwhelmingly common 3-arg if/then/else shape.
    // Avoids the variadic-chain loop's bookkeeping.
    if args.len() == 3 {
        let condition = engine.evaluate_node_cow_with_mode::<M>(&args[0], context, mode)?;
        let idx = if is_truthy(&condition, engine) { 1 } else { 2 };
        return engine.evaluate_node_with_mode::<M>(&args[idx], context, mode);
    }

    // Variadic if/elseif/else chains
    let mut i = 0;
    while i < args.len() {
        if i == args.len() - 1 {
            // Final else clause
            return engine.evaluate_node_with_mode::<M>(&args[i], context, mode);
        }

        // Evaluate condition using Cow to avoid cloning literals
        let condition = engine.evaluate_node_cow_with_mode::<M>(&args[i], context, mode)?;
        if is_truthy(&condition, engine) {
            // Evaluate then branch
            if i + 1 < args.len() {
                return engine.evaluate_node_with_mode::<M>(&args[i + 1], context, mode);
            } else {
                return Ok(condition.into_owned());
            }
        }

        // Move to next if/elseif pair
        i += 2;
    }

    Ok(Value::Null)
}

/// Ternary operator (`?:`) — only evaluates the selected branch.
#[inline(always)]
pub fn evaluate_ternary<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.len() < 3 {
        return Ok(Value::Null);
    }

    let condition = engine.evaluate_node_cow_with_mode::<M>(&args[0], context, mode)?;

    if is_truthy(&condition, engine) {
        engine.evaluate_node_with_mode::<M>(&args[1], context, mode)
    } else {
        engine.evaluate_node_with_mode::<M>(&args[2], context, mode)
    }
}

/// Coalesce operator (`??`) — returns first non-null value.
#[cfg(feature = "ext-control")]
#[inline]
pub fn evaluate_coalesce<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    // Empty args returns null
    if args.is_empty() {
        return Ok(Value::Null);
    }

    // Return the first non-null value
    for arg in args {
        let value = engine.evaluate_node_cow_with_mode::<M>(arg, context, mode)?;
        if *value != Value::Null {
            return Ok(value.into_owned());
        }
    }

    // All values were null
    Ok(Value::Null)
}

/// Switch/match operator — evaluates discriminant once and matches against case pairs.
#[cfg(feature = "ext-control")]
#[inline]
pub fn evaluate_switch<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    // args[0] = discriminant, args[1] = cases array, args[2] = optional default
    if args.len() < 2 {
        return Ok(Value::Null);
    }

    // Evaluate discriminant once
    let discriminant = engine.evaluate_node_with_mode::<M>(&args[0], context, mode)?;

    // Cases should be a CompiledNode::Array of [match_value, result] pairs
    // or a CompiledNode::Value containing a pre-evaluated array (from static optimization)
    match &args[1] {
        CompiledNode::Array { nodes, .. } => {
            for case_node in nodes.iter() {
                match case_node {
                    CompiledNode::Array { nodes: pair, .. } if pair.len() >= 2 => {
                        let case_value =
                            engine.evaluate_node_with_mode::<M>(&pair[0], context, mode)?;
                        if discriminant == case_value {
                            return engine.evaluate_node_with_mode::<M>(&pair[1], context, mode);
                        }
                    }
                    CompiledNode::Value {
                        value: Value::Array(pair),
                        ..
                    } if pair.len() >= 2 => {
                        // Static-optimized pair
                        if discriminant == pair[0] {
                            return Ok(pair[1].clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        CompiledNode::Value {
            value: Value::Array(cases),
            ..
        } => {
            // Entire cases array was statically evaluated
            for case in cases {
                if let Value::Array(pair) = case
                    && pair.len() >= 2
                    && discriminant == pair[0]
                {
                    return Ok(pair[1].clone());
                }
            }
        }
        _ => {}
    }

    // No match found - evaluate default if present
    if args.len() > 2 {
        return engine.evaluate_node_with_mode::<M>(&args[2], context, mode);
    }

    Ok(Value::Null)
}

// =============================================================================
// Arena-mode control operators
// =============================================================================

use crate::arena::{ArenaContextStack, ArenaValue, is_truthy_arena};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_if_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    check_invalid_args_marker(args)?;

    if args.len() == 3 {
        let cond = engine.evaluate_arena_node(&args[0], actx, context, arena)?;
        let idx = if is_truthy_arena(cond, engine) { 1 } else { 2 };
        return engine.evaluate_arena_node(&args[idx], actx, context, arena);
    }

    let mut i = 0;
    while i < args.len() {
        if i == args.len() - 1 {
            return engine.evaluate_arena_node(&args[i], actx, context, arena);
        }
        let cond = engine.evaluate_arena_node(&args[i], actx, context, arena)?;
        if is_truthy_arena(cond, engine) {
            if i + 1 < args.len() {
                return engine.evaluate_arena_node(&args[i + 1], actx, context, arena);
            } else {
                return Ok(cond);
            }
        }
        i += 2;
    }
    Ok(arena.alloc(ArenaValue::Null))
}

#[inline]
pub(crate) fn evaluate_ternary_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 3 {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    let cond = engine.evaluate_arena_node(&args[0], actx, context, arena)?;
    if is_truthy_arena(cond, engine) {
        engine.evaluate_arena_node(&args[1], actx, context, arena)
    } else {
        engine.evaluate_arena_node(&args[2], actx, context, arena)
    }
}

#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_coalesce_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    for arg in args {
        let v = engine.evaluate_arena_node(arg, actx, context, arena)?;
        // Non-null check on ArenaValue
        let is_null = matches!(v, ArenaValue::Null)
            || matches!(v, ArenaValue::InputRef(Value::Null));
        if !is_null {
            return Ok(v);
        }
    }
    Ok(arena.alloc(ArenaValue::Null))
}

#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_switch_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    use crate::arena::arena_to_value_cow;
    if args.len() < 2 {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    let disc_av = engine.evaluate_arena_node(&args[0], actx, context, arena)?;
    let disc = arena_to_value_cow(disc_av);

    match &args[1] {
        CompiledNode::Array { nodes, .. } => {
            for case_node in nodes.iter() {
                match case_node {
                    CompiledNode::Array { nodes: pair, .. } if pair.len() >= 2 => {
                        let cv_av = engine.evaluate_arena_node(&pair[0], actx, context, arena)?;
                        let cv = arena_to_value_cow(cv_av);
                        if *disc == *cv {
                            return engine.evaluate_arena_node(&pair[1], actx, context, arena);
                        }
                    }
                    CompiledNode::Value {
                        value: Value::Array(pair),
                        ..
                    } if pair.len() >= 2 => {
                        if *disc == pair[0] {
                            return Ok(arena.alloc(crate::arena::value_to_arena(&pair[1], arena)));
                        }
                    }
                    _ => {}
                }
            }
        }
        CompiledNode::Value {
            value: Value::Array(cases),
            ..
        } => {
            for case in cases {
                if let Value::Array(pair) = case
                    && pair.len() >= 2
                    && *disc == pair[0]
                {
                    return Ok(arena.alloc(crate::arena::value_to_arena(&pair[1], arena)));
                }
            }
        }
        _ => {}
    }

    if args.len() > 2 {
        return engine.evaluate_arena_node(&args[2], actx, context, arena);
    }
    Ok(arena.alloc(ArenaValue::Null))
}
