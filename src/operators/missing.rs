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

use crate::arena::{ArenaContextStack, ArenaValue, value_to_arena};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_missing_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    _root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);

    for arg in args {
        let path_val = engine.evaluate_node_cow(arg, context)?;
        match path_val.as_ref() {
            Value::Array(arr) => {
                for v in arr {
                    if let Some(path) = v.as_str()
                        && access_path_ref(context.current().data(), path).is_none()
                    {
                        let s = arena.alloc_str(path);
                        missing.push(ArenaValue::String(s));
                    }
                }
            }
            Value::String(s) => {
                if access_path_ref(context.current().data(), s).is_none() {
                    let owned = arena.alloc_str(s);
                    missing.push(ArenaValue::String(owned));
                }
            }
            _ => {}
        }
    }

    Ok(arena.alloc(ArenaValue::Array(missing.into_bump_slice())))
}

#[inline]
pub(crate) fn evaluate_missing_some_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    _root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        // Bridge: matches the value-mode "return empty array" semantics.
        let v = evaluate_missing_some(args, context, engine)?;
        return Ok(arena.alloc(value_to_arena(&v, arena)));
    }

    let min_present_val = engine.evaluate_node_cow(&args[0], context)?;
    let min_present = min_present_val.as_u64().unwrap_or(1) as usize;
    let paths_val = engine.evaluate_node_cow(&args[1], context)?;

    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);
    let mut present_count = 0usize;

    if let Value::Array(arr) = paths_val.as_ref() {
        for v in arr {
            if let Some(path) = v.as_str() {
                if access_path_ref(context.current().data(), path).is_none() {
                    let s = arena.alloc_str(path);
                    missing.push(ArenaValue::String(s));
                } else {
                    present_count += 1;
                    if present_count >= min_present {
                        return Ok(arena.alloc(ArenaValue::Array(&[])));
                    }
                }
            }
        }
    }

    if present_count >= min_present {
        Ok(arena.alloc(ArenaValue::Array(&[])))
    } else {
        Ok(arena.alloc(ArenaValue::Array(missing.into_bump_slice())))
    }
}
