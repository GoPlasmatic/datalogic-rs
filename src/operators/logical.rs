use super::helpers::check_invalid_args_marker;
use crate::arena::{ArenaContextStack, ArenaValue, is_truthy_arena};
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_not_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_true());
    }
    let v = engine.evaluate_arena_node(&args[0], actx, arena)?;
    Ok(crate::arena::pool::singleton_bool(!is_truthy_arena(
        v, engine,
    )))
}

#[inline]
pub(crate) fn evaluate_double_not_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_false());
    }
    let v = engine.evaluate_arena_node(&args[0], actx, arena)?;
    Ok(crate::arena::pool::singleton_bool(is_truthy_arena(
        v, engine,
    )))
}

#[inline]
pub(crate) fn evaluate_and_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    check_invalid_args_marker(args)?;
    let mut last: &ArenaValue<'a> = arena.alloc(ArenaValue::Bool(true));
    for arg in args {
        let v = engine.evaluate_arena_node(arg, actx, arena)?;
        if !is_truthy_arena(v, engine) {
            return Ok(v);
        }
        last = v;
    }
    Ok(last)
}

#[inline]
pub(crate) fn evaluate_or_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    check_invalid_args_marker(args)?;
    let mut last: &ArenaValue<'a> = arena.alloc(ArenaValue::Bool(false));
    for arg in args {
        let v = engine.evaluate_arena_node(arg, actx, arena)?;
        if is_truthy_arena(v, engine) {
            return Ok(v);
        }
        last = v;
    }
    Ok(last)
}
