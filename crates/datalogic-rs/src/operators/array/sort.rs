//! `sort` — order an array, optionally with an extractor expression for keys.

use std::cmp::Ordering;

use crate::arena::{ContextStack, DataValue, IterGuard, bvec};
use crate::node::{MetadataHint, ReduceHint};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::{IterArgKind, IterSrc, ResolvedInput, resolve_iter_input};

/// `sort`. Borrows input via `IterSrc` (no input clone), runs
/// `slice::sort_by` over indices, and emits `DataValue::Array` re-borrowing
/// the original arena items in their sorted order — avoids a deep-clone of
/// the input array, which dominates for object arrays.
///
/// Fast path (extractor is a root-scope `var`): keys come from
/// `traverse_segments` returning `&DataValue` directly, no key clones.
///
/// Scratch (index and key vectors) is bump-allocated in the eval arena,
/// which the session resets wholesale after each evaluation, so sorting
/// makes no per-call heap round-trips for its own buffers.
#[inline]
pub(crate) fn evaluate_sort<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }

    // Literal-null first arg is an error.
    if let CompiledNode::Value { value, .. } = &args[0] {
        if value.is_null() {
            return Err(crate::Error::invalid_args());
        }
    }

    let src = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(crate::arena::singletons::singleton_null()),
        ResolvedInput::Bridge(av) => {
            // Bridge is never Array/Null (see ResolvedInput::Bridge), so a sort
            // input reaching here is a scalar or object: not sortable.
            debug_assert!(!matches!(av, DataValue::Array(_) | DataValue::Null));
            return Err(crate::Error::invalid_args());
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }

    let ascending = sort_direction(args, ctx, engine, arena)?;

    // No extractor — sort items directly by DataValue order.
    if args.len() <= 2 {
        return Ok(sort_no_extractor(&src, ascending, arena));
    }

    let extractor = &args[2];

    // Fast path: extractor is a root-scope `var` over non-empty segments —
    // keys come from `traverse_segments` directly.
    if let Some(result) = sort_fast_path_var_extractor(&src, extractor, ascending, arena) {
        return Ok(result);
    }

    // General extractor — push each item, evaluate, collect keys, sort indices.
    sort_general_extractor(&src, extractor, ascending, ctx, engine, arena)
}

/// Read the optional `args[1]` direction flag — defaults to ascending.
#[inline]
fn sort_direction<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<bool> {
    if args.len() > 1 {
        let dir = engine.dispatch_node(&args[1], ctx, arena)?;
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

    // All-numeric fast path: sort scalar `(key, index)` pairs instead of
    // driving `compare_values` through an index indirection per comparison.
    // The index tiebreaker reproduces the stable sort's equal-key order, so
    // the output is identical to the general path's.
    if src.0.iter().all(|v| matches!(v, DataValue::Number(_))) {
        let mut keyed = bvec::<(f64, u32)>(arena, len);
        keyed.extend(src.0.iter().enumerate().map(|(i, v)| {
            let f = match v {
                DataValue::Number(n) => n.as_f64(),
                _ => unreachable!(),
            };
            // Collapse -0.0 to 0.0 so `total_cmp` can't order the two zero
            // representations, keeping zero-keyed ties on the index
            // tiebreaker exactly like the stable path.
            (if f == 0.0 { 0.0 } else { f }, i as u32)
        }));
        keyed.sort_unstable_by(|(ka, ia), (kb, ib)| {
            // `total_cmp` keeps the comparator a total order even for NaN
            // keys (which JSON data can't contain anyway — pdqsort panics
            // on inconsistent comparators, so `partial_cmp` is off the
            // table). Ties break by input position, reproducing the stable
            // sort's equal-key order.
            let cmp = ka.total_cmp(kb);
            let cmp = if ascending { cmp } else { cmp.reverse() };
            cmp.then(ia.cmp(ib))
        });
        let slice = arena.alloc_slice_fill_iter(keyed.iter().map(|&(_, i)| *src.get(i as usize)));
        return arena.alloc(DataValue::Array(slice));
    }

    let mut indices = bvec::<usize>(arena, len);
    indices.extend(0..len);
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(src.get(a), src.get(b));
        if ascending { cmp } else { cmp.reverse() }
    });
    let slice = arena.alloc_slice_fill_iter(indices.iter().map(|&i| *src.get(i)));
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
    let CompiledNode::Var {
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
    let mut keyed = bvec::<(usize, Option<&'a DataValue<'a>>)>(arena, len);
    keyed.extend((0..len).map(|i| {
        (
            i,
            crate::arena::value::traverse_segments(src.get(i), segments),
        )
    }));
    keyed.sort_by(|(_, ka), (_, kb)| {
        let cmp = match (ka, kb) {
            (Some(a), Some(b)) => compare_values(a, b),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        };
        if ascending { cmp } else { cmp.reverse() }
    });
    let slice = arena.alloc_slice_fill_iter(keyed.iter().map(|&(i, _)| *src.get(i)));
    Some(arena.alloc(DataValue::Array(slice)))
}

#[inline]
fn sort_general_extractor<'a>(
    src: &IterSrc<'a>,
    extractor: &'a CompiledNode,
    ascending: bool,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let len = src.len();
    let mut keys = bvec::<DataValue<'a>>(arena, len);
    let mut guard = IterGuard::new(ctx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_indexed(item, i);
        let key_av = engine.dispatch_node(extractor, guard.stack(), arena)?;
        keys.push(*key_av);
    }
    drop(guard);

    let mut indices = bvec::<usize>(arena, len);
    indices.extend(0..len);
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(&keys[a], &keys[b]);
        if ascending { cmp } else { cmp.reverse() }
    });
    let slice = arena.alloc_slice_fill_iter(indices.iter().map(|&i| *src.get(i)));
    Ok(arena.alloc(DataValue::Array(slice)))
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
