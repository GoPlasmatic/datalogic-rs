use crate::arena::{ContextStack, DataValue, truthy_arena};
use crate::operators::eager::Cx;
use crate::operators::meta::Logic;
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

/// `and` / `or`: evaluate left to right and return the first value whose
/// truthiness is the chain's absorbing one (falsy for `and`, truthy for
/// `or`), or the last value if none is.
#[inline]
pub(crate) fn short_circuit<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: Logic,
) -> Result<&'a DataValue<'a>> {
    let last = args.len() - 1;
    for arg in &args[..last] {
        let v = engine.dispatch_node(arg, ctx, arena)?;
        if truthy_arena(v, engine) == op.absorbing() {
            return Ok(v);
        }
    }
    engine.dispatch_node(&args[last], ctx, arena)
}
