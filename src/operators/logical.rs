use super::helpers::check_invalid_args_marker;
use crate::arena::{DataContextStack, DataValue, is_truthy_arena};
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_not_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_true());
    }
    let v = engine.evaluate_node(&args[0], actx, arena)?;
    Ok(crate::arena::pool::singleton_bool(!is_truthy_arena(
        v, engine,
    )))
}

#[inline]
pub(crate) fn evaluate_double_not_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_false());
    }
    let v = engine.evaluate_node(&args[0], actx, arena)?;
    Ok(crate::arena::pool::singleton_bool(is_truthy_arena(
        v, engine,
    )))
}

#[inline]
pub(crate) fn evaluate_and_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_null());
    }
    check_invalid_args_marker(args)?;
    let mut last: &DataValue<'a> = crate::arena::pool::singleton_true();
    for arg in args {
        let v = engine.evaluate_node(arg, actx, arena)?;
        if !is_truthy_arena(v, engine) {
            return Ok(v);
        }
        last = v;
    }
    Ok(last)
}

#[inline]
pub(crate) fn evaluate_or_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_null());
    }
    check_invalid_args_marker(args)?;
    let mut last: &DataValue<'a> = crate::arena::pool::singleton_false();
    for arg in args {
        let v = engine.evaluate_node(arg, actx, arena)?;
        if is_truthy_arena(v, engine) {
            return Ok(v);
        }
        last = v;
    }
    Ok(last)
}
