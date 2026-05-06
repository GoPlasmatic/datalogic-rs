use crate::{CompiledNode, Engine, Result};

// =============================================================================
// Arena-mode missing / missing_some
//
// Path lookups walk `&DataValue` natively via `path_exists_*`.
// =============================================================================

use crate::arena::{ContextStack, DataValue};
use bumpalo::Bump;

/// Resolve the lookup-target for `missing` / `missing_some` — current
/// context's data view as `&'a DataValue<'a>`.
#[inline(always)]
fn lookup_data<'a>(ctx: &ContextStack<'a>) -> &'a DataValue<'a> {
    if ctx.depth() > 0 {
        use crate::arena::context::ContextRef;
        match ctx.current() {
            ContextRef::Frame(f) => f.data(),
            ContextRef::Root(av) => av,
        }
    } else {
        ctx.root_input()
    }
}

/// Native arena-mode `missing`. Accumulates missing-path strings directly
/// into the arena.
#[inline]
pub(crate) fn evaluate_missing<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let lookup = lookup_data(ctx);
    let mut missing: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(args.len(), arena);

    for arg in args {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        match av {
            DataValue::Array(items) => {
                for it in *items {
                    if let Some(path) = value_as_str(it)
                        && !crate::arena::value::path_exists_str(lookup, path)
                    {
                        missing.push(DataValue::String(arena.alloc_str(path)));
                    }
                }
            }
            DataValue::String(s) => {
                if !crate::arena::value::path_exists_str(lookup, s) {
                    missing.push(DataValue::String(arena.alloc_str(s)));
                }
            }
            _ => {}
        }
    }

    if missing.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }
    Ok(arena.alloc(DataValue::Array(missing.into_bump_slice())))
}

/// Native arena-mode `missing_some`.
#[inline]
pub(crate) fn evaluate_missing_some<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }

    let min_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let min_present = min_av.as_i64().unwrap_or(1).max(0) as usize;

    let paths_av = engine.dispatch_node(&args[1], ctx, arena)?;
    let lookup = lookup_data(ctx);

    let mut missing: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);
    let mut present_count: usize = 0;

    let process_path = |path: &str,
                        missing: &mut bumpalo::collections::Vec<'a, DataValue<'a>>,
                        present_count: &mut usize|
     -> bool {
        if !crate::arena::value::path_exists_str(lookup, path) {
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
            value_as_str(it).is_some_and(|p| process_path(p, &mut missing, &mut present_count))
        }),
        _ => false,
    };

    if short_circuit || present_count >= min_present || missing.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }
    Ok(arena.alloc(DataValue::Array(missing.into_bump_slice())))
}

#[inline]
fn value_as_str<'a>(av: &'a DataValue<'a>) -> Option<&'a str> {
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
/// `path_exists_segments`; dynamic args use the runtime path string.
///
/// Defers the per-call bumpalo `Vec` allocation until the first missing
/// path is found. The dominant case in real workloads (and in
/// `compatible.json`) is "all paths present" → return the static
/// empty-array singleton without touching the arena at all.
#[inline]
pub(crate) fn evaluate_compiled_missing<'a>(
    data: &'a CompiledMissingData,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let lookup = lookup_data(ctx);
    let mut missing: Option<bumpalo::collections::Vec<'a, DataValue<'a>>> = None;

    for arg in data.args.iter() {
        match arg {
            CompiledMissingArg::Now((path, segments)) => {
                if !crate::arena::value::path_exists_segments(lookup, segments) {
                    missing
                        .get_or_insert_with(|| {
                            bumpalo::collections::Vec::with_capacity_in(data.args.len(), arena)
                        })
                        .push(DataValue::String(path.as_ref()));
                }
            }
            CompiledMissingArg::Later(node) => {
                let av = engine.dispatch_node(node, ctx, arena)?;
                let buf = missing.get_or_insert_with(|| {
                    bumpalo::collections::Vec::with_capacity_in(data.args.len(), arena)
                });
                accumulate_dynamic_missing(av, lookup, buf, arena);
            }
        }
    }
    match missing {
        Some(v) if !v.is_empty() => Ok(arena.alloc(DataValue::Array(v.into_bump_slice()))),
        _ => Ok(crate::arena::singletons::singleton_empty_array()),
    }
}

/// Evaluate a `missing_some` op whose literal min-count and literal array-of-
/// strings paths have been pre-resolved at compile time.
#[inline]
pub(crate) fn evaluate_compiled_missing_some<'a>(
    data: &'a CompiledMissingSomeData,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let min_present = match &data.min_present {
        CompiledMissingMin::Now(n) => *n,
        CompiledMissingMin::Later(node) => {
            let av = engine.dispatch_node(node, ctx, arena)?;
            av.as_i64().unwrap_or(1).max(0) as usize
        }
    };

    let lookup = lookup_data(ctx);

    match &data.paths {
        CompiledMissingPaths::Now(paths) => {
            // Defer the missing-paths buffer until the first path actually
            // misses — the dominant success case ("all paths present"
            // hits min_present early) returns the empty-array singleton
            // without allocating.
            let mut missing: Option<bumpalo::collections::Vec<'a, DataValue<'a>>> = None;
            let mut present = 0usize;
            for (path, segments) in paths.iter() {
                if crate::arena::value::path_exists_segments(lookup, segments) {
                    present += 1;
                    if present >= min_present {
                        return Ok(crate::arena::singletons::singleton_empty_array());
                    }
                } else {
                    missing
                        .get_or_insert_with(|| {
                            bumpalo::collections::Vec::with_capacity_in(paths.len(), arena)
                        })
                        .push(DataValue::String(path.as_ref()));
                }
            }
            match missing {
                Some(v) if present < min_present && !v.is_empty() => {
                    Ok(arena.alloc(DataValue::Array(v.into_bump_slice())))
                }
                _ => Ok(crate::arena::singletons::singleton_empty_array()),
            }
        }
        CompiledMissingPaths::Later(node) => {
            let paths_av = engine.dispatch_node(node, ctx, arena)?;
            let mut missing: bumpalo::collections::Vec<'a, DataValue<'a>> =
                bumpalo::collections::Vec::new_in(arena);
            let mut present = 0usize;
            let short = match paths_av {
                DataValue::Array(items) => items.iter().any(|it| {
                    value_as_str(it).is_some_and(|p| {
                        check_path(p, lookup, &mut missing, &mut present, min_present, arena)
                    })
                }),
                _ => false,
            };
            if short || present >= min_present || missing.is_empty() {
                return Ok(crate::arena::singletons::singleton_empty_array());
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
    if !crate::arena::value::path_exists_str(lookup, path) {
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
                if let Some(path) = value_as_str(it)
                    && !crate::arena::value::path_exists_str(lookup, path)
                {
                    missing.push(DataValue::String(arena.alloc_str(path)));
                }
            }
        }
        DataValue::String(s) => {
            if !crate::arena::value::path_exists_str(lookup, s) {
                missing.push(DataValue::String(arena.alloc_str(s)));
            }
        }
        _ => {}
    }
}
