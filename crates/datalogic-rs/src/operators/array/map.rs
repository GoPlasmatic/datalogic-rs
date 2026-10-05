//! `map` — transform each item via a body expression.

use crate::arena::{ContextStack, DataValue, IterGuard, bvec};
use crate::node::PathSegment;
use crate::operators::meta::ArithOp;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::{NumberValue, OwnedDataValue};
use std::ops::ControlFlow;

use super::helpers::{
    FieldCursor, FusedMapBody, Items, IterSrc, for_each_iter_array, for_each_iter_object,
};
use super::helpers::{combine_ints, with_arith, with_ops};

/// `map`: the body's value for each item (or object pair). A scalar source
/// is mapped as a one-item collection. Body fast path for var/field-extract
/// re-borrows the arena item per output entry with zero iteration allocs;
/// other body shapes evaluate the body via arena dispatch per item.
#[inline]
pub(crate) fn evaluate_map<'a>(
    items: Items<'a>,
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let body = &args[1];
    let src = match items {
        Items::Array(src) => src,
        Items::Object(pairs) => return map_bridge_object(pairs, body, ctx, engine, arena),
        Items::Scalar(value) => return map_bridge_single(value, body, ctx, engine, arena),
    };

    // Fast paths bypass `run_iter_body`, so they skip the tracer's
    // per-iteration markers. Only enter them when no tracer is attached.
    // Shape detection is shared with the reduce(map(...)) fusion — see
    // `FusedMapBody::detect`.
    if !ctx.is_tracing()
        && let Some(shape) = FusedMapBody::detect(body)
        && let Some(result) = map_fused(&src, &shape, arena)
    {
        return Ok(result);
    }

    map_general(&src, body, ctx, engine, arena)
}

/// Execute a classified fusible body shape as a tight materializing loop.
/// Returns `None` when the data doesn't fit the shape (non-numeric operands,
/// non-coercible literal) — caller falls through to the general path.
#[inline]
fn map_fused<'a>(
    src: &IterSrc<'a>,
    shape: &FusedMapBody<'_>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    match shape {
        FusedMapBody::Extract { segments } => Some(map_extract(src, segments, arena)),
        FusedMapBody::ArithVarLit {
            op,
            segments,
            lit,
            var_is_lhs,
        } => map_arith_var_lit(src, *op, segments, lit, *var_is_lhs, arena),
        FusedMapBody::ArithVarVar {
            op,
            a_segments,
            b_segments,
        } => map_arith_var_var(src, *op, a_segments, b_segments, arena),
    }
}

/// Execute a `{op: [var, literal]}` (or literal-first) body as a tight loop
/// with no per-item context push or dispatcher recursion. Covers the
/// dominant `{*: [{val:[]}, 2]}` style of arithmetic-with-literal map
/// bodies seen in real workloads.
///
/// Each element goes through [`combine`](super::helpers::combine) with the operation and operand
/// order fixed outside the loop, so results match the arithmetic
/// operators exactly. Returns `None` if the literal or any value is not a
/// number, or a field is missing: the caller falls through to the general
/// path, which owns coercion.
#[inline]
fn map_arith_var_lit<'a>(
    src: &IterSrc<'a>,
    op: ArithOp,
    var_segs: &[PathSegment],
    lit_value: &OwnedDataValue,
    var_is_lhs: bool,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let lit = *lit_value.as_number()?;
    let lit_f = lit.as_f64();
    // The literal is classified once: an integral one leaves each element a
    // single `as_i64` check from exact integer math (the float operation
    // only runs for a non-integer element or an overflow); a fractional one
    // makes every element float, exactly as `combine` would decide.
    with_ops!(op, |int_op, float_op| match (lit.as_i64(), var_is_lhs) {
        (Some(li), true) => map_numbers(src, var_segs, arena, |v| match v.as_i64() {
            Some(x) => combine_ints(x, li, int_op, float_op),
            None => NumberValue::from_f64(float_op(v.as_f64(), lit_f)),
        }),
        (Some(li), false) => map_numbers(src, var_segs, arena, |v| match v.as_i64() {
            Some(x) => combine_ints(li, x, int_op, float_op),
            None => NumberValue::from_f64(float_op(lit_f, v.as_f64())),
        }),
        (None, true) => map_numbers(src, var_segs, arena, |v| {
            NumberValue::from_f64(float_op(v.as_f64(), lit_f))
        }),
        (None, false) => map_numbers(src, var_segs, arena, |v| {
            NumberValue::from_f64(float_op(lit_f, v.as_f64()))
        }),
    })
}

