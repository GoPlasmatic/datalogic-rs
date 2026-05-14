//! Arena-mode `val` / `var` evaluation.
//!
//! Both source ops normalise to `OpCode::Val`; the var-specific arg shape
//! (path + default fallback) is collapsed at compile time by
//! `compile::operator::try_compile_var`. This module owns the runtime side:
//! the compiled fast path (`evaluate_val_compiled`) and the dynamic-arg form
//! (`evaluate_val`).

use bumpalo::Bump;

use super::{
    CompiledVarSpec, array_get, array_len, current_data, default_or_null, frame_data_at_level,
    level_marker_from_array, metadata_hint_lookup, path_str_from_data,
};
use crate::arena::{ContextStack, DataValue};
use crate::node::{MetadataHint, PathSegment, ReduceHint};
use crate::{CompiledNode, Error, Result};

/// Arena variant of `evaluate_val_compiled`. Dispatches through four
/// resolution stages in order:
///
/// 1. **Metadata** (`{"val": [n, "index"]}` / `"key"`) — reads the iteration
///    frame's bookkeeping directly.
/// 2. **Reduce** (`current` / `accumulator` and their `.path` siblings) —
///    reads the reduce frame's slots.
/// 3. **Root-scope fast path** (`scope_level == 0` at root depth) — arena
///    traversal straight from the input, no frame walk. This is the dominant
///    path in real workloads and stays inline for branch-prediction.
/// 4. **General context-stack walk** — for non-root scopes (`{"val": [[1], …]}`).
///
/// Each branch falls through to `default_or_null` on miss; the var's
/// `default_value` (when present) is evaluated lazily there.
#[inline(always)]
pub(crate) fn evaluate_val_compiled<'a>(
    spec: CompiledVarSpec<'a>,
    ctx: &mut ContextStack<'a>,
    engine: &crate::Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let CompiledVarSpec {
        scope_level,
        segments,
        reduce_hint,
        metadata_hint,
        default_value,
    } = spec;

    // Dominant-case fast path: plain root-scope `var` with no metadata/reduce
    // hints, evaluated outside any iteration frame. Probed first as a single
    // combined branch so the common case never pays for the metadata-hint
    // pattern match or the reduce-hint frame inspection below.
    if metadata_hint == MetadataHint::None
        && reduce_hint == ReduceHint::None
        && scope_level == 0
        && ctx.depth() == 0
    {
        let root_av = ctx.root_input();
        let resolved = if segments.is_empty() {
            Some(root_av)
        } else {
            crate::arena::value::traverse_segments(root_av, segments)
        };
        return match resolved {
            Some(av) => Ok(av),
            None => default_or_null(default_value, ctx, engine, arena),
        };
    }

    if let Some(av) = resolve_metadata_hint(metadata_hint, ctx, arena) {
        return Ok(av);
    }

    if let Some(res) = resolve_reduce_hint(reduce_hint, segments, ctx, engine, arena, default_value)
    {
        return res;
    }

    resolve_via_context_stack(scope_level, segments, ctx, engine, arena, default_value)
}

/// Stage 1 — metadata hints (`index` / `key`) read from the current iteration
/// frame. Returns `Some(av)` only when the corresponding slot is populated;
/// `None` lets the caller fall through to the next stage.
#[inline]
fn resolve_metadata_hint<'a>(
    hint: MetadataHint,
    ctx: &ContextStack<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    match hint {
        MetadataHint::Index => ctx.current().get_index().map(|idx| {
            let i = idx as i64;
            crate::arena::singletons::singleton_small_int(i).unwrap_or_else(|| {
                &*arena.alloc(DataValue::Number(datavalue::NumberValue::Integer(i)))
            })
        }),
        // `key` already has lifetime `'a` (object pairs live in the arena
        // for the call) — no `alloc_str` copy needed; only the `String`
        // wrapper requires a bump alloc.
        MetadataHint::Key => ctx
            .current()
            .get_key()
            .map(|key| &*arena.alloc(DataValue::String(key))),
        MetadataHint::None => None,
    }
}

