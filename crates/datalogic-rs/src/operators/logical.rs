use crate::arena::{ContextStack, DataValue, truthy_arena};
use crate::operators::eager::Cx;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

/// `!`: the negated truthiness of the argument; `true` with none.
#[inline]
pub(crate) fn not(_cx: &mut Cx<'_, '_>, value: Option<bool>) -> Result<bool> {
    Ok(!value.unwrap_or(false))
}

/// `!!`: the truthiness of the argument; `false` with none.
#[inline]
pub(crate) fn bool_cast(_cx: &mut Cx<'_, '_>, value: Option<bool>) -> Result<bool> {
    Ok(value.unwrap_or(false))
}

#[inline]
pub(crate) fn evaluate_and<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::singletons::singleton_null());
    }
    let mut last: &DataValue<'a> = crate::arena::singletons::singleton_true();
    for arg in args {
        let v = engine.dispatch_node(arg, ctx, arena)?;
        if !truthy_arena(v, engine) {
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
        return Ok(crate::arena::singletons::singleton_null());
    }
    let mut last: &DataValue<'a> = crate::arena::singletons::singleton_false();
    for arg in args {
        let v = engine.dispatch_node(arg, ctx, arena)?;
        if truthy_arena(v, engine) {
            return Ok(v);
        }
        last = v;
    }
    Ok(last)
}
