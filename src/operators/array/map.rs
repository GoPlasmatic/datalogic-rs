//! `map` — transform each item via a body expression.

use crate::arena::{DataContextStack, DataValue, IterGuard, bvec};
use crate::node::{MetadataHint, PathSegment, ReduceHint};
use crate::opcode::OpCode;
use crate::value::NumberValue;
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::{IterArgKind, IterSrc, ResolvedInput, resolve_iter_input};

/// `map`. Borrows input from root scope when possible. Body fast path for
/// var/field-extract re-borrows the arena item per output entry with zero
/// iteration allocs. Other body shapes evaluate the body via arena dispatch
/// per item.
#[inline]
pub(crate) fn evaluate_map_arena<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(crate::constants::invalid_args());
    }

    let body = &args[1];
    let src = match resolve_iter_input(&args[0], iter_arg_kind, actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(crate::arena::pool::singleton_empty_array()),
        ResolvedInput::Bridge(av) => {
            return map_arena_bridge(av, body, actx, engine, arena);
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(crate::arena::pool::singleton_empty_array());
    }

    // Fast paths bypass `eval_iter_body`, so they skip the tracer's
    // per-iteration markers. Only enter them when no tracer is attached.
    if !actx.is_tracing() {
        if let Some(result) = map_var_fast_path(&src, body, arena) {
            return Ok(result);
        }

        if let Some(result) = map_arith_var_lit_fast_path(&src, body, arena) {
            return Ok(result);
        }
    }

    map_general(&src, body, actx, engine, arena)
}

