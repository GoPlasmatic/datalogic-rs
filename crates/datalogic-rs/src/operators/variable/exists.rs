//! Arena-mode `exists` evaluation.
//!
//! Mirrors value-mode semantics: only Object types resolve, the final segment
//! is a `contains_key` probe so keys whose value is `null` still report as
//! present. The whole module is gated on `feature = "ext-control"` via the
//! `mod exists;` declaration in the parent.

use super::{array_get, array_len, current_data};
use crate::Result;
use crate::arena::{ContextStack, DataValue};
use crate::node::PathSegment;
use crate::operators::eager::Cx;
use crate::operators::extract::{Any, RestArgs};

/// Arena variant of `evaluate_exists_compiled`. Always returns a Bool singleton.
#[inline]
pub(crate) fn evaluate_exists_compiled<'a>(
    scope_level: u32,
    segments: &[PathSegment],
    binding: crate::node::ScopeBinding,
    ctx: &mut ContextStack<'a>,
) -> Result<&'a DataValue<'a>> {
    // Cross-validate the compile-time resolution against the runtime walk.
    super::debug_check_binding(binding, scope_level, ctx);
    // Root binding: walk the input directly (no clone, no frame access).
    if binding == crate::node::ScopeBinding::Root {
        let found = segments.is_empty()
            || crate::arena::value::traverse_segments(ctx.root_input(), segments).is_some();
        return Ok(crate::arena::singletons::singleton_bool(found));
    }

    let aref = match binding {
        crate::node::ScopeBinding::Current => ctx.current(),
        // `Ancestor` still walks — the pass proves only that the clamp cannot
        // fire, it does not re-implement the walk. `Unresolved` keeps nodes
        // built outside the compile pipeline on today's path.
        _ => match ctx.get_at_level(scope_level as isize) {
            Some(f) => f,
            None => return Ok(crate::arena::singletons::singleton_false()),
        },
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
fn object_step<'a>(av: &'a DataValue<'a>, key: &str) -> Option<&'a DataValue<'a>> {
    match av {
        DataValue::Object(pairs) => crate::arena::value::object_lookup_field(pairs, key),
        _ => None,
    }
}

/// `exists`: whether the path is present in the current data. Mirrors
/// value-mode semantics: only Object types resolve, the final segment is a
/// `contains_key` probe so keys with `null` values still report as present.
///
/// One argument is a key or an array of segments; several are one segment
/// each, evaluated in order until one is not a string. The compile hook
/// turns literal paths into `CompiledNode::Exists`; this body runs for the
/// rest.
#[inline]
pub(crate) fn exists<'a>(
    cx: &mut Cx<'_, 'a>,
    first: &'a DataValue<'a>,
    rest: RestArgs<'a, Any>,
) -> Result<bool> {
    let cur = current_data(cx.ctx);

    if rest.is_empty() {
        if let Some(s) = first.as_str() {
            return Ok(object_contains(cur, s));
        }
        let Some(arr_len) = array_len(first) else {
            return Ok(false);
        };
        if arr_len == 0 {
            return Ok(false);
        }
        let mut walk = cur;
        for i in 0..arr_len {
            let elem =
                array_get(first, i).unwrap_or_else(|| crate::arena::singletons::singleton_null());
            let Some(seg) = elem.as_str() else {
                return Ok(false);
            };
            if i == arr_len - 1 {
                return Ok(object_contains(walk, seg));
            }
            match object_step(walk, seg) {
                Some(next) => walk = next,
                None => return Ok(false),
            }
        }
        return Ok(true);
    }

    // Several arguments: each must evaluate to a string segment. A
    // non-string one ends the call before later ones are evaluated.
    let Some(head) = first.as_str() else {
        return Ok(false);
    };
    let mut segments: bumpalo::collections::Vec<'a, &'a str> =
        bumpalo::collections::Vec::with_capacity_in(rest.len() + 1, cx.arena);
    segments.push(head);
    for i in 0..rest.len() {
        let Some(seg) = rest.get(i, cx)?.as_str() else {
            return Ok(false);
        };
        segments.push(seg);
    }
    let (last, init) = segments.split_last().expect("at least the head segment");
    let mut walk = cur;
    for seg in init {
        match object_step(walk, seg) {
            Some(next) => walk = next,
            None => return Ok(false),
        }
    }
    Ok(object_contains(walk, last))
}