/// The array of `step(value)` over each item's numeric value; `None`
/// (abandoning the fast path) on a missing field or a non-number. The
/// cursor and the result buffer are locals of this loop, not borrowed from
/// the caller, so they stay in registers across the pushes.
#[inline(always)]
fn map_numbers<'a>(
    src: &IterSrc<'a>,
    segments: &[PathSegment],
    arena: &'a Bump,
    step: impl Fn(NumberValue) -> NumberValue,
) -> Option<&'a DataValue<'a>> {
    let mut field = FieldCursor::new(segments);
    let mut results = bvec::<DataValue<'a>>(arena, src.len());
    for item in src.0 {
        let v = *field.resolve(item)?.as_number()?;
        results.push(DataValue::Number(step(v)));
    }
    Some(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// Execute a `{op: [{var: a}, {var: b}]}` body — both plain scope-0 vars —
/// as a tight two-field-extract loop, the var⊗var sibling of
/// [`map_arith_var_lit`]. Covers the pervasive line-total shape
/// `{"*": [{var: "unit_price"}, {var: "qty"}]}`.
///
/// Only `Number` operands are handled; any missing field or non-numeric
/// value abandons the fast path (dropping the partial results in the
/// arena) so the general path re-runs with full coercion semantics.
#[inline]
fn map_arith_var_var<'a>(
    src: &IterSrc<'a>,
    op: ArithOp,
    a_segs: &[PathSegment],
    b_segs: &[PathSegment],
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let mut a_field = FieldCursor::new(a_segs);
    let mut b_field = FieldCursor::new(b_segs);
    let mut results = bvec::<DataValue<'a>>(arena, src.len());
    with_arith!(op, |f| {
        for item in src.0 {
            let a = *a_field.resolve(item)?.as_number()?;
            let b = *b_field.resolve(item)?.as_number()?;
            results.push(DataValue::Number(f(a, b)));
        }
    });
    Some(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// Execute a plain `var` body — identity (empty segments) or field
/// extract. Both re-borrow arena items with zero per-iteration allocs.
/// Missing paths extract as `Null`, matching the general path.
#[inline]
fn map_extract<'a>(
    src: &IterSrc<'a>,
    segments: &[PathSegment],
    arena: &'a Bump,
) -> &'a DataValue<'a> {
    let len = src.len();
    let mut results = bvec::<DataValue<'a>>(arena, len);
    if segments.is_empty() {
        for i in 0..len {
            results.push(*src.get(i));
        }
    } else {
        for i in 0..len {
            let item = src.get(i);
            match crate::arena::value::traverse_segments(item, segments) {
                Some(v) => results.push(*v),
                None => results.push(DataValue::Null),
            }
        }
    }
    arena.alloc(DataValue::Array(results.into_bump_slice()))
}

/// General path — dispatches body via the arena context stack per item.
#[inline]
fn map_general<'a>(
    src: &IterSrc<'a>,
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let mut results = bvec::<DataValue<'a>>(arena, src.len());
    for_each_iter_array(src.0, body, ctx, engine, arena, |_, _item, av| {
        results.push(*av);
        Ok(ControlFlow::Continue(()))
    })?;
    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// An object source: the body's value for each `(key, value)` pair.
#[inline]
fn map_bridge_object<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let mut results = bvec::<DataValue<'a>>(arena, pairs.len());
    for_each_iter_object(pairs, body, ctx, engine, arena, |_, _item, _key, av| {
        results.push(*av);
        Ok(ControlFlow::Continue(()))
    })?;
    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// A scalar source, mapped as a one-item collection.
#[inline]
fn map_bridge_single<'a>(
    input: &'a DataValue<'a>,
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // The guard restores the enclosing frame on drop, `?` included. A bare
    // push that skipped its pop on the error path once leaked this frame
    // into a surrounding `try`'s catch arm (`tests/error_context_test.rs`).
    let mut guard = IterGuard::new(ctx);
    guard.step_indexed(input, 0);
    let owned = *engine.run_iter_body(body, guard.stack(), arena, 0, 1)?;
    let slice = arena.alloc_slice_fill_iter(std::iter::once(owned));
    Ok(arena.alloc(DataValue::Array(slice)))
}
