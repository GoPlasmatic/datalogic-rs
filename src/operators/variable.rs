
use crate::node::{MetadataHint, PathSegment, ReduceHint};
use crate::{CompiledNode, Error, Result};

// =============================================================================
// Arena-mode variable access
// =============================================================================
//
// Arena variants for var / val / exists. The raw forms
// (`evaluate_var_arena` / `_val_arena` / `_exists_arena`) handle dynamic-path
// expressions natively against the arena context stack.

use crate::arena::{DataContextStack, DataValue};
use bumpalo::Bump;

/// Return the current frame's data as an `&'a DataValue<'a>`. Root and frame
/// branches both return their stored `&DataValue` directly — no per-call
/// allocation.
#[inline]
fn current_data_av<'a>(actx: &DataContextStack<'a>, _arena: &'a Bump) -> &'a DataValue<'a> {
    use crate::arena::context::ArenaContextRef;
    match actx.current() {
        ArenaContextRef::Frame(f) => f.data(),
        ArenaContextRef::Root(av) => av,
    }
}

/// Frame data at a given level (or `None` if the level walks past the root).
#[cfg(feature = "ext-control")]
#[inline]
fn frame_data_at_level<'a>(
    actx: &DataContextStack<'a>,
    level: isize,
    _arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    use crate::arena::context::ArenaContextRef;
    let aref = actx.get_at_level(level)?;
    Some(match aref {
        ArenaContextRef::Frame(f) => f.data(),
        ArenaContextRef::Root(av) => av,
    })
}

/// Coerce an evaluated arena value into a path string. Mirrors the
/// value-mode `match &path_arg { String, Number, _ => "" }` branch.
#[inline]
fn path_string_from_arena(av: &DataValue<'_>) -> String {
    if let Some(s) = av.as_str() {
        return s.to_string();
    }
    if let DataValue::Number(n) = av {
        return n.to_string();
    }
    String::new()
}

/// Pre-compiled `var`/`val` lookup spec — the five fields stored on
/// [`CompiledNode::CompiledVar`], bundled so the arena evaluator takes one
/// borrow instead of five loose params.
pub(crate) struct CompiledVarSpec<'n> {
    pub scope_level: u32,
    pub segments: &'n [PathSegment],
    pub reduce_hint: ReduceHint,
    pub metadata_hint: MetadataHint,
    pub default_value: Option<&'n CompiledNode>,
}

/// Arena variant of `evaluate_compiled_var`. Dispatches through four
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
/// Each branch falls through to `default_or_null_arena` on miss; the var's
/// `default_value` (when present) is evaluated lazily there.
#[inline]
pub(crate) fn evaluate_compiled_var_arena<'a>(
    spec: CompiledVarSpec<'a>,
    actx: &mut DataContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let CompiledVarSpec {
        scope_level,
        segments,
        reduce_hint,
        metadata_hint,
        default_value,
    } = spec;

    if let Some(av) = resolve_metadata_hint(metadata_hint, actx, arena) {
        return Ok(av);
    }

    if let Some(res) =
        resolve_reduce_hint(reduce_hint, segments, actx, engine, arena, default_value)
    {
        return res;
    }

    // Root-scope fast path: arena traversal directly on the root value. Kept
    // inline because it dominates real workloads and branch prediction
    // benefits from a flat call.
    if scope_level == 0 && actx.depth() == 0 {
        let root_av = actx.root_input();
        let resolved = if segments.is_empty() {
            Some(root_av)
        } else {
            crate::arena::value::arena_traverse_segments(root_av, segments, arena)
        };
        return match resolved {
            Some(av) => Ok(av),
            None => default_or_null_arena(default_value, actx, engine, arena),
        };
    }

    resolve_via_context_stack(scope_level, segments, actx, engine, arena, default_value)
}