/// Detect a `{op: [{val:[…]}, literal]}` (or literal-first) body and fold
/// the iteration into a tight loop with no per-item context push or
/// dispatcher recursion. Covers the dominant `{*: [{val:[]}, 2]}` style of
/// arithmetic-with-literal map bodies seen in real workloads.
///
/// Returns `None` if the body shape doesn't match — caller falls through to
/// the general path. On match, returns the fully-built result array.
#[inline]
fn map_arith_var_lit_fast_path<'a>(
    src: &IterSrc<'a>,
    body: &'a CompiledNode,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let CompiledNode::BuiltinOperator { opcode, args, .. } = body else {
        return None;
    };
    if args.len() != 2 {
        return None;
    }
    let opcode = *opcode;
    if !matches!(opcode, OpCode::Add | OpCode::Subtract | OpCode::Multiply) {
        return None;
    }

    // Detect (var(item), literal) or (literal, var(item)).
    let (var_segs, lit_value, var_is_lhs) = match (&args[0], &args[1]) {
        (
            CompiledNode::CompiledVar {
                scope_level: 0,
                segments,
                reduce_hint: ReduceHint::None,
                metadata_hint: MetadataHint::None,
                default_value: None,
                ..
            },
            CompiledNode::Value { value, .. },
        ) => (segments.as_ref(), value, true),
        (
            CompiledNode::Value { value, .. },
            CompiledNode::CompiledVar {
                scope_level: 0,
                segments,
                reduce_hint: ReduceHint::None,
                metadata_hint: MetadataHint::None,
                default_value: None,
                ..
            },
        ) => (segments.as_ref(), value, false),
        _ => return None,
    };

    let lit_f = lit_value.as_f64()?;
    let lit_i = lit_value.as_i64();
    let len = src.len();

    // Integer fast path. Aborts (without committing results) on the first
    // overflow or non-integer input — caller falls through to f64.
    if let Some(li) = lit_i
        && let Some(av) = map_arith_var_lit_int(src, var_segs, li, opcode, var_is_lhs, len, arena)
    {
        return Some(av);
    }

    // f64 path.
    let mut results = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        let val = if var_segs.is_empty() {
            item
        } else {
            crate::arena::value::arena_traverse_segments(item, var_segs, arena)?
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

/// Integer-only branch of [`map_arith_var_lit_fast_path`]. Returns `None`
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
            crate::arena::value::arena_traverse_segments(item, var_segs, arena)?
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

/// Body fast path: `var` body with simple shape — identity (empty segments)
/// or field extract. Both re-borrow arena items with zero per-iteration allocs.
#[inline]
fn map_var_fast_path<'a>(
    src: &IterSrc<'a>,
    body: &'a CompiledNode,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = body
    else {
        return None;
    };

    let len = src.len();
    let mut results = bvec::<DataValue<'a>>(arena, len);
    if segments.is_empty() {
        for i in 0..len {
            results.push(crate::arena::value::reborrow_arena_value(src.get(i)));
        }
    } else {
        for i in 0..len {
            let item = src.get(i);
            match crate::arena::value::arena_traverse_segments(item, segments, arena) {
                Some(v) => results.push(crate::arena::value::reborrow_arena_value(v)),
                None => results.push(DataValue::Null),
            }
        }
    }
    Some(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// General path — dispatches body via the arena context stack per item.
#[inline]
fn map_general<'a>(
    src: &IterSrc<'a>,
    body: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let len = src.len();
    let total = len as u32;
    let mut results = bvec::<DataValue<'a>>(arena, len);
    let mut guard = IterGuard::new(actx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_indexed(item, i);
        let av = engine.eval_iter_body(body, guard.stack(), arena, i as u32, total)?;
        results.push(crate::arena::value::reborrow_arena_value(av));
    }
    drop(guard);
    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// Map Bridge case — Object inputs iterate (key, value) pairs; inline arena
/// Array inputs (e.g. literal `[1,2,3]` arg) iterate items; other shapes are
/// treated as a single-element collection.
#[inline]
fn map_arena_bridge<'a>(
    input: &'a DataValue<'a>,
    body: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match input {
        DataValue::Object(pairs) => map_bridge_object(pairs, body, actx, engine, arena),
        DataValue::Array(items) => map_bridge_array(items, body, actx, engine, arena),
        // Single-element collection (number, string, bool primitive input).
        _ => map_bridge_single(input, body, actx, engine, arena),
    }
}

#[inline]
fn map_bridge_object<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    body: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let total = pairs.len() as u32;
    let mut results = bvec::<DataValue<'a>>(arena, pairs.len());
    let mut guard = IterGuard::new(actx);
    for (i, (k, v)) in pairs.iter().enumerate() {
        // SAFETY: pairs[i].1 lives in the arena for `'a`; reborrow as `&'a` is sound.
        let item_av: &'a DataValue<'a> = unsafe { &*(v as *const DataValue<'a>) };
        let key_arena: &'a str = k;
        guard.step_keyed(item_av, i, key_arena);
        let av = engine.eval_iter_body(body, guard.stack(), arena, i as u32, total)?;
        results.push(crate::arena::value::reborrow_arena_value(av));
    }
    drop(guard);
    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

#[inline]
fn map_bridge_array<'a>(
    items: &'a [DataValue<'a>],
    body: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let total = items.len() as u32;
    let mut results = bvec::<DataValue<'a>>(arena, items.len());
    let mut guard = IterGuard::new(actx);
    for (i, item_av) in items.iter().enumerate() {
        guard.step_indexed(item_av, i);
        let av = engine.eval_iter_body(body, guard.stack(), arena, i as u32, total)?;
        results.push(crate::arena::value::reborrow_arena_value(av));
    }
    drop(guard);
    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

#[inline]
fn map_bridge_single<'a>(
    input: &'a DataValue<'a>,
    body: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let item_av: &'a DataValue<'a> = input;
    actx.push_with_index(item_av, 0);
    let av = engine.eval_iter_body(body, actx, arena, 0, 1)?;
    let owned = crate::arena::value::reborrow_arena_value(av);
    actx.pop();
    let slice = arena.alloc_slice_fill_iter(std::iter::once(owned));
    Ok(arena.alloc(DataValue::Array(slice)))
}
