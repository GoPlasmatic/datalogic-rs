//! `sort` — order an array, optionally with an extractor expression for keys.

use std::cmp::Ordering;

use crate::arena::{DataContextStack, DataValue, IterGuard};
use crate::node::{MetadataHint, ReduceHint};
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::{IterArgKind, IterSrc, ResolvedInput, resolve_iter_input};

/// `sort`. Borrows input via `IterSrc` (no input clone), runs
/// `slice::sort_by` over indices, and emits `DataValue::Array` re-borrowing
/// the original arena items in their sorted order — avoids a deep-clone of
/// the input array, which dominates for object arrays.
///
/// Fast path (extractor is a root-scope `var`): keys come from
/// `arena_traverse_segments` returning `&DataValue` directly, no key clones.
#[inline]
pub(crate) fn evaluate_sort_arena<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }

    // Literal-null first arg is an error.
    if let CompiledNode::Value { value, .. } = &args[0]
        && value.is_null()
    {
        return Err(crate::constants::invalid_args());
    }

    let src = match resolve_iter_input(&args[0], iter_arg_kind, actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(DataValue::Null)),
        ResolvedInput::Bridge(av) => {
            return sort_arena_from_value(av, args, actx, engine, arena);
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(arena.alloc(DataValue::Array(&[])));
    }

    let ascending = sort_direction(args, actx, engine, arena)?;

    // No extractor — sort items directly by DataValue order.
    if args.len() <= 2 {
        return Ok(sort_no_extractor(&src, ascending, arena));
    }

    let extractor = &args[2];

    // Fast path: extractor is a root-scope `var` over non-empty segments —
    // keys come from `arena_traverse_segments` directly.
    if let Some(result) = sort_fast_path_var_extractor(&src, extractor, ascending, arena) {
        return Ok(result);
    }

    // General extractor — push each item, evaluate, collect keys, sort indices.
    sort_general_extractor(&src, extractor, ascending, actx, engine, arena)
}

/// Read the optional `args[1]` direction flag — defaults to ascending.
#[inline]
fn sort_direction<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<bool> {
    if args.len() > 1 {
        let dir = engine.evaluate_node(&args[1], actx, arena)?;
        Ok(match dir {
            DataValue::Bool(b) => *b,
            _ => true,
        })
    } else {
        Ok(true)
    }
}

#[inline]
fn sort_no_extractor<'a>(src: &IterSrc<'a>, ascending: bool, arena: &'a Bump) -> &'a DataValue<'a> {
    let len = src.len();
    let mut indices: Vec<usize> = (0..len).collect();
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(src.get(a), src.get(b));
        if ascending { cmp } else { cmp.reverse() }
    });
    let slice = arena.alloc_slice_fill_iter(
        indices
            .into_iter()
            .map(|i| crate::arena::value::reborrow_arena_value(src.get(i))),
    );
    arena.alloc(DataValue::Array(slice))
}

/// Extractor fast path: `{var: "field..."}` over non-empty segments at scope 0.
#[inline]
fn sort_fast_path_var_extractor<'a>(
    src: &IterSrc<'a>,
    extractor: &'a CompiledNode,
    ascending: bool,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = extractor
    else {
        return None;
    };
    if segments.is_empty() {
        return None;
    }

    let len = src.len();
    let mut keyed: Vec<(usize, Option<&DataValue<'a>>)> = (0..len)
        .map(|i| {
            (
                i,
                crate::arena::value::arena_traverse_segments(src.get(i), segments, arena),
            )
        })
        .collect();
    keyed.sort_by(|(_, ka), (_, kb)| {
        let cmp = match (ka, kb) {
            (Some(a), Some(b)) => compare_values(a, b),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        };
        if ascending { cmp } else { cmp.reverse() }
    });
    let slice = arena.alloc_slice_fill_iter(
        keyed
            .into_iter()
            .map(|(i, _)| crate::arena::value::reborrow_arena_value(src.get(i))),
    );
    Some(arena.alloc(DataValue::Array(slice)))
}

