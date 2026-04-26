use serde_json::Value;

use crate::node::{MetadataHint, PathSegment, ReduceHint};
use crate::{CompiledNode, Error, Result};

// =============================================================================
// Arena-mode variable access
// =============================================================================
//
// Arena variants for var / val / exists. The raw forms
// (`evaluate_var_arena` / `_val_arena` / `_exists_arena`) handle dynamic-path
// expressions natively against the arena context stack.

use crate::arena::{ArenaContextStack, ArenaValue};
use bumpalo::Bump;

/// Return the current frame's data as an `&'a ArenaValue<'a>`. Root and frame
/// branches both return their stored `&ArenaValue` directly — no per-call
/// allocation.
#[inline]
fn current_data_av<'a>(actx: &ArenaContextStack<'a>, _arena: &'a Bump) -> &'a ArenaValue<'a> {
    use crate::arena::context::ArenaContextRef;
    match actx.current() {
        ArenaContextRef::Frame(f) => f.data(),
        ArenaContextRef::Root(av) => av,
    }
}

/// Frame data at a given level (or `None` if the level walks past the root).
#[inline]
fn frame_data_at_level<'a>(
    actx: &ArenaContextStack<'a>,
    level: isize,
    _arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
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
fn path_string_from_arena(av: &ArenaValue<'_>) -> String {
    if let Some(s) = av.as_str() {
        return s.to_string();
    }
    if let ArenaValue::Number(n) = av {
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

/// Arena variant of `evaluate_compiled_var`. Re-borrows arena-resident input
/// for root-scope lookups; otherwise clones into the arena.
#[inline]
pub(crate) fn evaluate_compiled_var_arena<'a>(
    spec: CompiledVarSpec<'a>,
    actx: &mut ArenaContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let CompiledVarSpec {
        scope_level,
        segments,
        reduce_hint,
        metadata_hint,
        default_value,
    } = spec;
    // 1. Metadata hints from the arena iteration frame.
    match metadata_hint {
        MetadataHint::Index => {
            if let Some(idx) = actx.current().get_index() {
                return Ok(
                    arena.alloc(ArenaValue::Number(crate::value::NumberValue::Integer(
                        idx as i64,
                    ))),
                );
            }
        }
        MetadataHint::Key => {
            if let Some(key) = actx.current().get_key() {
                let s: &'a str = arena.alloc_str(key);
                return Ok(arena.alloc(ArenaValue::String(s)));
            }
        }
        MetadataHint::None => {}
    }

    // 2. Reduce-context hints — read from the arena reduce frame.
    if reduce_hint != ReduceHint::None && actx.depth() > 0 {
        use crate::arena::context::ArenaContextRef;
        if let ArenaContextRef::Frame(f) = actx.current() {
            let arena_reduce: Option<&'a ArenaValue<'a>> = match reduce_hint {
                ReduceHint::Current => f.get_reduce_current(),
                ReduceHint::Accumulator => f.get_reduce_accumulator(),
                ReduceHint::CurrentPath | ReduceHint::AccumulatorPath => None,
                ReduceHint::None => unreachable!(),
            };
            if let Some(av) = arena_reduce {
                return Ok(av);
            }
            // Path variants: traverse segments on the reduce slot.
            let path_av: Option<&'a ArenaValue<'a>> = match reduce_hint {
                ReduceHint::CurrentPath => f.get_reduce_current().and_then(|cur| {
                    crate::arena::value::arena_traverse_segments(cur, &segments[1..], arena)
                }),
                ReduceHint::AccumulatorPath => f.get_reduce_accumulator().and_then(|acc| {
                    crate::arena::value::arena_traverse_segments(acc, &segments[1..], arena)
                }),
                _ => None,
            };
            match (reduce_hint, path_av) {
                (ReduceHint::CurrentPath | ReduceHint::AccumulatorPath, Some(av)) => {
                    return Ok(av);
                }
                (ReduceHint::CurrentPath | ReduceHint::AccumulatorPath, None) => {
                    // Frame existed but path didn't resolve — return default.
                    return default_or_null_arena(default_value, actx, engine, arena);
                }
                _ => {}
            }
        }
    }

    // 3. Root-scope fast path: arena traversal directly on the root value.
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

    // 4. General path via the arena context stack.
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
    actx: &mut ArenaContextStack<'a>,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
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
    actx: &mut ArenaContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    use crate::arena::value::arena_access_path_str_ref;

    if args.is_empty() {
        return Ok(current_data_av(actx, arena));
    }

    // Resolve path string. Literal string/number is the common shape.
    let owned_path: String;
    let path: &str = match &args[0] {
        CompiledNode::Value {
            value: Value::String(s),
            ..
        } => s.as_str(),
        CompiledNode::Value {
            value: Value::Number(n),
            ..
        } => {
            owned_path = n.to_string();
            owned_path.as_str()
        }
        other => {
            let av = engine.evaluate_arena_node(other, actx, arena)?;
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
                engine.evaluate_arena_node(&args[1], actx, arena)
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
fn level_marker_from_array(av: &ArenaValue<'_>) -> Option<i64> {
    match av {
        ArenaValue::Array(items) if !items.is_empty() => items[0].as_i64(),
        _ => None,
    }
}

/// Length of an arena array, or `None` if not array-shaped.
#[cfg(feature = "ext-control")]
#[inline]
fn arena_array_len(av: &ArenaValue<'_>) -> Option<usize> {
    match av {
        ArenaValue::Array(items) => Some(items.len()),
        _ => None,
    }
}

/// Get the i-th element of an arena array as a fresh `&'a ArenaValue<'a>`.
#[cfg(feature = "ext-control")]
#[inline]
fn arena_array_get<'a>(
    av: &'a ArenaValue<'a>,
    i: usize,
    _arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    match av {
        ArenaValue::Array(items) => items.get(i).map(|entry| {
            let av_ref: &'a ArenaValue<'a> = unsafe { &*(entry as *const ArenaValue<'a>) };
            av_ref
        }),
        _ => None,
    }
}

/// Arena-native `val` operator. Mirrors the value-mode shape (level access,
/// path chains, reduce shortcuts) but stays on `&ArenaValue` throughout.
#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_val_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    use crate::arena::context::ArenaContextRef;
    use crate::arena::value::{arena_access_path_str_ref, arena_apply_path_element};

    if args.is_empty() {
        return Ok(current_data_av(actx, arena));
    }

    // Multi-arg form: evaluate first to detect [[level], ...] vs path chain.
    if args.len() >= 2 {
        let first_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
        if let Some(level) = level_marker_from_array(first_av) {
            // Metadata short-circuits — only valid with exactly 2 args.
            if args.len() == 2 {
                let path_av = engine.evaluate_arena_node(&args[1], actx, arena)?;
                let path_str = path_av.as_str().unwrap_or("");
                if path_str == "index"
                    && let Some(idx) = actx.current().get_index()
                {
                    return Ok(arena.alloc(ArenaValue::Number(
                        crate::value::NumberValue::Integer(idx as i64),
                    )));
                }
                if path_str == "key"
                    && let Some(key) = actx.current().get_key()
                {
                    let s: &'a str = arena.alloc_str(key);
                    return Ok(arena.alloc(ArenaValue::String(s)));
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
                let av = engine.evaluate_arena_node(item, actx, arena)?;
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
        let mut evaluated: Vec<&'a ArenaValue<'a>> = Vec::with_capacity(args.len());
        evaluated.push(first_av);
        for arg in args.iter().skip(1) {
            evaluated.push(engine.evaluate_arena_node(arg, actx, arena)?);
        }

        // Reduce shortcut for the first segment.
        let mut start: Option<&'a ArenaValue<'a>> = None;
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
    let path_av = engine.evaluate_arena_node(&args[0], actx, arena)?;

    // Array argument: either [[level], path...] or a path chain.
    if let Some(arr_len) = arena_array_len(path_av) {
        if arr_len >= 2 {
            // Try the level form: first element is `[number, ...]`.
            let first_elem = arena_array_get(path_av, 0, arena);
            let level_opt = first_elem.and_then(|e| match e {
                ArenaValue::Array(level_arr) if !level_arr.is_empty() => level_arr[0].as_i64(),
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
                        return Ok(arena.alloc(ArenaValue::Number(
                            crate::value::NumberValue::Integer(idx as i64),
                        )));
                    }
                    if path_str == "key"
                        && let Some(key) = actx.current().get_key()
                    {
                        let s: &'a str = arena.alloc_str(key);
                        return Ok(arena.alloc(ArenaValue::String(s)));
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
        if let ArenaValue::Object(pairs) = cur {
            for (k, v) in *pairs {
                if *k == s {
                    let av_ref: &'a ArenaValue<'a> = unsafe { &*(v as *const ArenaValue<'a>) };
                    return Ok(av_ref);
                }
            }
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
fn arena_object_contains(av: &ArenaValue<'_>, key: &str) -> bool {
    match av {
        ArenaValue::Object(pairs) => pairs.iter().any(|(k, _)| *k == key),
        _ => false,
    }
}

/// Step into an arena Object at `key`. Returns `None` for non-objects or
/// missing keys.
#[cfg(feature = "ext-control")]
#[inline]
fn arena_object_step<'a>(
    av: &'a ArenaValue<'a>,
    key: &str,
    _arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    match av {
        ArenaValue::Object(pairs) => {
            for (k, v) in *pairs {
                if *k == key {
                    let av_ref: &'a ArenaValue<'a> = unsafe { &*(v as *const ArenaValue<'a>) };
                    return Some(av_ref);
                }
            }
            None
        }
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
    actx: &mut ArenaContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_false());
    }

    let cur = current_data_av(actx, arena);

    if args.len() == 1 {
        let arg = engine.evaluate_arena_node(&args[0], actx, arena)?;
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
    let mut paths: Vec<&'a ArenaValue<'a>> = Vec::with_capacity(args.len());
    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
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
    actx: &mut ArenaContextStack<'a>,
    engine: &crate::DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    match default_value {
        Some(node) => engine.evaluate_arena_node(node, actx, arena),
        None => Ok(crate::arena::pool::singleton_null()),
    }
}
