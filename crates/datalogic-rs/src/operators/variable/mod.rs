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
use crate::node::{MetadataHint, PathSegment, ReduceHint, ScopeBinding};
use crate::{CompiledNode, Result};

#[cfg(feature = "ext-control")]
mod exists;
mod val;

#[cfg(feature = "ext-control")]
pub(crate) use exists::{evaluate_exists, evaluate_exists_compiled};
pub(crate) use val::{evaluate_val, evaluate_val_compiled};

/// Resolve a `[level]` + metadata-hint path (`"index"` / `"key"`) for the
/// interpreted path, which only learns the path string at runtime. Used by
/// both the multi-arg and single-arg array branches of `evaluate_val` — and
/// it owns only the classification: the value comes from the same
/// [`val::resolve_metadata_hint`] the compiled path uses.
///
/// `None` means "not a metadata access at this level" — either the path is
/// not `index`/`key`, or the level names an element frame (an even, non-zero
/// level), where those are ordinary field names. A metadata access that finds
/// no frame or no slot is `Some(null)`, not a fall-through.
#[inline]
fn metadata_hint_lookup<'a>(
    ctx: &ContextStack<'a>,
    level: i64,
    path: &str,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let hint = MetadataHint::from_path(path);
    if hint == MetadataHint::None {
        return None;
    }
    // An even, non-zero level names an element frame, where `index` and `key`
    // are ordinary field names — not a metadata access.
    crate::arena::metadata_climb(level.unsigned_abs() as usize)?;
    Some(val::resolve_metadata_hint(hint, level as isize, ctx, arena))
}

/// Return the current frame's data as an `&'a DataValue<'a>`. Root and frame
/// branches both return their stored `&DataValue` directly — no per-call
/// allocation.
#[inline(always)]
fn current_data<'a>(ctx: &ContextStack<'a>) -> &'a DataValue<'a> {
    ctx.current().data()
}

/// Frame data at a given level (or `None` if the level walks past the root).
#[inline]
fn frame_data_at_level<'a>(ctx: &ContextStack<'a>, level: isize) -> Option<&'a DataValue<'a>> {
    Some(ctx.get_at_level(level)?.data())
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

/// Pre-compiled `var`/`val` lookup spec — the fields stored on
/// [`CompiledNode::Var`], bundled so the arena evaluator takes one
/// borrow instead of several loose params.
pub(crate) struct CompiledVarSpec<'n> {
    pub scope_level: u32,
    pub segments: &'n [PathSegment],
    pub reduce_hint: ReduceHint,
    pub metadata_hint: MetadataHint,
    pub default_value: Option<&'n CompiledNode>,
    /// Compile-time frame resolution from [`crate::compile::scope::resolve`].
    pub binding: ScopeBinding,
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

/// Debug-only oracle for [`crate::compile::scope::resolve`].
///
/// Recomputes the frame the *runtime* walk would land on, then asserts the
/// compile-time [`ScopeBinding`] predicts the same one — by pointer identity,
/// not value equality, since two distinct frames can hold equal data and that
/// would still be a resolution bug.
///
/// Compiled out in release (the `cfg!` test folds to a constant). In debug it
/// turns every test in the corpus — the whole conformance battery, the
/// property generators, the fuzz target — into a differential check of the
/// analysis against the walk it replaces.
///
/// [`ScopeBinding::Unresolved`] is skipped: those nodes deliberately keep the
/// runtime path. [`ScopeBinding::Ancestor`] resolves through the walk too, so
/// there is no second answer to compare it against; what the pass claims for
/// it is only that the clamp cannot fire, and that is what is asserted.
///
#[inline]
pub(super) fn debug_check_binding<'a>(
    binding: ScopeBinding,
    scope_level: u32,
    ctx: &ContextStack<'a>,
) {
    if !cfg!(debug_assertions) {
        return;
    }
    let predicted: *const DataValue<'a> = match binding {
        ScopeBinding::Unresolved => return,
        ScopeBinding::Root => ctx.root_input(),
        ScopeBinding::Current => ctx.current().data(),
        ScopeBinding::Ancestor => {
            debug_assert!(
                matches!(
                    crate::arena::frame_target(ctx.depth(), scope_level as usize),
                    crate::arena::FrameTarget::Ancestor(_)
                ),
                "Ancestor binding (level {scope_level}) at depth {} is not a strict \
                 interior frame",
                ctx.depth()
            );
            return;
        }
    };
    // The walk exactly as it stood before the pass existed.
    let actual: *const DataValue<'a> = if scope_level == 0 {
        ctx.current().data()
    } else {
        match ctx.get_at_level(scope_level as isize) {
            Some(r) => r.data(),
            None => return,
        }
    };
    debug_assert!(
        std::ptr::eq(predicted, actual),
        "static scope binding {binding:?} (level {scope_level}) diverged from \
         the runtime walk at depth {}",
        ctx.depth()
    );
}