/// Stage 2 — reduce-frame hints. `Current` / `Accumulator` return the slot
/// directly; `CurrentPath` / `AccumulatorPath` traverse `segments[1..]` on
/// the slot (segments[0] is `current`/`accumulator`). Returns:
/// - `Some(Ok(av))` — slot resolved.
/// - `Some(Err(...))` or `Some(Ok(default))` — frame existed but path missed.
/// - `None` — no reduce frame at the current depth; fall through.
#[inline]
fn resolve_reduce_hint<'a>(
    reduce_hint: ReduceHint,
    segments: &[PathSegment],
    ctx: &mut ContextStack<'a>,
    engine: &crate::Engine,
    arena: &'a Bump,
    default_value: Option<&'a CompiledNode>,
) -> Option<Result<&'a DataValue<'a>>> {
    if reduce_hint == ReduceHint::None || ctx.depth() == 0 {
        return None;
    }
    use crate::arena::context::ContextRef;
    let ContextRef::Frame(f) = ctx.current() else {
        return None;
    };

    match reduce_hint {
        ReduceHint::Current => f.get_reduce_current().map(Ok),
        ReduceHint::Accumulator => f.get_reduce_accumulator().map(Ok),
        ReduceHint::CurrentPath | ReduceHint::AccumulatorPath => {
            let slot = if reduce_hint == ReduceHint::CurrentPath {
                f.get_reduce_current()
            } else {
                f.get_reduce_accumulator()
            };
            // Slot must exist for the frame to be considered a reduce frame.
            // If the path traversal misses, return the var's `default_value`.
            let slot = slot?;
            let resolved = crate::arena::value::traverse_segments(slot, &segments[1..]);
            Some(match resolved {
                Some(av) => Ok(av),
                None => default_or_null(default_value, ctx, engine, arena),
            })
        }
        ReduceHint::None => unreachable!(),
    }
}

/// Stage 4 — generic context-stack walk for non-root scopes. `scope_level`
/// of 0 at non-root depth reads the current frame; positive levels walk up.
#[inline]
fn resolve_via_context_stack<'a>(
    scope_level: u32,
    segments: &[PathSegment],
    ctx: &mut ContextStack<'a>,
    engine: &crate::Engine,
    arena: &'a Bump,
    default_value: Option<&'a CompiledNode>,
) -> Result<&'a DataValue<'a>> {
    use crate::arena::context::ContextRef;
    let aref = if scope_level == 0 {
        ctx.current()
    } else {
        ctx.get_at_level(scope_level as isize)
            .ok_or(Error::invalid_context_level(scope_level as isize))?
    };
    let av = match aref {
        ContextRef::Frame(f) => f.data(),
        ContextRef::Root(av) => av,
    };
    if segments.is_empty() {
        return Ok(av);
    }
    match crate::arena::value::traverse_segments(av, segments) {
        Some(child) => Ok(child),
        None => default_or_null(default_value, ctx, engine, arena),
    }
}

/// Arena-native `val` operator. Mirrors the value-mode shape (level access,
/// path chains, reduce shortcuts) but stays on `&DataValue` throughout.
///
/// Three branches:
/// - **Empty args** → current frame data.
/// - **Multi-arg** ([`eval_val_multiarg`]) — distinguishes `[[level], …]`
///   from path-chain at runtime by looking at the first arg.
/// - **Single arg** — null/array/scalar dispatch via
///   [`eval_val_array_path`] and [`eval_val_scalar_path`].
///
/// Also handles the dynamic-fallback `var` shape (path + default) — when
/// `var`'s args[0] is a non-array path that resolves to null and args[1]
/// exists, args[1] is evaluated as the default. The path-chain
/// interpretation (`{"val": ["a", "b"]}` walks `a.b`) is unaffected because
/// chain-walking runs ahead of the fallback check.
#[inline]
pub(crate) fn evaluate_val<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &crate::Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(current_data(ctx, arena));
    }
    if args.len() >= 2 {
        return eval_val_multiarg(args, ctx, engine, arena);
    }
    let path_av = engine.dispatch_node(&args[0], ctx, arena)?;
    if matches!(path_av, DataValue::Null) {
        return Ok(current_data(ctx, arena));
    }
    if let Some(arr_len) = array_len(path_av) {
        return eval_val_array_path(path_av, arr_len, ctx, arena);
    }
    eval_val_scalar_path(path_av, ctx, arena)
}

