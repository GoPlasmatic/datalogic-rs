//! Arena-mode `exists` evaluation.
//!
//! Mirrors value-mode semantics: only Object types resolve, the final segment
//! is a `contains_key` probe so keys whose value is `null` still report as
//! present. The whole module is gated on `feature = "ext-control"` via the
//! `mod exists;` declaration in the parent.

use bumpalo::Bump;

use super::{array_get, array_len, current_data};
use crate::Result;
use crate::arena::{ContextStack, DataValue};
use crate::node::PathSegment;

/// Arena variant of `evaluate_exists_compiled`. Always returns a Bool singleton.
#[inline]
pub(crate) fn evaluate_exists_compiled<'a>(
    scope_level: u32,
    segments: &[PathSegment],
    ctx: &mut ContextStack<'a>,
) -> Result<&'a DataValue<'a>> {
    // Root scope at depth 0: walk input directly (no clone, no frame access).
    if scope_level == 0 && ctx.depth() == 0 {
        let found = segments.is_empty()
            || crate::arena::value::traverse_segments(ctx.root_input(), segments).is_some();
        return Ok(crate::arena::singletons::singleton_bool(found));
    }

    let aref = if scope_level == 0 {
        ctx.current()
    } else {
        match ctx.get_at_level(scope_level as isize) {
            Some(f) => f,
            None => return Ok(crate::arena::singletons::singleton_false()),
        }
    };
    let av = aref.data();
    let found =
        segments.is_empty() || crate::arena::value::traverse_segments(av, segments).is_some();
    Ok(crate::arena::singletons::singleton_bool(found))
}

/// Test whether `key` exists on an arena Object. Matches the value-mode
/// `obj.contains_key` semantics — Null values still count as present.
#[inline]
fn object_contains(av: &DataValue<'_>, key: &str) -> bool {
    match av {
        DataValue::Object(pairs) => crate::arena::value::object_lookup_field(pairs, key).is_some(),
        _ => false,
    }
}

/// Step into an arena Object at `key`. Returns `None` for non-objects or
/// missing keys.
#[inline]
fn object_step<'a>(
    av: &'a DataValue<'a>,
    key: &str,
    _arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    match av {
        DataValue::Object(pairs) => crate::arena::value::object_lookup_field(pairs, key),
        _ => None,
    }
}

/// Arena-native `exists` operator (raw form). Mirrors value-mode semantics:
/// only Object types resolve, the final segment is a `contains_key` probe so
/// keys with `null` values still report as present.
#[inline]
pub(crate) fn evaluate_exists<'a>(
    args: &'a [crate::CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &crate::Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::singletons::singleton_false());
    }

    let cur = current_data(ctx, arena);

    if args.len() == 1 {
        let arg = engine.dispatch_node(&args[0], ctx, arena)?;
        if let Some(s) = arg.as_str() {
            return Ok(crate::arena::singletons::singleton_bool(object_contains(
                cur, s,
            )));
        }
        if let Some(arr_len) = array_len(arg) {
            if arr_len == 0 {
                return Ok(crate::arena::singletons::singleton_false());
            }
            let mut walk = cur;
            for i in 0..arr_len {
                let elem =
                    array_get(arg, i).unwrap_or_else(|| crate::arena::singletons::singleton_null());
                let Some(seg) = elem.as_str() else {
                    return Ok(crate::arena::singletons::singleton_false());
                };
                if i == arr_len - 1 {
                    return Ok(crate::arena::singletons::singleton_bool(object_contains(
                        walk, seg,
                    )));
                }
                match object_step(walk, seg, arena) {
                    Some(next) => walk = next,
                    None => return Ok(crate::arena::singletons::singleton_false()),
                }
            }
            return Ok(crate::arena::singletons::singleton_true());
        }
        return Ok(crate::arena::singletons::singleton_false());
    }

    // Multiple args — each must evaluate to a string segment.
    let mut paths: bumpalo::collections::Vec<'a, &'a DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(args.len(), arena);
    for arg in args {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        if av.as_str().is_none() {
            return Ok(crate::arena::singletons::singleton_false());
        }
        paths.push(av);
    }
    let mut walk = cur;
    let last = paths.len() - 1;
    for (i, av) in paths.iter().enumerate() {
        let seg = av.as_str().expect("checked above");
        if i == last {
            return Ok(crate::arena::singletons::singleton_bool(object_contains(
                walk, seg,
            )));
        }
        match object_step(walk, seg, arena) {
            Some(next) => walk = next,
            None => return Ok(crate::arena::singletons::singleton_false()),
        }
    }
    Ok(crate::arena::singletons::singleton_true())
}
