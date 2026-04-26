//! `merge` — flatten args into a single array, skipping nulls.

use crate::arena::{ArenaContextStack, ArenaValue, bvec};
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::item_is_null;

/// Arena-mode `merge`. Flattens its args (each may itself be a nested arena
/// op) into a single array, skipping nulls.
#[inline]
pub(crate) fn evaluate_merge_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // Pre-size for the scalar-arg case (one push per arg). Array args may push
    // more and trigger growth, but profile shows scalar/single-element args
    // dominate — saves the first reserve_internal_or_panic in the common case.
    let mut results = bvec::<ArenaValue<'a>>(arena, args.len());

    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        match av {
            // Direct arena Array (e.g. result of upstream arena filter/map).
            ArenaValue::Array(items) => {
                for item in items.iter() {
                    if !item_is_null(item) {
                        results.push(crate::arena::value::reborrow_arena_value(item));
                    }
                }
            }
            // Null inputs are skipped per merge semantics.
            ArenaValue::Null => {}
            // Scalar / object — push as-is.
            other => results.push(crate::arena::value::reborrow_arena_value(other)),
        }
    }

    Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())))
}
