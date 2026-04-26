use serde_json::Value;

use crate::{CompiledNode, DataLogic, Result};

// =============================================================================
// Arena-mode missing / missing_some
//
// Path lookups walk `&ArenaValue` natively via `arena_path_exists_*`. For
// `InputRef(v)` operands the helper delegates to the legacy `&Value` walker
// internally; arena-native variants walk the slice form inline. No `Value`
// materialization for non-InputRef arena values.
// =============================================================================

use crate::arena::{ArenaContextStack, ArenaValue};
use bumpalo::Bump;

/// Resolve the lookup-target for `missing` / `missing_some` — current
/// context's data view as `&'a ArenaValue<'a>`.
#[inline]
fn lookup_av<'a>(actx: &ArenaContextStack<'a>) -> &'a ArenaValue<'a> {
    if actx.depth() > 0 {
        use crate::arena::context::ArenaContextRef;
        match actx.current() {
            ArenaContextRef::Frame(f) => f.data(),
            ArenaContextRef::Root(av) => av,
        }
    } else {
        actx.root_input()
    }
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
    let lookup = lookup_av(actx);
    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(args.len(), arena);

    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        match av {
            ArenaValue::Array(items) => {
                for it in *items {
                    if let Some(path) = arena_value_as_str(it)
                        && !crate::arena::value::arena_path_exists_str(lookup, path)
                    {
                        missing.push(ArenaValue::String(arena.alloc_str(path)));
                    }
                }
            }
            ArenaValue::InputRef(Value::Array(arr)) => {
                for v in arr {
                    if let Some(path) = v.as_str()
                        && !crate::arena::value::arena_path_exists_str(lookup, path)
                    {
                        missing.push(ArenaValue::String(arena.alloc_str(path)));
                    }
                }
            }
            ArenaValue::String(s) => {
                if !crate::arena::value::arena_path_exists_str(lookup, s) {
                    missing.push(ArenaValue::String(arena.alloc_str(s)));
                }
            }
            ArenaValue::InputRef(Value::String(s)) => {
                if !crate::arena::value::arena_path_exists_str(lookup, s) {
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
    let lookup = lookup_av(actx);

    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);
    let mut present_count: usize = 0;

    let process_path = |path: &str,
                        missing: &mut bumpalo::collections::Vec<'a, ArenaValue<'a>>,
                        present_count: &mut usize|
     -> bool {
        if !crate::arena::value::arena_path_exists_str(lookup, path) {
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

// =============================================================================
// Pre-compiled missing / missing_some — paths parsed into segments at compile.
// =============================================================================

use crate::node::{
    CompiledMissingArg, CompiledMissingData, CompiledMissingMin, CompiledMissingPaths,
    CompiledMissingSomeData,
};

/// Evaluate a `missing` op whose static literal-string paths have been
/// pre-parsed into segments. Static paths walk via
/// `arena_path_exists_segments`; dynamic args use the runtime path string.
#[inline]
pub(crate) fn evaluate_compiled_missing_arena<'a>(
    data: &'a CompiledMissingData,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let lookup = lookup_av(actx);
    let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(data.args.len(), arena);

    for arg in data.args.iter() {
        match arg {
            CompiledMissingArg::Static { path, segments } => {
                if !crate::arena::value::arena_path_exists_segments(lookup, segments) {
                    missing.push(ArenaValue::String(path.as_ref()));
                }
            }
            CompiledMissingArg::Dynamic(node) => {
                let av = engine.evaluate_arena_node(node, actx, arena)?;
                accumulate_dynamic_missing(av, lookup, &mut missing, arena);
            }
        }
    }
    Ok(arena.alloc(ArenaValue::Array(missing.into_bump_slice())))
}

/// Evaluate a `missing_some` op whose literal min-count and literal array-of-
/// strings paths have been pre-resolved at compile time.
#[inline]
pub(crate) fn evaluate_compiled_missing_some_arena<'a>(
    data: &'a CompiledMissingSomeData,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let min_present = match &data.min_present {
        CompiledMissingMin::Static(n) => *n,
        CompiledMissingMin::Dynamic(node) => {
            let av = engine.evaluate_arena_node(node, actx, arena)?;
            av.as_i64().unwrap_or(1).max(0) as usize
        }
    };

    let lookup = lookup_av(actx);

    match &data.paths {
        CompiledMissingPaths::Static(paths) => {
            let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
                bumpalo::collections::Vec::with_capacity_in(paths.len(), arena);
            let mut present = 0usize;
            for (path, segments) in paths.iter() {
                if crate::arena::value::arena_path_exists_segments(lookup, segments) {
                    present += 1;
                    if present >= min_present {
                        return Ok(crate::arena::pool::singleton_empty_array());
                    }
                } else {
                    missing.push(ArenaValue::String(path.as_ref()));
                }
            }
            if present >= min_present {
                return Ok(crate::arena::pool::singleton_empty_array());
            }
            Ok(arena.alloc(ArenaValue::Array(missing.into_bump_slice())))
        }
        CompiledMissingPaths::Dynamic(node) => {
            let paths_av = engine.evaluate_arena_node(node, actx, arena)?;
            let mut missing: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
                bumpalo::collections::Vec::new_in(arena);
            let mut present = 0usize;
            let short = match paths_av {
                ArenaValue::Array(items) => items.iter().any(|it| {
                    arena_value_as_str(it).is_some_and(|p| {
                        check_path(p, lookup, &mut missing, &mut present, min_present, arena)
                    })
                }),
                ArenaValue::InputRef(Value::Array(arr)) => arr.iter().any(|v| {
                    v.as_str().is_some_and(|p| {
                        check_path(p, lookup, &mut missing, &mut present, min_present, arena)
                    })
                }),
                _ => false,
            };
            if short || present >= min_present {
                return Ok(crate::arena::pool::singleton_empty_array());
            }
            Ok(arena.alloc(ArenaValue::Array(missing.into_bump_slice())))
        }
    }
}

#[inline]
fn check_path<'a>(
    path: &str,
    lookup: &'a ArenaValue<'a>,
    missing: &mut bumpalo::collections::Vec<'a, ArenaValue<'a>>,
    present: &mut usize,
    min_present: usize,
    arena: &'a Bump,
) -> bool {
    if !crate::arena::value::arena_path_exists_str(lookup, path) {
        missing.push(ArenaValue::String(arena.alloc_str(path)));
    } else {
        *present += 1;
        if *present >= min_present {
            return true;
        }
    }
    false
}

#[inline]
fn accumulate_dynamic_missing<'a>(
    av: &'a ArenaValue<'a>,
    lookup: &'a ArenaValue<'a>,
    missing: &mut bumpalo::collections::Vec<'a, ArenaValue<'a>>,
    arena: &'a Bump,
) {
    match av {
        ArenaValue::Array(items) => {
            for it in *items {
                if let Some(path) = arena_value_as_str(it)
                    && !crate::arena::value::arena_path_exists_str(lookup, path)
                {
                    missing.push(ArenaValue::String(arena.alloc_str(path)));
                }
            }
        }
        ArenaValue::InputRef(Value::Array(arr)) => {
            for v in arr {
                if let Some(path) = v.as_str()
                    && !crate::arena::value::arena_path_exists_str(lookup, path)
                {
                    missing.push(ArenaValue::String(arena.alloc_str(path)));
                }
            }
        }
        ArenaValue::String(s) => {
            if !crate::arena::value::arena_path_exists_str(lookup, s) {
                missing.push(ArenaValue::String(arena.alloc_str(s)));
            }
        }
        ArenaValue::InputRef(Value::String(s)) => {
            if !crate::arena::value::arena_path_exists_str(lookup, s) {
                missing.push(ArenaValue::String(arena.alloc_str(s.as_str())));
            }
        }
        _ => {}
    }
}