/// Multi-arg `val` form (`args.len() >= 2`). Evaluates `args[0]` once and
/// branches on whether it is a `[level]` marker:
/// - `[[level], path...]` — frame walk at relative level, optional metadata
///   short-circuit when there are exactly 2 args.
/// - Otherwise — path chain on current data, with reduce shortcut for
///   the first segment (`current` / `accumulator`).
fn eval_val_multiarg<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &crate::Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    use crate::arena::context::ContextRef;
    use crate::arena::value::{access_path_str_ref, apply_path_element};

    let first_av = engine.dispatch_node(&args[0], ctx, arena)?;
    if let Some(level) = level_marker_from_array(first_av) {
        // Metadata short-circuits — only valid with exactly 2 args.
        if args.len() == 2 {
            let path_av = engine.dispatch_node(&args[1], ctx, arena)?;
            let path_str = path_av.as_str().unwrap_or("");
            if let Some(av) = metadata_hint_lookup(ctx, path_str, arena) {
                return Ok(av);
            }

            let path_str = path_str_from_data(path_av, arena);
            let frame_data = frame_data_at_level(ctx, level as isize, arena)
                .ok_or(Error::invalid_context_level(level as isize))?;
            return Ok(access_path_str_ref(frame_data, path_str)
                .unwrap_or_else(|| crate::arena::singletons::singleton_null()));
        }

        // Multi-arg path chain at a relative level.
        let mut paths: bumpalo::collections::Vec<'a, &'a str> =
            bumpalo::collections::Vec::with_capacity_in(args.len() - 1, arena);
        for item in args.iter().skip(1) {
            let av = engine.dispatch_node(item, ctx, arena)?;
            paths.push(path_str_from_data(av, arena));
        }
        let mut cur = frame_data_at_level(ctx, level as isize, arena)
            .ok_or(Error::invalid_context_level(level as isize))?;
        for path in paths.iter() {
            match access_path_str_ref(cur, path) {
                Some(next) => cur = next,
                None => return Ok(crate::arena::singletons::singleton_null()),
            }
        }
        return Ok(cur);
    }

    // Non-level multi-arg path chain: pre-eval all args.
    let mut evaluated: bumpalo::collections::Vec<'a, &'a DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(args.len(), arena);
    evaluated.push(first_av);
    for arg in args.iter().skip(1) {
        evaluated.push(engine.dispatch_node(arg, ctx, arena)?);
    }

    // Reduce shortcut for the first segment.
    let mut start: Option<&'a DataValue<'a>> = None;
    if let ContextRef::Frame(frame) = ctx.current() {
        if let Some(s) = evaluated[0].as_str() {
            start = if s == "current" {
                frame.get_reduce_current()
            } else if s == "accumulator" {
                frame.get_reduce_accumulator()
            } else {
                None
            };
        }
    }

    let (mut cur, rest_start) = match start {
        Some(s) => (s, 1),
        None => (current_data(ctx, arena), 0),
    };
    for elem in &evaluated[rest_start..] {
        match apply_path_element(cur, elem) {
            Some(next) => cur = next,
            None => return Ok(crate::arena::singletons::singleton_null()),
        }
    }
    Ok(cur)
}

