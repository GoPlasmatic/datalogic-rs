use serde_json::Value;

use crate::value_helpers::access_path_ref;
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// Missing operator function - checks for missing variables
#[inline]
pub fn evaluate_missing(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let mut missing = Vec::new();

    for arg in args {
        let path_val = engine.evaluate_node_cow(arg, context)?;

        match path_val.as_ref() {
            Value::Array(arr) => {
                for v in arr {
                    if let Some(path) = v.as_str()
                        && access_path_ref(context.current().data(), path).is_none()
                    {
                        missing.push(Value::String(path.to_string()));
                    }
                }
            }
            Value::String(s) => {
                if access_path_ref(context.current().data(), s).is_none() {
                    missing.push(Value::String(s.clone()));
                }
            }
            _ => {}
        }
    }

    Ok(Value::Array(missing))
}

/// MissingSome operator function - returns empty array if minimum present fields are met,
/// or array of missing fields otherwise
#[inline]
pub fn evaluate_missing_some(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Array(vec![]));
    }

    // First argument is the minimum number of fields that must be PRESENT
    let min_present_val = engine.evaluate_node_cow(&args[0], context)?;
    let min_present = min_present_val.as_u64().unwrap_or(1) as usize;

    let paths_val = engine.evaluate_node_cow(&args[1], context)?;

    let mut missing = Vec::new();
    let mut present_count = 0;

    if let Value::Array(arr) = paths_val.as_ref() {
        for v in arr {
            if let Some(path) = v.as_str() {
                if access_path_ref(context.current().data(), path).is_none() {
                    missing.push(Value::String(path.to_string()));
                } else {
                    present_count += 1;
                    // Early exit if we've found enough present fields
                    if present_count >= min_present {
                        return Ok(Value::Array(vec![]));
                    }
                }
            }
        }
    }

    // Return empty array if minimum present requirement is met,
    // otherwise return the array of missing fields
    if present_count >= min_present {
        Ok(Value::Array(vec![]))
    } else {
        Ok(Value::Array(missing))
    }
}

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
fn lookup_snapshot<'a>(
    actx: &ArenaContextStack<'a>,
    context: &ContextStack,
) -> LookupSnap<'a> {
    if actx.depth() > 0 {
        use crate::arena::context::ArenaContextRef;
        if let ArenaContextRef::Frame(f) = actx.current() {
            // Frame data is `&'a ArenaValue<'a>`. Borrow when InputRef
            // (zero-cost); materialize only for arena-resident composites.
            return match f.data() {
                ArenaValue::InputRef(v) => LookupSnap::Borrowed(v),
                other => LookupSnap::Owned(arena_to_value(other)),
            };
        }
    }
    if context.depth() > 0 {
        // Legacy bridge inside value-mode iteration (rare post-Stage-D).
        // The value-mode frame holds an owned `Value`; clone is unavoidable
        // because the frame data borrow is tied to the temporary
        // ContextFrameRef. Stage E removes this path entirely.
        return LookupSnap::Owned(context.current().data().clone());
    }
    LookupSnap::Borrowed(actx.root_input())
}

/// Native arena-mode `missing`. Accumulates missing-path strings directly
/// into the arena.
#[inline]
pub(crate) fn evaluate_missing_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let lookup = lookup_snapshot(actx, context);
    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);

    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, context, arena)?;
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
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Ok(crate::arena::pool::singleton_empty_array());
    }

    let min_av = engine.evaluate_arena_node(&args[0], actx, context, arena)?;
    let min_present = min_av.as_i64().unwrap_or(1).max(0) as usize;

    let paths_av = engine.evaluate_arena_node(&args[1], actx, context, arena)?;
    let lookup = lookup_snapshot(actx, context);

    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);
    let mut present_count: usize = 0;

    let mut process_path = |path: &str,
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