#[inline]
fn sort_general_extractor<'a>(
    src: &IterSrc<'a>,
    extractor: &'a CompiledNode,
    ascending: bool,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let len = src.len();
    let mut keys: Vec<DataValue<'a>> = Vec::with_capacity(len);
    let mut guard = IterGuard::new(actx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_indexed(item, i);
        let key_av = engine.evaluate_node(extractor, guard.stack(), arena)?;
        keys.push(crate::arena::value::reborrow_arena_value(key_av));
    }
    drop(guard);

    let mut indices: Vec<usize> = (0..len).collect();
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(&keys[a], &keys[b]);
        if ascending { cmp } else { cmp.reverse() }
    });
    let slice = arena.alloc_slice_fill_iter(
        indices
            .into_iter()
            .map(|i| crate::arena::value::reborrow_arena_value(src.get(i))),
    );
    Ok(arena.alloc(DataValue::Array(slice)))
}

/// Sort a resolved arena value when the input wasn't borrowable as a
/// flat `&[Value]` — falls into one of: Null (→ Null), Array (→ sort),
/// anything else (→ error). Re-uses the same direction/extractor logic
/// as the borrowed path.
#[inline]
fn sort_arena_from_value<'a>(
    av: &'a DataValue<'a>,
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let arena_items_slice: &'a [DataValue<'a>] = match av {
        DataValue::Null => {
            return Ok(crate::arena::pool::singleton_null());
        }
        DataValue::Array(items) => items,
        _ => return Err(crate::constants::invalid_args()),
    };
    if arena_items_slice.is_empty() {
        return Ok(arena.alloc(DataValue::Array(&[])));
    }

    let ascending = sort_direction(args, actx, engine, arena)?;
    let n = arena_items_slice.len();

    if args.len() <= 2 {
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by(|&a, &b| {
            let cmp = compare_values(&arena_items_slice[a], &arena_items_slice[b]);
            if ascending { cmp } else { cmp.reverse() }
        });
        let items = arena.alloc_slice_fill_iter(
            indices
                .into_iter()
                .map(|i| crate::arena::value::reborrow_arena_value(&arena_items_slice[i])),
        );
        return Ok(arena.alloc(DataValue::Array(items)));
    }

    // Extractor present — push items into arena context, evaluate,
    // collect keys, sort indices.
    let extractor = &args[2];
    let mut keys: Vec<DataValue<'a>> = Vec::with_capacity(n);
    let mut guard = IterGuard::new(actx);
    for (i, item_av) in arena_items_slice.iter().enumerate() {
        guard.step_indexed(item_av, i);
        let key_av = engine.evaluate_node(extractor, guard.stack(), arena)?;
        keys.push(crate::arena::value::reborrow_arena_value(key_av));
    }
    drop(guard);

    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(&keys[a], &keys[b]);
        if ascending { cmp } else { cmp.reverse() }
    });

    let out = arena.alloc_slice_fill_iter(
        indices
            .into_iter()
            .map(|i| crate::arena::value::reborrow_arena_value(&arena_items_slice[i])),
    );
    Ok(arena.alloc(DataValue::Array(out)))
}

/// Compare arena values for sorting.
/// Type order: null < bool < number < string < array < object.
#[inline]
fn compare_values(a: &DataValue<'_>, b: &DataValue<'_>) -> Ordering {
    #[inline]
    fn type_rank(v: &DataValue<'_>) -> u8 {
        match v {
            DataValue::Null => 0,
            DataValue::Bool(_) => 1,
            DataValue::Number(_) => 2,
            DataValue::String(_) => 3,
            DataValue::Array(_) => 4,
            DataValue::Object(_) => 5,
            #[cfg(feature = "datetime")]
            DataValue::DateTime(_) | DataValue::Duration(_) => 3,
        }
    }

    match (a, b) {
        (DataValue::Null, DataValue::Null) => Ordering::Equal,
        (DataValue::Bool(a), DataValue::Bool(b)) => a.cmp(b),
        (DataValue::Number(a), DataValue::Number(b)) => {
            let a_f = a.as_f64();
            let b_f = b.as_f64();
            if a_f < b_f {
                Ordering::Less
            } else if a_f > b_f {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }
        (DataValue::String(a), DataValue::String(b)) => a.cmp(b),
        (DataValue::Array(_), DataValue::Array(_)) => Ordering::Equal,
        (DataValue::Object(_), DataValue::Object(_)) => Ordering::Equal,
        _ => type_rank(a).cmp(&type_rank(b)),
    }
}
