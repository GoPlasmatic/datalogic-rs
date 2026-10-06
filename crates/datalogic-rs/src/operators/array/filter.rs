//! `filter` — keep array items / object pairs whose predicate is truthy.

use crate::arena::{ContextStack, DataValue, bvec};
use crate::operators::meta::{Algebra, EqOp};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use std::ops::ControlFlow;

use super::fused::FieldCursor;
use super::helpers::{FastPredicate, try_extract_filter_field_cmp};
use super::input::{Items, IterSrc, for_each_iter_array, for_each_iter_object};

/// `filter`: the items (or object pairs) whose predicate is truthy. A
/// scalar source is an error.
#[inline]
pub(crate) fn evaluate_filter<'a>(
    items: Items<'a>,
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let predicate = &args[1];
    let src = match items {
        Items::Array(src) => src,
        Items::Object(pairs) => return filter_bridge_object(pairs, predicate, ctx, engine, arena),
        Items::Scalar(_) => return Err(crate::Error::invalid_args()),
    };

    // Fast paths bypass `run_iter_body` and skip tracer markers. Defer to the
    // general path when a tracer is attached.
    if super::fast_paths::allowed(ctx, engine) {
        if let Some(result) = filter_strict_eq_field_fast_path(&src, predicate, ctx, engine, arena)?
        {
            return Ok(result);
        }

        if let Some(fast_pred) = FastPredicate::from_node(predicate)
            && let Some(result) = filter_with_fast_predicate(&src, fast_pred, engine, arena)
        {
            return Ok(result);
        }
    }

    filter_general(&src, predicate, ctx, engine, arena)
}

/// Fast path for `filter(arr, == [{var: "field"}, invariant])` — direct field
/// traversal + invariant comparison, no context push, no item clone.
#[inline]
fn filter_strict_eq_field_fast_path<'a>(
    src: &IterSrc<'a>,
    predicate: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<Option<&'a DataValue<'a>>> {
    let Some((segments, invariant_node, negate)) = strict_eq_field_shape(predicate) else {
        return Ok(None);
    };

    // Evaluated once, with no per-item frame pushed: `is_filter_invariant`
    // admits only literals and root-bound references, neither of which
    // reads the frame stack.
    let invariant_val = engine.dispatch_node(invariant_node, ctx, arena)?;
    let is_eq = !negate;
    let len = src.len();
    // Local hinted cursor — homogeneous rows resolve the field in one key
    // compare after the first item (same mechanism as the map fast paths).
    let mut field = FieldCursor::new(segments);
    let mut results = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        // A missing field is `var`'s implicit null. Compared as `===`
        // compares, so numbers compare as `f64` and datetime strings as
        // instants, whatever path the filter takes.
        let av = field.resolve(item).unwrap_or(&DataValue::Null);
        let matches =
            crate::operators::comparison::compare_equals(av, invariant_val, true, engine, ctx)?;
        if matches == is_eq {
            results.push(*item);
        }
    }
    if results.is_empty() {
        return Ok(Some(crate::arena::singletons::singleton_empty_array()));
    }
    Ok(Some(
        arena.alloc(DataValue::Array(results.into_bump_slice())),
    ))
}

/// The strict-equality fast path's shape: `{"===" | "!==": [field, x]}`
/// (either order) where `field` is a plain element field and `x` is loop
/// invariant. Returns the field's segments, `x`, and whether it negates.
/// Listed in [`super::fast_paths`].
#[inline]
pub(super) fn strict_eq_field_shape(
    predicate: &CompiledNode,
) -> Option<(&[crate::node::PathSegment], &CompiledNode, bool)> {
    let CompiledNode::BuiltinOperator {
        opcode,
        args: pred_args,
        ..
    } = predicate
    else {
        return None;
    };
    let Some(Algebra::Eq(EqOp {
        strict: true,
        negate,
    })) = opcode.algebra()
    else {
        return None;
    };
    if pred_args.len() != 2 {
        return None;
    }
    let (segments, invariant) = try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
        .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]))?;
    Some((segments, invariant, negate))
}

/// Filter using a `FastPredicate` — predicate evaluates in-place against each
/// item with zero context push and zero per-item allocation. Returns `None`
/// when any item evaluates indeterminate (see
/// [`FastPredicate::evaluate_opt`]); the caller re-runs the whole collection
/// through the general path, which is exact because fast evaluation is pure.
#[inline]
fn filter_with_fast_predicate<'a>(
    src: &IterSrc<'a>,
    fast_pred: &FastPredicate,
    engine: &Engine,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let mut results = bvec::<DataValue<'a>>(arena, src.len());
    fast_pred.scan(src, engine, |item, keep| {
        if keep {
            results.push(*item);
        }
        ControlFlow::Continue(())
    })?;
    if results.is_empty() {
        return Some(crate::arena::singletons::singleton_empty_array());
    }
    Some(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// General filter path — dispatches the predicate per item via the arena
/// context stack.
#[inline]
fn filter_general<'a>(
    src: &IterSrc<'a>,
    predicate: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let mut results = bvec::<DataValue<'a>>(arena, src.len());
    for_each_iter_array(src.0, predicate, ctx, engine, arena, |_, item, av| {
        if crate::arena::truthy_arena(av, engine) {
            results.push(*item);
        }
        Ok(ControlFlow::Continue(()))
    })?;
    if results.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }
    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// An object source: the `(key, value)` pairs whose predicate is truthy, as
/// a new object.
#[inline]
fn filter_bridge_object<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    predicate: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let mut kept = bvec::<(&'a str, DataValue<'a>)>(arena, pairs.len());
    for_each_iter_object(pairs, predicate, ctx, engine, arena, |_, item, key, av| {
        if crate::arena::truthy_arena(av, engine) {
            kept.push((key, *item));
        }
        Ok(ControlFlow::Continue(()))
    })?;
    if kept.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_object());
    }
    Ok(arena.alloc(DataValue::Object(kept.into_bump_slice())))
}
