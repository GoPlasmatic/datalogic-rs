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

/// Stringified small integers, indexed by their value. Returned as
/// `&'static str` from [`small_int_str`] so common small-index numeric
/// path segments (the dominant case for array indexing) skip the
/// per-call `arena.alloc_str` + heap `String` round trip.
#[rustfmt::skip]
static SMALL_INT_STRS: [&str; 100] = [
    "0",  "1",  "2",  "3",  "4",  "5",  "6",  "7",  "8",  "9",
    "10", "11", "12", "13", "14", "15", "16", "17", "18", "19",
    "20", "21", "22", "23", "24", "25", "26", "27", "28", "29",
    "30", "31", "32", "33", "34", "35", "36", "37", "38", "39",
    "40", "41", "42", "43", "44", "45", "46", "47", "48", "49",
    "50", "51", "52", "53", "54", "55", "56", "57", "58", "59",
    "60", "61", "62", "63", "64", "65", "66", "67", "68", "69",
    "70", "71", "72", "73", "74", "75", "76", "77", "78", "79",
    "80", "81", "82", "83", "84", "85", "86", "87", "88", "89",
    "90", "91", "92", "93", "94", "95", "96", "97", "98", "99",
];

/// Static-string lookup for small integer path segments. Returns
/// `Some(&'static str)` when `i` is in `0..100` (the dominant range for
/// array indices in real workloads), `None` otherwise — callers fall
/// back to per-call stringification for larger values.
#[inline]
pub(super) fn small_int_str(i: i64) -> Option<&'static str> {
    if (0..100).contains(&i) {
        Some(SMALL_INT_STRS[i as usize])
    } else {
        None
    }
}

/// Coerce an evaluated arena value into a path `&str`. Strings already
/// resident in the arena are re-borrowed without copying; integer paths
/// in `0..100` return a `&'static str` from [`small_int_str`]; everything
/// else pays one `arena.alloc_str` per call. Single arena-allocating
/// helper used by every `val`/`exists` lookup site that needs a path
/// string.
#[inline]
fn path_str_from_data<'a>(av: &'a DataValue<'a>, arena: &'a Bump) -> &'a str {
    if let Some(s) = av.as_str() {
        return s;
    }
    if let DataValue::Number(n) = av {
        if let Some(i) = n.as_i64()
            && let Some(s) = small_int_str(i)
        {
            return s;
        }
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
