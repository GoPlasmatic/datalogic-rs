//! `length` — string char count or array length.

use crate::arena::{ContextStack, DataValue};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

/// Arena-mode `length`. Critical for the COMPOSITION test: when called as
/// `length(filter(...))`, the filter result lives in the arena and length
/// just reads the slice length — zero conversion cost on the intermediate.
#[inline]
pub(crate) fn evaluate_length<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 1 {
        return Err(crate::constants::invalid_args());
    }

    // Recurse into arena dispatcher so composed cases (e.g. length(filter(...)))
    // stay arena-resident on the intermediate.
    let arg = engine.dispatch_node(&args[0], ctx, arena)?;

    let n: i64 = match arg {
        DataValue::String(s) => s.chars().count() as i64,
        DataValue::Array(items) => items.len() as i64,
        _ => return Err(crate::constants::invalid_args()),
    };

    if let Some(av) = crate::arena::pool::singleton_small_int(n) {
        return Ok(av);
    }
    Ok(arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(n))))
}
