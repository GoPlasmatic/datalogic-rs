//! `merge` — flatten args into a single array, skipping nulls.

use crate::arena::{ContextStack, DataValue, bvec};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::item_is_null;

/// Arena-mode `merge`. Flattens its args (each may itself be a nested arena
/// op) into a single array, skipping nulls.
///
/// The result buffer is allocated lazily on the first non-null push so
/// "merge with all-null args" and "merge with no args" return the
/// empty-array singleton without touching the arena. Array args may push
/// many items and trigger growth, but profile shows scalar/single-element
/// args dominate — pre-size the buffer to `args.len()` on first push to
/// avoid the immediate-grow that the previous unconditional bvec was
/// already paying for.
#[inline]
pub(crate) fn evaluate_merge<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let mut results: Option<bumpalo::collections::Vec<'a, DataValue<'a>>> = None;
    let mut push = |item: DataValue<'a>| {
        results
            .get_or_insert_with(|| bvec::<DataValue<'a>>(arena, args.len().max(1)))
            .push(item);
    };

    for arg in args {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        match av {
            // Direct arena Array (e.g. result of upstream arena filter/map).
            DataValue::Array(items) => {
                for item in items.iter() {
                    if !item_is_null(item) {
                        push(*item);
                    }
                }
            }
            // Null inputs are skipped per merge semantics.
            DataValue::Null => {}
            // Scalar / object — push as-is.
            other => push(*other),
        }
    }

    match results {
        Some(v) if !v.is_empty() => Ok(arena.alloc(DataValue::Array(v.into_bump_slice()))),
        _ => Ok(crate::arena::singletons::singleton_empty_array()),
    }
}
