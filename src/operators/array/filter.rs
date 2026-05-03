//! `filter` — keep array items / object pairs whose predicate is truthy.

use crate::arena::{DataContextStack, DataValue, IterGuard, bvec};
use crate::opcode::OpCode;
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::{
    FastPredicate, IterArgKind, IterSrc, ResolvedInput, arena_value_equals_arena,
    evaluate_invariant_no_push, resolve_iter_input, try_extract_filter_field_cmp,
};

/// `filter`. Fast path: input collection resolves at root scope (the dominant
/// pattern in real workloads). Bridge path handles non-borrowable inputs.
#[inline]
pub(crate) fn evaluate_filter_arena<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(crate::constants::invalid_args());
    }

    // Resolve input via unified helper (root borrow OR upstream arena op).
    let src = match resolve_iter_input(&args[0], iter_arg_kind, actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(crate::arena::pool::singleton_empty_array()),
        ResolvedInput::Bridge(av) => {
            return filter_arena_bridge(av, &args[1], actx, engine, arena);
        }
    };

    let predicate = &args[1];
    let len = src.len();
    if len == 0 {
        return Ok(crate::arena::pool::singleton_empty_array());
    }

    if let Some(result) = filter_strict_eq_field_fast_path(&src, predicate, actx, engine, arena)? {
        return Ok(result);
    }

    if let Some(fast_pred) = FastPredicate::from_node(predicate) {
        return Ok(filter_with_fast_predicate(&src, fast_pred, arena));
    }

    filter_general(&src, predicate, actx, engine, arena)
}

/// Fast path for `filter(arr, == [{var: "field"}, invariant])` — direct field
/// traversal + invariant comparison, no context push, no item clone.
#[inline]
fn filter_strict_eq_field_fast_path<'a>(
    src: &IterSrc<'a>,
    predicate: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
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

    let invariant_val = evaluate_invariant_no_push(invariant_node, actx, engine, arena)?;
    let is_eq = matches!(opcode, OpCode::StrictEquals);
    let len = src.len();
    let mut results = bvec::<DataValue<'a>>(arena, len);
    for i in 0..len {
        let item = src.get(i);
        let matches = match crate::arena::value::arena_traverse_segments(item, segments, arena) {
            Some(av) => arena_value_equals_arena(av, invariant_val),
            None => false,
        };
        if matches == is_eq {
            results.push(crate::arena::value::reborrow_arena_value(item));
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
            results.push(crate::arena::value::reborrow_arena_value(item));
        }
    }
    if results.is_empty() {
        return crate::arena::pool::singleton_empty_array();
    }
    arena.alloc(DataValue::Array(results.into_bump_slice()))
}

/// General filter path — dispatches the predicate per item via the arena
/// context stack. `IterGuard` handles the push-on-first / replace-on-rest /
/// pop-on-drop bookkeeping.
#[inline]
fn filter_general<'a>(
    src: &IterSrc<'a>,
    predicate: &'a CompiledNode,
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
        let keep = engine.eval_iter_body(predicate, guard.stack(), arena, i as u32, total)?;
        if crate::arena::is_truthy_arena(keep, engine) {
            results.push(crate::arena::value::reborrow_arena_value(item));
        }
    }
    drop(guard);
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
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match input {
        DataValue::Object(pairs) => filter_bridge_object(pairs, predicate, actx, engine, arena),
        DataValue::Array(items) => filter_bridge_array(items, predicate, actx, engine, arena),
        _ => Err(crate::constants::invalid_args()),
    }
}

#[inline]
fn filter_bridge_object<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    predicate: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let total = pairs.len() as u32;
    let mut kept = bvec::<(&'a str, DataValue<'a>)>(arena, pairs.len());
    let mut guard = IterGuard::new(actx);
    for (i, (k, v)) in pairs.iter().enumerate() {
        // SAFETY: pairs[i].1 lives in the arena for `'a`; the slice borrow is
        // a sub-borrow of that arena, and reborrowing it as `&'a` is sound.
        let item_av: &'a DataValue<'a> = unsafe { &*(v as *const DataValue<'a>) };
        let key_arena: &'a str = k;
        guard.step_keyed(item_av, i, key_arena);
        let keep = engine.eval_iter_body(predicate, guard.stack(), arena, i as u32, total)?;
        if crate::arena::is_truthy_arena(keep, engine) {
            kept.push((
                key_arena,
                crate::arena::value::reborrow_arena_value(item_av),
            ));
        }
    }
    drop(guard);
    if kept.is_empty() {
        return Ok(crate::arena::pool::singleton_empty_object());
    }
    Ok(arena.alloc(DataValue::Object(kept.into_bump_slice())))
}

#[inline]
fn filter_bridge_array<'a>(
    items: &'a [DataValue<'a>],
    predicate: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let total = items.len() as u32;
    let mut kept = bvec::<DataValue<'a>>(arena, items.len());
    let mut guard = IterGuard::new(actx);
    for (i, item_av) in items.iter().enumerate() {
        guard.step_indexed(item_av, i);
        let keep = engine.eval_iter_body(predicate, guard.stack(), arena, i as u32, total)?;
        if crate::arena::is_truthy_arena(keep, engine) {
            kept.push(crate::arena::value::reborrow_arena_value(item_av));
        }
    }
    drop(guard);
    if kept.is_empty() {
        return Ok(crate::arena::pool::singleton_empty_array());
    }
    Ok(arena.alloc(DataValue::Array(kept.into_bump_slice())))
}
