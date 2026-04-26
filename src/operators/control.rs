use serde_json::Value;

use super::helpers::check_invalid_args_marker;
use crate::{CompiledNode, DataLogic, Result};

// =============================================================================
// Arena-mode control operators
// =============================================================================

use crate::arena::{ArenaContextStack, ArenaValue, is_truthy_arena};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_if_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    check_invalid_args_marker(args)?;

    if args.len() == 3 {
        let cond = engine.evaluate_arena_node(&args[0], actx, arena)?;
        let idx = if is_truthy_arena(cond, engine) { 1 } else { 2 };
        return engine.evaluate_arena_node(&args[idx], actx, arena);
    }

    let mut i = 0;
    while i < args.len() {
        if i == args.len() - 1 {
            return engine.evaluate_arena_node(&args[i], actx, arena);
        }
        let cond = engine.evaluate_arena_node(&args[i], actx, arena)?;
        if is_truthy_arena(cond, engine) {
            if i + 1 < args.len() {
                return engine.evaluate_arena_node(&args[i + 1], actx, arena);
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
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 3 {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    let cond = engine.evaluate_arena_node(&args[0], actx, arena)?;
    if is_truthy_arena(cond, engine) {
        engine.evaluate_arena_node(&args[1], actx, arena)
    } else {
        engine.evaluate_arena_node(&args[2], actx, arena)
    }
}

#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_coalesce_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    for arg in args {
        let v = engine.evaluate_arena_node(arg, actx, arena)?;
        // Non-null check on ArenaValue
        let is_null =
            matches!(v, ArenaValue::Null) || matches!(v, ArenaValue::InputRef(Value::Null));
        if !is_null {
            return Ok(v);
        }
    }
    Ok(arena.alloc(ArenaValue::Null))
}

#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_switch_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    use crate::arena::arena_to_value_cow;
    if args.len() < 2 {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    let disc_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let disc = arena_to_value_cow(disc_av);

    match &args[1] {
        CompiledNode::Array { nodes, .. } => {
            for case_node in nodes.iter() {
                match case_node {
                    CompiledNode::Array { nodes: pair, .. } if pair.len() >= 2 => {
                        let cv_av = engine.evaluate_arena_node(&pair[0], actx, arena)?;
                        let cv = arena_to_value_cow(cv_av);
                        if *disc == *cv {
                            return engine.evaluate_arena_node(&pair[1], actx, arena);
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
        return engine.evaluate_arena_node(&args[2], actx, arena);
    }
    Ok(arena.alloc(ArenaValue::Null))
}
