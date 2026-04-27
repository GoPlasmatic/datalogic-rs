//! `min` and `max` — reductions over an array or variadic args. These are
//! "pipeline tops" that consume an array (typically produced by an upstream
//! filter/map). Arena wins:
//!   1. Input borrow: when args[0] is a root var, no clone of the input array.
//!   2. Composition: when args[0] is filter/map/all/some/none, the arena
//!      intermediate slice is consumed directly.
//!
//! Each op handles the SINGLE-ARG ARRAY form (e.g. `max(items)` over an array).
//! The multi-arg form (`max(a, b, c)`) is handled separately — it doesn't
//! involve array iteration.


use crate::arena::{DataContextStack, DataValue};
use crate::operators::array::{ResolvedInput, resolve_iter_input};
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

/// Generic helper for max/min over an arena-iterable input. `pick_better`
/// returns true when `candidate_f` should replace `best_f` (strictly better).
#[inline]
fn arena_min_max<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    init: f64,
    pick_better: fn(f64, f64) -> bool,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }

    // Multi-arg variadic form: evaluate each arg, pick the best Number.
    if args.len() > 1 {
        return arena_min_max_variadic(args, actx, engine, arena, init, pick_better);
    }

    // Reject literal-array arg shape.
    if matches!(&args[0], CompiledNode::Array { .. }) {
        return Err(crate::constants::invalid_args());
    }
    if let CompiledNode::Value { value, .. } = &args[0]
        && matches!(value, datavalue::OwnedDataValue::Array(_))
    {
        return Err(crate::constants::invalid_args());
    }

    let src = match resolve_iter_input(&args[0], actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Err(crate::constants::invalid_args()),
        ResolvedInput::Bridge(av) => {
            // Array-shaped bridges iterate natively.
            if matches!(av, DataValue::Array(_)) {
                return arena_min_max_from_av(av, init, pick_better, arena);
            }
            // Single non-array arg: must be a `Number`; returned unchanged.
            if !matches!(av, DataValue::Number(_)) {
                return Err(crate::constants::invalid_args());
            }
            return Ok(av);
        }
    };

    if src.is_empty() {
        return Err(crate::constants::invalid_args());
    }

    let mut best_f = init;
    let mut best_idx: Option<usize> = None;
    let len = src.len();
    for i in 0..len {
        match src.get(i) {
            DataValue::Number(n) => {
                let f = n.as_f64();
                if pick_better(f, best_f) {
                    best_f = f;
                    best_idx = Some(i);
                }
            }
            _ => return Err(crate::constants::invalid_args()),
        }
    }

    match best_idx {
        // Re-borrow the arena value to preserve the original Number variant
        // (integer typing).
        Some(i) => Ok(src.get(i)),
        None => Ok(arena.alloc(DataValue::Null)),
    }
}

#[inline]
fn arena_min_max_variadic<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    init: f64,
    pick_better: fn(f64, f64) -> bool,
) -> Result<&'a DataValue<'a>> {
    let mut best_f = init;
    let mut best_av: Option<&'a DataValue<'a>> = None;
    for arg in args {
        let av = engine.evaluate_node(arg, actx, arena)?;
        let f = match av {
            DataValue::Number(n) => n.as_f64(),
            _ => return Err(crate::constants::invalid_args()),
        };
        if pick_better(f, best_f) {
            best_f = f;
            best_av = Some(av);
        }
    }
    match best_av {
        Some(av) => Ok(av),
        None => Ok(crate::arena::pool::singleton_null()),
    }
}

/// Iterate an `&'a DataValue<'a>` (Array variant) for min/max. Used when
/// the input came from a composed arena op (e.g. `merge`).
#[inline]
fn arena_min_max_from_av<'a>(
    av: &'a DataValue<'a>,
    init: f64,
    pick_better: fn(f64, f64) -> bool,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let items: &[DataValue<'a>] = match av {
        DataValue::Array(items) => items,
        _ => return Err(crate::constants::invalid_args()),
    };
    if items.is_empty() {
        return Err(crate::constants::invalid_args());
    }
    let mut best_f = init;
    let mut best_idx: Option<usize> = None;
    for (i, it) in items.iter().enumerate() {
        let f = it.as_f64().ok_or_else(crate::constants::invalid_args)?;
        if pick_better(f, best_f) {
            best_f = f;
            best_idx = Some(i);
        }
    }
    match best_idx {
        Some(i) => Ok(arena.alloc(crate::arena::value::reborrow_arena_value(&items[i]))),
        None => Ok(arena.alloc(DataValue::Null)),
    }
}

/// Arena-mode max(single_array_arg).
#[inline]
pub(crate) fn evaluate_max_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    arena_min_max(args, actx, engine, arena, f64::NEG_INFINITY, |c, b| c > b)
}

/// Arena-mode min(single_array_arg).
#[inline]
pub(crate) fn evaluate_min_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    arena_min_max(args, actx, engine, arena, f64::INFINITY, |c, b| c < b)
}
