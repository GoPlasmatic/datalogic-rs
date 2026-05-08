//! Arena-mode variable access (`val` / `var` / `exists`).
//!
//! Both `var` and `val` operator names normalize to `OpCode::Val`
//! (see `OpCode::FromStr`); the var-specific arg shape (path + default
//! fallback) is collapsed at compile time by `try_compile_var`. The runtime
//! split lives here:
//!
//! - [`val`] — `evaluate_val` and the compiled fast path
//!   (`evaluate_val_compiled`), plus the four-stage resolution helpers.
//! - [`exists`] — `evaluate_exists` and `evaluate_exists_compiled`
//!   (gated on the `ext-control` feature).
//!
//! Helpers shared by both flows live at module level here.

use bumpalo::Bump;

use crate::arena::{ContextStack, DataValue};
use crate::node::{MetadataHint, PathSegment, ReduceHint};
use crate::{CompiledNode, Result};

#[cfg(feature = "ext-control")]
mod exists;
mod val;

#[cfg(feature = "ext-control")]
pub(crate) use exists::{evaluate_exists, evaluate_exists_compiled};
pub(crate) use val::{evaluate_val, evaluate_val_compiled};

/// Resolve a `[level]` + metadata-hint path (`"index"` / `"key"`) against
/// the current frame, returning the singleton/string handle if it matches.
/// Used by both the multi-arg and single-arg array branches of `evaluate_val`
/// — extracted so the branches don't drift.
#[inline]
fn metadata_hint_lookup<'a>(
    ctx: &ContextStack<'a>,
    path: &str,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    if path == "index" {
        let idx = ctx.current().get_index()?;
        let i = idx as i64;
        return Some(
            crate::arena::singletons::singleton_small_int(i).unwrap_or_else(|| {
                arena.alloc(DataValue::Number(datavalue::NumberValue::Integer(i)))
            }),
        );
    }
    if path == "key" {
        let key = ctx.current().get_key()?;
        return Some(arena.alloc(DataValue::String(key)));
    }
    None
}

/// Return the current frame's data as an `&'a DataValue<'a>`. Root and frame
/// branches both return their stored `&DataValue` directly — no per-call
/// allocation.
#[inline(always)]
fn current_data<'a>(ctx: &ContextStack<'a>, _arena: &'a Bump) -> &'a DataValue<'a> {
    use crate::arena::context::ContextRef;
    match ctx.current() {
        ContextRef::Frame(f) => f.data(),
        ContextRef::Root(av) => av,
    }
}

/// Frame data at a given level (or `None` if the level walks past the root).
#[inline]
fn frame_data_at_level<'a>(
    ctx: &ContextStack<'a>,
    level: isize,
    _arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    use crate::arena::context::ContextRef;
    let aref = ctx.get_at_level(level)?;
    Some(match aref {
        ContextRef::Frame(f) => f.data(),
        ContextRef::Root(av) => av,
    })
}

/// Coerce an evaluated arena value into a path `&str`. Strings already
/// resident in the arena are re-borrowed without copying; numeric paths
/// pay one `arena.alloc_str` per call. Single arena-allocating helper used
/// by every `val`/`exists` lookup site that needs a path string.
#[inline]
fn path_str_from_data<'a>(av: &'a DataValue<'a>, arena: &'a Bump) -> &'a str {
    if let Some(s) = av.as_str() {
        return s;
    }
    if let DataValue::Number(n) = av {
        return arena.alloc_str(&n.to_string());
    }
    ""
}

/// Pre-compiled `var`/`val` lookup spec — the five fields stored on
/// [`CompiledNode::Var`], bundled so the arena evaluator takes one
/// borrow instead of five loose params.
pub(crate) struct CompiledVarSpec<'n> {
    pub scope_level: u32,
    pub segments: &'n [PathSegment],
    pub reduce_hint: ReduceHint,
    pub metadata_hint: MetadataHint,
    pub default_value: Option<&'n CompiledNode>,
}

/// Read a `[level]` marker — the value-mode multi-arg `val` shape where
/// `args[0]` evaluates to a one-element numeric array. Returns the `i64`
/// level on a hit, `None` otherwise.
#[inline]
fn level_marker_from_array(av: &DataValue<'_>) -> Option<i64> {
    match av {
        DataValue::Array(items) if !items.is_empty() => items[0].as_i64(),
        _ => None,
    }
}

/// Length of an arena array, or `None` if not array-shaped.
#[inline]
fn array_len(av: &DataValue<'_>) -> Option<usize> {
    match av {
        DataValue::Array(items) => Some(items.len()),
        _ => None,
    }
}

/// Get the i-th element of an arena array.
///
/// Safe access: `items` is bound as `&&'a [DataValue<'a>]` by the pattern
/// (default-bind-by-ref), and `*items` copies the inner `&'a [...]` via
/// `&T: Copy` — the slice's `.get` then preserves the `'a` element lifetime.
#[inline]
fn array_get<'a>(av: &'a DataValue<'a>, i: usize) -> Option<&'a DataValue<'a>> {
    let DataValue::Array(items) = av else {
        return None;
    };
    items.get(i)
}

/// Resolve the var's `default_value` when the primary lookup misses, or
/// fall back to a null singleton.
#[inline]
fn default_or_null<'a>(
    default_value: Option<&'a CompiledNode>,
    ctx: &mut ContextStack<'a>,
    engine: &crate::Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match default_value {
        Some(node) => engine.dispatch_node(node, ctx, arena),
        None => Ok(crate::arena::singletons::singleton_null()),
    }
}
