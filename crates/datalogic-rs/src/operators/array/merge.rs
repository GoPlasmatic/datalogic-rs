//! `merge` — flatten args into a single array, skipping nulls.

use crate::Result;
use crate::arena::{DataValue, bvec};
use crate::operators::eager::Cx;
use crate::operators::extract::{Any, RestArgs};

/// `merge`: its arguments flattened into one array, skipping nulls (each
/// argument may itself be a nested arena op).
///
/// The result buffer is allocated lazily on the first non-null push so
/// "merge with all-null args" and "merge with no args" return the
/// empty-array singleton without touching the arena. Array args may push
/// many items and trigger growth, but profile shows scalar/single-element
/// args dominate — pre-size the buffer to `args.len()` on first push to
/// avoid the immediate-grow that the previous unconditional bvec was
/// already paying for.
///
/// Charges one operation per item of each array argument, before copying
/// them, nulls included since each is examined to be skipped. The node
/// charge alone would price an accumulator (`merge` of the accumulator and
/// one item, inside `reduce`) at a constant per step while it copies the
/// whole accumulator each time (#77).
#[inline]
pub(crate) fn merge<'a>(
    cx: &mut Cx<'_, 'a>,
    parts: RestArgs<'a, Any>,
) -> Result<bumpalo::collections::Vec<'a, DataValue<'a>>> {
    let arena = cx.arena;
    let mut results: Option<bumpalo::collections::Vec<'a, DataValue<'a>>> = None;
    let mut push = |item: DataValue<'a>| {
        results
            .get_or_insert_with(|| bvec::<DataValue<'a>>(arena, parts.len().max(1)))
            .push(item);
    };

    for i in 0..parts.len() {
        match parts.get(i, cx)? {
            // Direct arena Array (e.g. result of upstream arena filter/map).
            DataValue::Array(items) => {
                cx.charge(items.len() as u64)?;
                for item in items.iter() {
                    if !item.is_null() {
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

    // No pushes: an empty `Vec` allocates nothing and converts to the
    // shared empty-array singleton.
    Ok(results.unwrap_or_else(|| bumpalo::collections::Vec::new_in(arena)))
}
