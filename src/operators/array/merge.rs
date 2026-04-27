//! `merge` — flatten args into a single array, skipping nulls.

use crate::arena::{DataContextStack, DataValue, bvec};
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::item_is_null;

/// Arena-mode `merge`. Flattens its args (each may itself be a nested arena
/// op) into a single array, skipping nulls.
#[inline]
pub(crate) fn evaluate_merge_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // Pre-size for the scalar-arg case (one push per arg). Array args may push
    // more and trigger growth, but profile shows scalar/single-element args
    // dominate — saves the first reserve_internal_or_panic in the common case.
    let mut results = bvec::<DataValue<'a>>(arena, args.len());

    for arg in args {
        let av = engine.evaluate_node(arg, actx, arena)?;
        match av {
            // Direct arena Array (e.g. result of upstream arena filter/map).
            DataValue::Array(items) => {
                for item in items.iter() {
                    if !item_is_null(item) {
                        results.push(crate::arena::value::reborrow_arena_value(item));
                    }
                }
            }
            // Null inputs are skipped per merge semantics.
            DataValue::Null => {}
            // Scalar / object — push as-is.
            other => results.push(crate::arena::value::reborrow_arena_value(other)),
        }
    }

    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}
