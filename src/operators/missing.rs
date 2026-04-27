use crate::{CompiledNode, DataLogic, Result};

// =============================================================================
// Arena-mode missing / missing_some
//
// Path lookups walk `&DataValue` natively via `arena_path_exists_*`.
// =============================================================================

use crate::arena::{DataContextStack, DataValue};
use bumpalo::Bump;

/// Resolve the lookup-target for `missing` / `missing_some` — current
/// context's data view as `&'a DataValue<'a>`.
#[inline]
fn lookup_av<'a>(actx: &DataContextStack<'a>) -> &'a DataValue<'a> {
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
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let lookup = lookup_av(actx);
    let mut missing: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(args.len(), arena);

    for arg in args {
        let av = engine.evaluate_node(arg, actx, arena)?;
        match av {
            DataValue::Array(items) => {
                for it in *items {
                    if let Some(path) = arena_value_as_str(it)
                        && !crate::arena::value::arena_path_exists_str(lookup, path)
                    {
                        missing.push(DataValue::String(arena.alloc_str(path)));
                    }
                }
            }
            DataValue::String(s) => {
                if !crate::arena::value::arena_path_exists_str(lookup, s) {
                    missing.push(DataValue::String(arena.alloc_str(s)));
                }
            }
            _ => {}
        }
    }

    Ok(arena.alloc(DataValue::Array(missing.into_bump_slice())))
}

/// Native arena-mode `missing_some`.
#[inline]
pub(crate) fn evaluate_missing_some_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Ok(crate::arena::pool::singleton_empty_array());
    }

    let min_av = engine.evaluate_node(&args[0], actx, arena)?;
    let min_present = min_av.as_i64().unwrap_or(1).max(0) as usize;

    let paths_av = engine.evaluate_node(&args[1], actx, arena)?;
    let lookup = lookup_av(actx);

    let mut missing: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);
    let mut present_count: usize = 0;

    let process_path = |path: &str,
                        missing: &mut bumpalo::collections::Vec<'a, DataValue<'a>>,
                        present_count: &mut usize|
     -> bool {
        if !crate::arena::value::arena_path_exists_str(lookup, path) {
            missing.push(DataValue::String(arena.alloc_str(path)));
        } else {
            *present_count += 1;
            if *present_count >= min_present {
                return true; // short-circuit
            }
        }
        false
    };

    let short_circuit = match paths_av {
        DataValue::Array(items) => items.iter().any(|it| {
            arena_value_as_str(it)
                .is_some_and(|p| process_path(p, &mut missing, &mut present_count))
        }),
        _ => false,
    };

    if short_circuit || present_count >= min_present {
        return Ok(crate::arena::pool::singleton_empty_array());
    }
    Ok(arena.alloc(DataValue::Array(missing.into_bump_slice())))
}

#[inline]
fn arena_value_as_str<'a>(av: &'a DataValue<'a>) -> Option<&'a str> {
    match av {
        DataValue::String(s) => Some(*s),
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
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let lookup = lookup_av(actx);
    let mut missing: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(data.args.len(), arena);

    for arg in data.args.iter() {
        match arg {
            CompiledMissingArg::Static { path, segments } => {
                if !crate::arena::value::arena_path_exists_segments(lookup, segments) {
                    missing.push(DataValue::String(path.as_ref()));
                }
            }
            CompiledMissingArg::Dynamic(node) => {
                let av = engine.evaluate_node(node, actx, arena)?;
                accumulate_dynamic_missing(av, lookup, &mut missing, arena);
            }
        }
    }
    Ok(arena.alloc(DataValue::Array(missing.into_bump_slice())))
}

/// Evaluate a `missing_some` op whose literal min-count and literal array-of-
/// strings paths have been pre-resolved at compile time.
#[inline]
pub(crate) fn evaluate_compiled_missing_some_arena<'a>(
    data: &'a CompiledMissingSomeData,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let min_present = match &data.min_present {
        CompiledMissingMin::Static(n) => *n,
        CompiledMissingMin::Dynamic(node) => {
            let av = engine.evaluate_node(node, actx, arena)?;
            av.as_i64().unwrap_or(1).max(0) as usize
        }
    };

    let lookup = lookup_av(actx);

    match &data.paths {
        CompiledMissingPaths::Static(paths) => {
            let mut missing: bumpalo::collections::Vec<'a, DataValue<'a>> =
                bumpalo::collections::Vec::with_capacity_in(paths.len(), arena);
            let mut present = 0usize;
            for (path, segments) in paths.iter() {
                if crate::arena::value::arena_path_exists_segments(lookup, segments) {
                    present += 1;
                    if present >= min_present {
                        return Ok(crate::arena::pool::singleton_empty_array());
                    }
                } else {
                    missing.push(DataValue::String(path.as_ref()));
                }
            }
            if present >= min_present {
                return Ok(crate::arena::pool::singleton_empty_array());
            }
            Ok(arena.alloc(DataValue::Array(missing.into_bump_slice())))
        }
        CompiledMissingPaths::Dynamic(node) => {
            let paths_av = engine.evaluate_node(node, actx, arena)?;
            let mut missing: bumpalo::collections::Vec<'a, DataValue<'a>> =
                bumpalo::collections::Vec::new_in(arena);
            let mut present = 0usize;
            let short = match paths_av {
                DataValue::Array(items) => items.iter().any(|it| {
                    arena_value_as_str(it).is_some_and(|p| {
                        check_path(p, lookup, &mut missing, &mut present, min_present, arena)
                    })
                }),
                _ => false,
            };
            if short || present >= min_present {
                return Ok(crate::arena::pool::singleton_empty_array());
            }
            Ok(arena.alloc(DataValue::Array(missing.into_bump_slice())))
        }
    }
}

#[inline]
fn check_path<'a>(
    path: &str,
    lookup: &'a DataValue<'a>,
    missing: &mut bumpalo::collections::Vec<'a, DataValue<'a>>,
    present: &mut usize,
    min_present: usize,
    arena: &'a Bump,
) -> bool {
    if !crate::arena::value::arena_path_exists_str(lookup, path) {
        missing.push(DataValue::String(arena.alloc_str(path)));
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
    av: &'a DataValue<'a>,
    lookup: &'a DataValue<'a>,
    missing: &mut bumpalo::collections::Vec<'a, DataValue<'a>>,
    arena: &'a Bump,
) {
    match av {
        DataValue::Array(items) => {
            for it in *items {
                if let Some(path) = arena_value_as_str(it)
                    && !crate::arena::value::arena_path_exists_str(lookup, path)
                {
                    missing.push(DataValue::String(arena.alloc_str(path)));
                }
            }
        }
        DataValue::String(s) => {
            if !crate::arena::value::arena_path_exists_str(lookup, s) {
                missing.push(DataValue::String(arena.alloc_str(s)));
            }
        }
        _ => {}
    }
}