/// Single-arg `val` where the path arg evaluated to an array. Distinguishes
/// `[[level], path...]` from a plain path-chain array. Empty array →
/// current data (matches `{"var": []}` semantics).
fn eval_val_array_path<'a>(
    path_av: &'a DataValue<'a>,
    arr_len: usize,
    ctx: &mut ContextStack<'a>,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    use crate::arena::value::{access_path_str_ref, apply_path_element};

    if arr_len == 0 {
        return Ok(current_data(ctx, arena));
    }
    if arr_len >= 2 {
        let level_opt = array_get(path_av, 0).and_then(|e| match e {
            DataValue::Array(level_arr) if !level_arr.is_empty() => level_arr[0].as_i64(),
            _ => None,
        });
        if let Some(level) = level_opt {
            if arr_len == 2 {
                let second = array_get(path_av, 1)
                    .unwrap_or_else(|| crate::arena::singletons::singleton_null());
                let path_str = second.as_str().unwrap_or("");
                if let Some(av) = metadata_hint_lookup(ctx, path_str, arena) {
                    return Ok(av);
                }
            }

            let mut cur = frame_data_at_level(ctx, level as isize, arena)
                .ok_or(Error::invalid_context_level(level as isize))?;
            for i in 1..arr_len {
                let item = array_get(path_av, i)
                    .unwrap_or_else(|| crate::arena::singletons::singleton_null());
                let Some(seg) = item.as_str() else {
                    return Ok(crate::arena::singletons::singleton_null());
                };
                match access_path_str_ref(cur, seg) {
                    Some(next) => cur = next,
                    None => return Ok(crate::arena::singletons::singleton_null()),
                }
            }
            return Ok(cur);
        }
    }

    // Plain path-chain array.
    let mut cur = current_data(ctx, arena);
    for i in 0..arr_len {
        let elem =
            array_get(path_av, i).unwrap_or_else(|| crate::arena::singletons::singleton_null());
        match apply_path_element(cur, elem) {
            Some(next) => cur = next,
            None => return Ok(crate::arena::singletons::singleton_null()),
        }
    }
    Ok(cur)
}

/// Single-arg `val` where the path arg is a string or numeric scalar.
/// Strings get the reduce-shortcut probe (`current` / `accumulator` /
/// dotted siblings) and the "direct key wins over dotted-path" rule;
/// non-negative integers index a numeric key on current data.
fn eval_val_scalar_path<'a>(
    path_av: &'a DataValue<'a>,
    ctx: &ContextStack<'a>,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    use crate::arena::context::ContextRef;
    use crate::arena::value::access_path_str_ref;

    if let Some(s) = path_av.as_str() {
        if let ContextRef::Frame(frame) = ctx.current() {
            if s == "current" {
                if let Some(av) = frame.get_reduce_current() {
                    return Ok(av);
                }
            } else if s == "accumulator" {
                if let Some(av) = frame.get_reduce_accumulator() {
                    return Ok(av);
                }
            } else if let Some(rest) = s.strip_prefix("current.") {
                if let Some(cur) = frame.get_reduce_current() {
                    return Ok(access_path_str_ref(cur, rest)
                        .unwrap_or_else(|| crate::arena::singletons::singleton_null()));
                }
            } else if let Some(rest) = s.strip_prefix("accumulator.") {
                if let Some(acc) = frame.get_reduce_accumulator() {
                    return Ok(access_path_str_ref(acc, rest)
                        .unwrap_or_else(|| crate::arena::singletons::singleton_null()));
                }
            }
        }

        let cur = current_data(ctx, arena);
        // Direct object key lookup beats dot-path traversal so empty keys and
        // keys containing dots resolve correctly.
        if let DataValue::Object(pairs) = cur {
            if let Some(av) = crate::arena::value::object_lookup_field(pairs, s) {
                return Ok(av);
            }
        }
        return Ok(access_path_str_ref(cur, s)
            .unwrap_or_else(|| crate::arena::singletons::singleton_null()));
    }

    if let Some(i) = path_av.as_i64() {
        if i >= 0 {
            let cur = current_data(ctx, arena);
            // Common small indices (0..100) hit the static `&'static str`
            // cache; only larger keys pay the heap `String` allocation.
            if let Some(static_key) = super::small_int_str(i) {
                return Ok(access_path_str_ref(cur, static_key)
                    .unwrap_or_else(|| crate::arena::singletons::singleton_null()));
            }
            let key = i.to_string();
            return Ok(access_path_str_ref(cur, &key)
                .unwrap_or_else(|| crate::arena::singletons::singleton_null()));
        }
    }

    Ok(crate::arena::singletons::singleton_null())
}
