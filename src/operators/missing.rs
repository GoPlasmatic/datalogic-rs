use serde_json::Value;

use crate::value_helpers::access_path_ref;
use crate::{CompiledNode, DataLogic, Result};

// =============================================================================
// Arena-mode missing / missing_some
//
// Targets the 12.2% of compatible.json CPU spent on these ops in Phase 5
// profiling. The win comes from accumulating result paths in a bumpalo Vec
// (no Vec growth allocs, no drop cost) and storing path strings as
// arena-allocated &str (no String::clone during accumulation; the per-string
// allocations are deferred to the boundary conversion).
// =============================================================================

use crate::arena::{ArenaContextStack, ArenaValue, arena_to_value};
use bumpalo::Bump;

/// Snapshot of the lookup-target for `missing` / `missing_some` — the
/// current context's data view. Returns a borrow when possible
/// (root or InputRef-wrapped iter frame); materializes only when the
/// frame data lives in the arena and the borrow lifetime would conflict
/// with subsequent mutable borrows.
///
/// Callers use `lookup.as_ref()` to get `&Value` for `access_path_ref`.
enum LookupSnap<'a> {
    Borrowed(&'a Value),
    Owned(Value),
}

impl LookupSnap<'_> {
    #[inline]
    fn as_ref(&self) -> &Value {
        match self {
            LookupSnap::Borrowed(v) => v,
            LookupSnap::Owned(v) => v,
        }
    }
}

#[inline]
fn lookup_snapshot<'a>(actx: &ArenaContextStack<'a>) -> LookupSnap<'a> {
    if actx.depth() > 0 {
        use crate::arena::context::ArenaContextRef;
        if let ArenaContextRef::Frame(f) = actx.current() {
            return match f.data() {
                ArenaValue::InputRef(v) => LookupSnap::Borrowed(v),
                other => LookupSnap::Owned(arena_to_value(other)),
            };
        }
    }
    LookupSnap::Borrowed(actx.root_input())
}

/// Native arena-mode `missing`. Accumulates missing-path strings directly
/// into the arena.
#[inline]
pub(crate) fn evaluate_missing_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let lookup = lookup_snapshot(actx);
    // Pre-size for the worst case where every direct path is missing — we'd
    // push one entry per arg (array-shaped args may exceed but those expand).
    // Saves the first growth allocation in the typical "few-paths-checked"
    // call shape that dominates compatible.json.
    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(args.len(), arena);

    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        match av {
            ArenaValue::Array(items) => {
                for it in *items {
                    if let Some(path) = arena_value_as_str(it)
                        && access_path_ref(lookup.as_ref(), path).is_none()
                    {
                        missing.push(ArenaValue::String(arena.alloc_str(path)));
                    }
                }
            }
            ArenaValue::InputRef(Value::Array(arr)) => {
                for v in arr {
                    if let Some(path) = v.as_str()
                        && access_path_ref(lookup.as_ref(), path).is_none()
                    {
                        missing.push(ArenaValue::String(arena.alloc_str(path)));
                    }
                }
            }
            ArenaValue::String(s) => {
                if access_path_ref(lookup.as_ref(), s).is_none() {
                    missing.push(ArenaValue::String(arena.alloc_str(s)));
                }
            }
            ArenaValue::InputRef(Value::String(s)) => {
                if access_path_ref(lookup.as_ref(), s).is_none() {
                    missing.push(ArenaValue::String(arena.alloc_str(s.as_str())));
                }
            }
            _ => {}
        }
    }

    Ok(arena.alloc(ArenaValue::Array(missing.into_bump_slice())))
}

/// Native arena-mode `missing_some`.
#[inline]
pub(crate) fn evaluate_missing_some_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Ok(crate::arena::pool::singleton_empty_array());
    }

    let min_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let min_present = min_av.as_i64().unwrap_or(1).max(0) as usize;

    let paths_av = engine.evaluate_arena_node(&args[1], actx, arena)?;
    let lookup = lookup_snapshot(actx);

    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);
    let mut present_count: usize = 0;

    let process_path = |path: &str,
                        missing: &mut bumpalo::collections::Vec<'a, ArenaValue<'a>>,
                        present_count: &mut usize|
     -> bool {
        if access_path_ref(lookup.as_ref(), path).is_none() {
            missing.push(ArenaValue::String(arena.alloc_str(path)));
        } else {
            *present_count += 1;
            if *present_count >= min_present {
                return true; // short-circuit
            }
        }
        false
    };

    let short_circuit = match paths_av {
        ArenaValue::Array(items) => items.iter().any(|it| {
            arena_value_as_str(it)
                .is_some_and(|p| process_path(p, &mut missing, &mut present_count))
        }),
        ArenaValue::InputRef(Value::Array(arr)) => arr.iter().any(|v| {
            v.as_str()
                .is_some_and(|p| process_path(p, &mut missing, &mut present_count))
        }),
        _ => false,
    };

    if short_circuit || present_count >= min_present {
        return Ok(crate::arena::pool::singleton_empty_array());
    }
    Ok(arena.alloc(ArenaValue::Array(missing.into_bump_slice())))
}

#[inline]
fn arena_value_as_str<'a>(av: &'a ArenaValue<'a>) -> Option<&'a str> {
    match av {
        ArenaValue::String(s) => Some(*s),
        ArenaValue::InputRef(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}