/// Stage 1 — metadata hints (`index` / `key`) read from the current iteration
/// frame. Returns `Some(av)` only when the corresponding slot is populated;
/// `None` lets the caller fall through to the next stage.
#[inline]
fn resolve_metadata_hint<'a>(
    hint: MetadataHint,
    actx: &DataContextStack<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    match hint {
        MetadataHint::Index => actx.current().get_index().map(|idx| {
            &*arena.alloc(DataValue::Number(crate::value::NumberValue::Integer(
                idx as i64,
            )))
        }),
        MetadataHint::Key => actx.current().get_key().map(|key| {
            let s: &'a str = arena.alloc_str(key);
            &*arena.alloc(DataValue::String(s))
        }),
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
    actx: &mut DataContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
    default_value: Option<&'a CompiledNode>,
) -> Option<Result<&'a DataValue<'a>>> {
    if reduce_hint == ReduceHint::None || actx.depth() == 0 {
        return None;
    }
    use crate::arena::context::ArenaContextRef;
    let ArenaContextRef::Frame(f) = actx.current() else {
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
            let resolved =
                crate::arena::value::arena_traverse_segments(slot, &segments[1..], arena);
            Some(match resolved {
                Some(av) => Ok(av),
                None => default_or_null_arena(default_value, actx, engine, arena),
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
    actx: &mut DataContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
    default_value: Option<&'a CompiledNode>,
) -> Result<&'a DataValue<'a>> {
    use crate::arena::context::ArenaContextRef;
    let aref = if scope_level == 0 {
        actx.current()
    } else {
        actx.get_at_level(scope_level as isize)
            .ok_or(Error::InvalidContextLevel(scope_level as isize))?
    };
    let av = match aref {
        ArenaContextRef::Frame(f) => f.data(),
        ArenaContextRef::Root(av) => av,
    };
    if segments.is_empty() {
        return Ok(av);
    }
    match crate::arena::value::arena_traverse_segments(av, segments, arena) {
        Some(child) => Ok(child),
        None => default_or_null_arena(default_value, actx, engine, arena),
    }
}

/// Arena variant of `evaluate_compiled_exists`. Always returns a Bool singleton.
#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_compiled_exists_arena<'a>(
    scope_level: u32,
    segments: &[PathSegment],
    actx: &mut DataContextStack<'a>,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // Root scope at depth 0: walk input directly (no clone, no frame access).
    if scope_level == 0 && actx.depth() == 0 {
        let found = segments.is_empty()
            || crate::arena::value::arena_traverse_segments(actx.root_input(), segments, arena)
                .is_some();
        return Ok(crate::arena::pool::singleton_bool(found));
    }

    use crate::arena::context::ArenaContextRef;
    let aref = if scope_level == 0 {
        actx.current()
    } else {
        match actx.get_at_level(scope_level as isize) {
            Some(f) => f,
            None => return Ok(crate::arena::pool::singleton_false()),
        }
    };
    let av = match aref {
        ArenaContextRef::Frame(f) => f.data(),
        ArenaContextRef::Root(av) => av,
    };
    let found = segments.is_empty()
        || crate::arena::value::arena_traverse_segments(av, segments, arena).is_some();
    Ok(crate::arena::pool::singleton_bool(found))
}

/// Arena-native `var` operator (path resolved at runtime).
///
/// Raw `var` (not statically compiled to `CompiledVar`) is rare — hit only
/// for dynamic paths like `{"var": [{"if": ...}, "x"]}`. The path string is
/// resolved against the current arena frame's data without any value-mode
/// detour.
#[inline]
pub(crate) fn evaluate_var_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    use crate::arena::value::arena_access_path_str_ref;

    if args.is_empty() {
        return Ok(current_data_av(actx, arena));
    }

    // Resolve path string. Literal string/number is the common shape.
    let owned_path: String;
    let path: &str = match &args[0] {
        CompiledNode::Value {
            value: datavalue::OwnedDataValue::String(s),
            ..
        } => s.as_str(),
        CompiledNode::Value {
            value: datavalue::OwnedDataValue::Number(n),
            ..
        } => {
            owned_path = n.to_string();
            owned_path.as_str()
        }
        other => {
            let av = engine.evaluate_node(other, actx, arena)?;
            owned_path = path_string_from_arena(av);
            owned_path.as_str()
        }
    };

    // Reduce-context fast paths — resolved on the current frame's reduce slots.
    use crate::arena::context::ArenaContextRef;
    if let ArenaContextRef::Frame(frame) = actx.current() {
        if path == "current" {
            if let Some(av) = frame.get_reduce_current() {
                return Ok(av);
            }
        } else if path == "accumulator" {
            if let Some(av) = frame.get_reduce_accumulator() {
                return Ok(av);
            }
        } else if let Some(rest) = path.strip_prefix("current.") {
            if let Some(cur) = frame.get_reduce_current() {
                return Ok(arena_access_path_str_ref(cur, rest, arena)
                    .unwrap_or_else(|| crate::arena::pool::singleton_null()));
            }
        } else if let Some(rest) = path.strip_prefix("accumulator.")
            && let Some(acc) = frame.get_reduce_accumulator()
        {
            return Ok(arena_access_path_str_ref(acc, rest, arena)
                .unwrap_or_else(|| crate::arena::pool::singleton_null()));
        }
    }

    // Walk the path on current frame data.
    let cur = current_data_av(actx, arena);
    match arena_access_path_str_ref(cur, path, arena) {
        Some(av) => Ok(av),
        None => {
            if args.len() > 1 {
                engine.evaluate_node(&args[1], actx, arena)
            } else {
                Ok(crate::arena::pool::singleton_null())
            }
        }
    }
}

/// Read a `[level]` marker — the value-mode multi-arg `val` shape where
/// `args[0]` evaluates to a one-element numeric array. Returns the `i64`
/// level on a hit, `None` otherwise.
#[cfg(feature = "ext-control")]
#[inline]
fn level_marker_from_array(av: &DataValue<'_>) -> Option<i64> {
    match av {
        DataValue::Array(items) if !items.is_empty() => items[0].as_i64(),
        _ => None,
    }
}

/// Length of an arena array, or `None` if not array-shaped.
#[cfg(feature = "ext-control")]
#[inline]
fn arena_array_len(av: &DataValue<'_>) -> Option<usize> {
    match av {
        DataValue::Array(items) => Some(items.len()),
        _ => None,
    }
}

/// Get the i-th element of an arena array as a fresh `&'a DataValue<'a>`.
#[cfg(feature = "ext-control")]
#[inline]
fn arena_array_get<'a>(
    av: &'a DataValue<'a>,
    i: usize,
    _arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    match av {
        DataValue::Array(items) => items.get(i).map(|entry| {
            let av_ref: &'a DataValue<'a> = unsafe { &*(entry as *const DataValue<'a>) };
            av_ref
        }),
        _ => None,
    }
}

/// Arena-native `val` operator. Mirrors the value-mode shape (level access,
/// path chains, reduce shortcuts) but stays on `&DataValue` throughout.
#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_val_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    use crate::arena::context::ArenaContextRef;
    use crate::arena::value::{arena_access_path_str_ref, arena_apply_path_element};

    if args.is_empty() {
        return Ok(current_data_av(actx, arena));
    }

    // Multi-arg form: evaluate first to detect [[level], ...] vs path chain.
    if args.len() >= 2 {
        let first_av = engine.evaluate_node(&args[0], actx, arena)?;
        if let Some(level) = level_marker_from_array(first_av) {
            // Metadata short-circuits — only valid with exactly 2 args.
            if args.len() == 2 {
                let path_av = engine.evaluate_node(&args[1], actx, arena)?;
                let path_str = path_av.as_str().unwrap_or("");
                if path_str == "index"
                    && let Some(idx) = actx.current().get_index()
                {
                    return Ok(arena.alloc(DataValue::Number(
                        crate::value::NumberValue::Integer(idx as i64),
                    )));
                }
                if path_str == "key"
                    && let Some(key) = actx.current().get_key()
                {
                    let s: &'a str = arena.alloc_str(key);
                    return Ok(arena.alloc(DataValue::String(s)));
                }

                let path_owned = path_string_from_arena(path_av);
                let frame_av = frame_data_at_level(actx, level as isize, arena)
                    .ok_or(Error::InvalidContextLevel(level as isize))?;
                return Ok(arena_access_path_str_ref(frame_av, &path_owned, arena)
                    .unwrap_or_else(|| crate::arena::pool::singleton_null()));
            }

            // Multi-arg path chain at a relative level.
            let mut paths: Vec<String> = Vec::with_capacity(args.len() - 1);
            for item in args.iter().skip(1) {
                let av = engine.evaluate_node(item, actx, arena)?;
                paths.push(path_string_from_arena(av));
            }
            let mut cur = frame_data_at_level(actx, level as isize, arena)
                .ok_or(Error::InvalidContextLevel(level as isize))?;
            for path in &paths {
                match arena_access_path_str_ref(cur, path, arena) {
                    Some(next) => cur = next,
                    None => return Ok(crate::arena::pool::singleton_null()),
                }
            }
            return Ok(cur);
        }

        // Non-level multi-arg path chain: pre-eval all args.
        let mut evaluated: Vec<&'a DataValue<'a>> = Vec::with_capacity(args.len());
        evaluated.push(first_av);
        for arg in args.iter().skip(1) {
            evaluated.push(engine.evaluate_node(arg, actx, arena)?);
        }

        // Reduce shortcut for the first segment.
        let mut start: Option<&'a DataValue<'a>> = None;
        if let ArenaContextRef::Frame(frame) = actx.current()
            && let Some(s) = evaluated[0].as_str()
        {
            start = if s == "current" {
                frame.get_reduce_current()
            } else if s == "accumulator" {
                frame.get_reduce_accumulator()
            } else {
                None
            };
        }

        let (mut cur, rest_start) = match start {
            Some(s) => (s, 1),
            None => (current_data_av(actx, arena), 0),
        };
        for elem in &evaluated[rest_start..] {
            match arena_apply_path_element(cur, elem, arena) {
                Some(next) => cur = next,
                None => return Ok(crate::arena::pool::singleton_null()),
            }
        }
        return Ok(cur);
    }

    // Single-arg form: evaluate it.
    let path_av = engine.evaluate_node(&args[0], actx, arena)?;

    // Array argument: either [[level], path...] or a path chain.
    if let Some(arr_len) = arena_array_len(path_av) {
        if arr_len >= 2 {
            // Try the level form: first element is `[number, ...]`.
            let first_elem = arena_array_get(path_av, 0, arena);
            let level_opt = first_elem.and_then(|e| match e {
                DataValue::Array(level_arr) if !level_arr.is_empty() => level_arr[0].as_i64(),
                _ => None,
            });
            if let Some(level) = level_opt {
                if arr_len == 2 {
                    let second = arena_array_get(path_av, 1, arena)
                        .unwrap_or_else(|| crate::arena::pool::singleton_null());
                    let path_str = second.as_str().unwrap_or("");
                    if path_str == "index"
                        && let Some(idx) = actx.current().get_index()
                    {
                        return Ok(arena.alloc(DataValue::Number(
                            crate::value::NumberValue::Integer(idx as i64),
                        )));
                    }
                    if path_str == "key"
                        && let Some(key) = actx.current().get_key()
                    {
                        let s: &'a str = arena.alloc_str(key);
                        return Ok(arena.alloc(DataValue::String(s)));
                    }
                }

                let mut cur = frame_data_at_level(actx, level as isize, arena)
                    .ok_or(Error::InvalidContextLevel(level as isize))?;
                for i in 1..arr_len {
                    let item = arena_array_get(path_av, i, arena)
                        .unwrap_or_else(|| crate::arena::pool::singleton_null());
                    let Some(seg) = item.as_str() else {
                        return Ok(crate::arena::pool::singleton_null());
                    };
                    match arena_access_path_str_ref(cur, seg, arena) {
                        Some(next) => cur = next,
                        None => return Ok(crate::arena::pool::singleton_null()),
                    }
                }
                return Ok(cur);
            }
        }

        // Plain path-chain array.
        let mut cur = current_data_av(actx, arena);
        for i in 0..arr_len {
            let elem = arena_array_get(path_av, i, arena)
                .unwrap_or_else(|| crate::arena::pool::singleton_null());
            match arena_apply_path_element(cur, elem, arena) {
                Some(next) => cur = next,
                None => return Ok(crate::arena::pool::singleton_null()),
            }
        }
        return Ok(cur);
    }

    // String / number path on current data, with reduce shortcuts and the
    // "direct-key wins over dotted-path" rule from the value-mode val.
    if let Some(s) = path_av.as_str() {
        if let ArenaContextRef::Frame(frame) = actx.current() {
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
                    return Ok(arena_access_path_str_ref(cur, rest, arena)
                        .unwrap_or_else(|| crate::arena::pool::singleton_null()));
                }
            } else if let Some(rest) = s.strip_prefix("accumulator.")
                && let Some(acc) = frame.get_reduce_accumulator()
            {
                return Ok(arena_access_path_str_ref(acc, rest, arena)
                    .unwrap_or_else(|| crate::arena::pool::singleton_null()));
            }
        }

        let cur = current_data_av(actx, arena);
        // Direct object key lookup beats dot-path traversal so empty keys and
        // keys containing dots resolve correctly.
        if let DataValue::Object(pairs) = cur
            && let Some(av) = crate::arena::value::arena_object_lookup_field(pairs, s)
        {
            return Ok(av);
        }
        return Ok(arena_access_path_str_ref(cur, s, arena)
            .unwrap_or_else(|| crate::arena::pool::singleton_null()));
    }

    if let Some(i) = path_av.as_i64()
        && i >= 0
    {
        let cur = current_data_av(actx, arena);
        let key = i.to_string();
        return Ok(arena_access_path_str_ref(cur, &key, arena)
            .unwrap_or_else(|| crate::arena::pool::singleton_null()));
    }

    Ok(crate::arena::pool::singleton_null())
}

