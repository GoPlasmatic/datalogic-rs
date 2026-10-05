//! `distinct` — drop duplicate elements, by value or by a computed key.

use crate::arena::{ContextStack, DataValue, IterGuard, bvec};
use crate::operators::comparison::compare_equals;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::{Items, IterSrc};

/// `distinct: [array]` (dedup by value) or `distinct: [array, key_expr]`
/// (dedup by computed key). First occurrence wins in both forms, so output
/// order is the input order of survivors.
///
/// Equality is strict deep equality (`compare_equals` with `strict`), the
/// same predicate `in` uses — `1` and `"1"` stay distinct, objects and
/// arrays compare structurally. Dedup is a linear scan over the kept set
/// (`DataValue` has no `Hash`/`Ord`); see the note on `group_by` for the
/// upgrade path if this ever shows up in profiles. Each scan is charged the
/// size of the kept set before it runs, so n distinct values cost
/// n(n-1)/2 on top of the iterator's one per item.
///
/// The unkeyed form runs no callback and never touches the context stack,
/// which is what lets `opcode_is_static` classify it as fold-eligible.
#[inline]
pub(crate) fn evaluate_distinct<'a>(
    items: Items<'a>,
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // An object or scalar source has nothing to deduplicate.
    let Items::Array(src) = items else {
        return Err(crate::Error::invalid_args());
    };
    if args.len() < 2 {
        return distinct_by_value(&src, ctx, engine, arena);
    }
    distinct_by_key(&src, &args[1], ctx, engine, arena)
}

#[inline]
fn distinct_by_value<'a>(
    src: &IterSrc<'a>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let len = src.len();
    let mut kept = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        ctx.charge(kept.len() as u64)?;
        let mut seen = false;
        for prev in kept.iter() {
            if compare_equals(prev, item, true, engine, ctx)? {
                seen = true;
                break;
            }
        }
        if !seen {
            kept.push(*item);
        }
    }
    Ok(arena.alloc(DataValue::Array(kept.into_bump_slice())))
}

#[inline]
fn distinct_by_key<'a>(
    src: &IterSrc<'a>,
    key_expr: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let len = src.len();
    let mut seen_keys = bvec::<&'a DataValue<'a>>(arena, len);
    let mut kept = bvec::<DataValue<'a>>(arena, len);
    let mut guard = IterGuard::new(ctx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_indexed(item, i);
        let key = engine.dispatch_node(key_expr, guard.stack(), arena)?;

        guard.stack().charge(seen_keys.len() as u64)?;
        let mut seen = false;
        for prev in seen_keys.iter() {
            if compare_equals(prev, key, true, engine, guard.stack())? {
                seen = true;
                break;
            }
        }
        if !seen {
            seen_keys.push(key);
            kept.push(*item);
        }
    }
    drop(guard);
    Ok(arena.alloc(DataValue::Array(kept.into_bump_slice())))
}
