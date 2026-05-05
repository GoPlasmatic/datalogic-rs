//! `filter` — keep array items / object pairs whose predicate is truthy.

use crate::arena::{ContextStack, DataValue, bvec};
use crate::opcode::OpCode;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use std::ops::ControlFlow;

use super::helpers::{
    FastPredicate, IterArgKind, IterSrc, ResolvedInput, evaluate_invariant_no_push,
    for_each_iter_array, for_each_iter_object, resolve_iter_input, try_extract_filter_field_cmp,
};

/// `filter`. Fast path: input collection resolves at root scope (the dominant
/// pattern in real workloads). Bridge path handles non-borrowable inputs.
#[inline]
pub(crate) fn evaluate_filter<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(crate::constants::invalid_args());
    }

    // Resolve input via unified helper (root borrow OR upstream arena op).
    let src = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(crate::arena::pool::singleton_empty_array()),
        ResolvedInput::Bridge(av) => {
            return filter_arena_bridge(av, &args[1], ctx, engine, arena);
        }
    };

    let predicate = &args[1];
    let len = src.len();
    if len == 0 {
        return Ok(crate::arena::pool::singleton_empty_array());
    }

    // Fast paths bypass `run_iter_body` and skip tracer markers. Defer to the
    // general path when a tracer is attached.
    if !ctx.is_tracing() {
        if let Some(result) = filter_strict_eq_field_fast_path(&src, predicate, ctx, engine, arena)?
        {
            return Ok(result);
        }

        if let Some(fast_pred) = FastPredicate::from_node(predicate) {
            return Ok(filter_with_fast_predicate(&src, fast_pred, arena));
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
    let CompiledNode::BuiltinOperator {
        opcode,
        args: pred_args,
        ..
    } = predicate
    else {
        return Ok(None);
    };
    if pred_args.len() != 2 || !matches!(opcode, OpCode::StrictEquals | OpCode::StrictNotEquals) {
        return Ok(None);
    }

    let Some((segments, invariant_node)) =
        try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
            .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]))
    else {
        return Ok(None);
    };

    let invariant_val = evaluate_invariant_no_push(invariant_node, ctx, engine, arena)?;
    let is_eq = matches!(opcode, OpCode::StrictEquals);
    let len = src.len();
    let mut results = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        let matches = match crate::arena::value::traverse_segments(item, segments, arena) {
            Some(av) => av == invariant_val,
            None => false,
        };
        if matches == is_eq {
            results.push(*item);
        }
    }
    if results.is_empty() {
        return Ok(Some(crate::arena::pool::singleton_empty_array()));
    }
    Ok(Some(
        arena.alloc(DataValue::Array(results.into_bump_slice())),
    ))
}

/// Filter using a `FastPredicate` — predicate evaluates in-place against each
/// item with zero context push and zero per-item allocation.
#[inline]
fn filter_with_fast_predicate<'a>(
    src: &IterSrc<'a>,
    fast_pred: &FastPredicate,
    arena: &'a Bump,
) -> &'a DataValue<'a> {
    let len = src.len();
    let mut results = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        if fast_pred.evaluate(item, arena) {
            results.push(*item);
        }
    }
    if results.is_empty() {
        return crate::arena::pool::singleton_empty_array();
    }
    arena.alloc(DataValue::Array(results.into_bump_slice()))
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
        return Ok(crate::arena::pool::singleton_empty_array());
    }
    Ok(arena.alloc(DataValue::Array(results.into_bump_slice())))
}

/// Filter Bridge case — input is an Object, an inline arena Array (e.g. a
/// literal `[1,2,3]` arg) or a non-array primitive. Object inputs iterate
/// `(key, value)` pairs into a new arena `Object`; arena Array inputs iterate
/// items into a new arena `Array`; other shapes are an error.
#[inline]
fn filter_arena_bridge<'a>(
    input: &'a DataValue<'a>,
    predicate: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match input {
        DataValue::Object(pairs) => filter_bridge_object(pairs, predicate, ctx, engine, arena),
        DataValue::Array(items) => filter_bridge_array(items, predicate, ctx, engine, arena),
        _ => Err(crate::constants::invalid_args()),
    }
}

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
        return Ok(crate::arena::pool::singleton_empty_object());
    }
    Ok(arena.alloc(DataValue::Object(kept.into_bump_slice())))
}

#[inline]
fn filter_bridge_array<'a>(
    items: &'a [DataValue<'a>],
    predicate: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let mut kept = bvec::<DataValue<'a>>(arena, items.len());
    for_each_iter_array(items, predicate, ctx, engine, arena, |_, item, av| {
        if crate::arena::truthy_arena(av, engine) {
            kept.push(*item);
        }
        Ok(ControlFlow::Continue(()))
    })?;
    if kept.is_empty() {
        return Ok(crate::arena::pool::singleton_empty_array());
    }
    Ok(arena.alloc(DataValue::Array(kept.into_bump_slice())))
}