/// Test whether `key` exists on an arena Object. Matches the value-mode
/// `obj.contains_key` semantics — Null values still count as present.
#[cfg(feature = "ext-control")]
#[inline]
fn arena_object_contains(av: &DataValue<'_>, key: &str) -> bool {
    match av {
        DataValue::Object(pairs) => {
            crate::arena::value::arena_object_lookup_field(pairs, key).is_some()
        }
        _ => false,
    }
}

/// Step into an arena Object at `key`. Returns `None` for non-objects or
/// missing keys.
#[cfg(feature = "ext-control")]
#[inline]
fn arena_object_step<'a>(
    av: &'a DataValue<'a>,
    key: &str,
    _arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    match av {
        DataValue::Object(pairs) => crate::arena::value::arena_object_lookup_field(pairs, key),
        _ => None,
    }
}

/// Arena-native `exists` operator (raw form). Mirrors value-mode semantics:
/// only Object types resolve, the final segment is a `contains_key` probe so
/// keys with `null` values still report as present.
#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_exists_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_false());
    }

    let cur = current_data_av(actx, arena);

    if args.len() == 1 {
        let arg = engine.evaluate_node(&args[0], actx, arena)?;
        if let Some(s) = arg.as_str() {
            return Ok(crate::arena::pool::singleton_bool(arena_object_contains(
                cur, s,
            )));
        }
        if let Some(arr_len) = arena_array_len(arg) {
            if arr_len == 0 {
                return Ok(crate::arena::pool::singleton_false());
            }
            let mut walk = cur;
            for i in 0..arr_len {
                let elem = arena_array_get(arg, i, arena)
                    .unwrap_or_else(|| crate::arena::pool::singleton_null());
                let Some(seg) = elem.as_str() else {
                    return Ok(crate::arena::pool::singleton_false());
                };
                if i == arr_len - 1 {
                    return Ok(crate::arena::pool::singleton_bool(arena_object_contains(
                        walk, seg,
                    )));
                }
                match arena_object_step(walk, seg, arena) {
                    Some(next) => walk = next,
                    None => return Ok(crate::arena::pool::singleton_false()),
                }
            }
            return Ok(crate::arena::pool::singleton_true());
        }
        return Ok(crate::arena::pool::singleton_false());
    }

    // Multiple args — each must evaluate to a string segment.
    let mut paths: Vec<&'a DataValue<'a>> = Vec::with_capacity(args.len());
    for arg in args {
        let av = engine.evaluate_node(arg, actx, arena)?;
        if av.as_str().is_none() {
            return Ok(crate::arena::pool::singleton_false());
        }
        paths.push(av);
    }
    let mut walk = cur;
    let last = paths.len() - 1;
    for (i, av) in paths.iter().enumerate() {
        let seg = av.as_str().expect("checked above");
        if i == last {
            return Ok(crate::arena::pool::singleton_bool(arena_object_contains(
                walk, seg,
            )));
        }
        match arena_object_step(walk, seg, arena) {
            Some(next) => walk = next,
            None => return Ok(crate::arena::pool::singleton_false()),
        }
    }
    Ok(crate::arena::pool::singleton_true())
}

#[inline]
fn default_or_null_arena<'a>(
    default_value: Option<&'a CompiledNode>,
    actx: &mut DataContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match default_value {
        Some(node) => engine.evaluate_node(node, actx, arena),
        None => Ok(crate::arena::pool::singleton_null()),
    }
}
