//! `merge` — flatten args into a single array, skipping nulls.

use crate::arena::{ContextStack, DataValue, bvec};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::item_is_null;

/// Arena-mode `merge`. Flattens its args (each may itself be a nested arena
/// op) into a single array, skipping nulls.
#[inline]
pub(crate) fn evaluate_merge<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // Pre-size for the scalar-arg case (one push per arg). Array args may push
    // more and trigger growth, but profile shows scalar/single-element args
    // dominate — saves the first reserve_internal_or_panic in the common case.
    let mut results = bvec::<DataValue<'a>>(arena, args.len());

    for arg in args {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        match av {
            // Direct arena Array (e.g. result of upstream arena filter/map).
            DataValue::Array(items) => {
                for item in items.iter() {
                    if !item_is_null(item) {
                        results.push(*item);
                    }
                }
            }
            // Null inputs are skipped per merge semantics.
            DataValue::Null => {}
            // Scalar / object — push as-is.
            other => results.push(*other),
        }
    }

    if results.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }
    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}
