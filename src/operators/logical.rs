use super::helpers::check_invalid_args_marker;
use crate::arena::{ContextStack, DataValue, is_truthy};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_not<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_true());
    }
    let v = engine.dispatch_node(&args[0], ctx, arena)?;
    Ok(crate::arena::pool::singleton_bool(!is_truthy(v, engine)))
}

#[inline]
pub(crate) fn evaluate_bool_cast<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_false());
    }
    let v = engine.dispatch_node(&args[0], ctx, arena)?;
    Ok(crate::arena::pool::singleton_bool(is_truthy(v, engine)))
}

#[inline]
pub(crate) fn evaluate_and<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_null());
    }
    check_invalid_args_marker(args)?;
    let mut last: &DataValue<'a> = crate::arena::pool::singleton_true();
    for arg in args {
        let v = engine.dispatch_node(arg, ctx, arena)?;
        if !is_truthy(v, engine) {
            return Ok(v);
        }
        last = v;
    }
    Ok(last)
}

#[inline]
pub(crate) fn evaluate_or<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_null());
    }
    check_invalid_args_marker(args)?;
    let mut last: &DataValue<'a> = crate::arena::pool::singleton_false();
    for arg in args {
        let v = engine.dispatch_node(arg, ctx, arena)?;
        if is_truthy(v, engine) {
            return Ok(v);
        }
        last = v;
    }
    Ok(last)
}
