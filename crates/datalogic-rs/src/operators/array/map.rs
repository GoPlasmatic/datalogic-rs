//! `map` — transform each item via a body expression.

use crate::arena::{ContextStack, DataValue, bvec};
use crate::node::PathSegment;
use crate::opcode::OpCode;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::{NumberValue, OwnedDataValue};
use std::ops::ControlFlow;

use super::helpers::{
    FieldCursor, FusedMapBody, IterArgKind, IterSrc, ResolvedInput, for_each_iter_array,
    for_each_iter_object, resolve_iter_input,
};

/// `map`. Borrows input from root scope when possible. Body fast path for
/// var/field-extract re-borrows the arena item per output entry with zero
/// iteration allocs. Other body shapes evaluate the body via arena dispatch
/// per item.
#[inline]
pub(crate) fn evaluate_map<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(crate::Error::invalid_args());
    }

    let body = &args[1];
    let src = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(crate::arena::singletons::singleton_empty_array()),
        ResolvedInput::Bridge(av) => {
            return map_arena_bridge(av, body, ctx, engine, arena);
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }

    // Fast paths bypass `run_iter_body`, so they skip the tracer's
    // per-iteration markers. Only enter them when no tracer is attached.
    // Shape detection is shared with the reduce(map(...)) fusion — see
    // `FusedMapBody::detect`.
    if !ctx.is_tracing() {
        if let Some(shape) = FusedMapBody::detect(body) {
            if let Some(result) = map_fused(&src, &shape, arena) {
                return Ok(result);
            }
        }
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
/// Returns `None` if the literal isn't numeric or the data doesn't fit —
/// caller falls through to the general path.
#[inline]
fn map_arith_var_lit<'a>(
    src: &IterSrc<'a>,
    opcode: OpCode,
    var_segs: &[PathSegment],
    lit_value: &OwnedDataValue,
    var_is_lhs: bool,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let lit_f = lit_value.as_f64()?;
    let lit_i = lit_value.as_i64();
    let len = src.len();

    // Integer fast path. Aborts (without committing results) on the first
    // overflow or non-integer input — caller falls through to f64.
    if let Some(li) = lit_i {
        if let Some(av) = map_arith_var_lit_int(src, var_segs, li, opcode, var_is_lhs, len, arena) {
            return Some(av);
        }
    }

    // f64 path.
    let mut results = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        let val = if var_segs.is_empty() {
            item
        } else {
            crate::arena::value::traverse_segments(item, var_segs)?
        };
        let item_f = val.as_f64()?;
        let (a, b) = if var_is_lhs {
            (item_f, lit_f)
        } else {
            (lit_f, item_f)
        };
        let r = match opcode {
            OpCode::Add => a + b,
            OpCode::Subtract => a - b,
            OpCode::Multiply => a * b,
            _ => unreachable!(),
        };
        results.push(DataValue::Number(NumberValue::from_f64(r)));
    }
    Some(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// Integer-only branch of [`map_arith_var_lit`]. Returns `None`
/// (without allocating into the arena) on overflow or non-integer input so
/// the caller's f64 path can take over.
#[inline]
fn map_arith_var_lit_int<'a>(
    src: &IterSrc<'a>,
    var_segs: &[PathSegment],
    li: i64,
    opcode: OpCode,
    var_is_lhs: bool,
    len: usize,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let mut results = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        let val = if var_segs.is_empty() {
            item
        } else {
            crate::arena::value::traverse_segments(item, var_segs)?
        };
        let item_i = val.as_i64()?;
        let (a, b) = if var_is_lhs {
            (item_i, li)
        } else {
            (li, item_i)
        };
        let r = match opcode {
            OpCode::Add => a.checked_add(b)?,
            OpCode::Subtract => a.checked_sub(b)?,
            OpCode::Multiply => a.checked_mul(b)?,
            _ => unreachable!(),
        };
        results.push(DataValue::Number(NumberValue::Integer(r)));
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
    opcode: OpCode,
    a_segs: &[PathSegment],
    b_segs: &[PathSegment],
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let len = src.len();
    let mut a_field = FieldCursor::new(a_segs);
    let mut b_field = FieldCursor::new(b_segs);

    // Integer pass. Aborts (without committing results) on the first
    // overflow or non-integer operand — the f64 pass below takes over, and
    // `NumberValue::from_f64`'s whole-value canonicalization keeps the two
    // passes' outputs identical where they overlap.
    'int_pass: {
        let mut results = bvec::<DataValue<'a>>(arena, len);
        for i in 0..len {
            let item = src.get(i);
            let a = a_field.resolve(item)?;
            let b = b_field.resolve(item)?;
            let (Some(ia), Some(ib)) = (a.as_i64(), b.as_i64()) else {
                break 'int_pass;
            };
            let r = match opcode {
                OpCode::Add => ia.checked_add(ib),
                OpCode::Subtract => ia.checked_sub(ib),
                OpCode::Multiply => ia.checked_mul(ib),
                _ => unreachable!(),
            };
            let Some(r) = r else { break 'int_pass };
            results.push(DataValue::Number(NumberValue::Integer(r)));
        }
        return Some(arena.alloc(DataValue::Array(results.into_bump_slice())));
    }

    // f64 pass — still Numbers only; anything else falls to the general path.
    let mut results = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        let a_f = a_field.resolve(item)?.as_f64()?;
        let b_f = b_field.resolve(item)?.as_f64()?;
        let r = match opcode {
            OpCode::Add => a_f + b_f,
            OpCode::Subtract => a_f - b_f,
            OpCode::Multiply => a_f * b_f,
            _ => unreachable!(),
        };
        results.push(DataValue::Number(NumberValue::from_f64(r)));
    }
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

/// Map Bridge case — Object inputs iterate (key, value) pairs; inline arena
/// Array inputs (e.g. literal `[1,2,3]` arg) iterate items; other shapes are
/// treated as a single-element collection.
#[inline]
fn map_arena_bridge<'a>(
    input: &'a DataValue<'a>,
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    debug_assert!(
        !matches!(input, DataValue::Array(_) | DataValue::Null),
        "Bridge is never Array/Null (see ResolvedInput::Bridge)"
    );
    match input {
        DataValue::Object(pairs) => map_bridge_object(pairs, body, ctx, engine, arena),
        // Single-element collection (number, string, bool primitive input).
        _ => map_bridge_single(input, body, ctx, engine, arena),
    }
}

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

#[inline]
fn map_bridge_single<'a>(
    input: &'a DataValue<'a>,
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let item_av: &'a DataValue<'a> = input;
    ctx.push_with_index(item_av, 0);
    // Pop before propagating errors. A bare `?` on `run_iter_body` would skip
    // the `pop` and leak this frame; when a surrounding `try` catches the
    // error, later evaluation would then resolve `var`/`val` against the
    // stale frame instead of the real context.
    let result = engine.run_iter_body(body, ctx, arena, 0, 1);
    ctx.pop();
    let owned = *result?;
    let slice = arena.alloc_slice_fill_iter(std::iter::once(owned));
    Ok(arena.alloc(DataValue::Array(slice)))
}
