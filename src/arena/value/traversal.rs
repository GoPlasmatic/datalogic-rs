//! Path traversal on `DataValue`. Two pairs:
//! - **By segments** (`PathSegment` slice from compiled vars): preferred,
//!   no string parsing.
//! - **By dot-string**: legacy / dynamic paths.
//!
//! Each pair is dedupe'd via a private `_step` helper that does one
//! traversal step; the `_exists` variants are thin `.is_some()` wrappers
//! over the value-returning core.

use super::DataValue;
use super::lookup::arena_object_lookup_field;
use crate::node::PathSegment;
use bumpalo::Bump;

/// Reborrow an arena array entry up to the arena's `'a` lifetime. `slice.get(i)`
/// gives `Option<&'short DataValue<'a>>` — the cast restores `'a` on the outer
/// reference, which is sound because the slice is `&'a [DataValue<'a>]` and
/// nothing reallocates.
#[inline(always)]
unsafe fn reborrow_slice_entry<'a>(entry: &DataValue<'a>) -> &'a DataValue<'a> {
    unsafe { &*(entry as *const DataValue<'a>) }
}

/// Take one traversal step by `PathSegment`. Tight loop body — must always
/// inline so the cross-module callers in `variable.rs` see a flat walk.
#[inline(always)]
fn step_segment<'a>(cur: &'a DataValue<'a>, seg: &PathSegment) -> Option<&'a DataValue<'a>> {
    match (cur, seg) {
        (DataValue::Object(pairs), PathSegment::Field(key)) => {
            arena_object_lookup_field(pairs, key.as_ref())
        }
        (DataValue::Array(items), PathSegment::Index(idx)) => {
            items.get(*idx).map(|e| unsafe { reborrow_slice_entry(e) })
        }
        (DataValue::Object(pairs), PathSegment::FieldOrIndex(key, _)) => {
            arena_object_lookup_field(pairs, key.as_ref())
        }
        (DataValue::Array(items), PathSegment::FieldOrIndex(_, idx)) => {
            items.get(*idx).map(|e| unsafe { reborrow_slice_entry(e) })
        }
        _ => None,
    }
}

/// Take one traversal step by string segment (parses numeric segments as
/// array indices on the fly). Tight loop body — `inline(always)` for the
/// same reason as `step_segment`.
#[inline(always)]
fn step_str<'a>(cur: &'a DataValue<'a>, seg: &str) -> Option<&'a DataValue<'a>> {
    match cur {
        DataValue::Object(pairs) => arena_object_lookup_field(pairs, seg),
        DataValue::Array(items) => {
            let idx = seg.parse::<usize>().ok()?;
            items.get(idx).map(|e| unsafe { reborrow_slice_entry(e) })
        }
        _ => None,
    }
}

/// Walk path segments on an `&'a DataValue<'a>`. Used by variable-arena
/// lookups. Returns `None` if any segment misses or the value isn't
/// traversable.
#[inline(always)]
pub(crate) fn arena_traverse_segments<'a>(
    av: &'a DataValue<'a>,
    segments: &[PathSegment],
    _arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    // Single-segment fast path: 76% of paths in real workloads are length-1
    // (`{var: "x"}`-style). Avoids the loop's range bookkeeping and lets
    // step_segment + arena_object_lookup_field collapse into the caller.
    match segments.len() {
        0 => Some(av),
        1 => step_segment(av, &segments[0]),
        _ => {
            let mut cur = av;
            for seg in segments {
                cur = step_segment(cur, seg)?;
            }
            Some(cur)
        }
    }
}

/// Allocation-free segments-exists check. Companion of [`arena_traverse_segments`]
/// for compile-time-parsed paths where the leaf value isn't consumed.
#[inline(always)]
pub(crate) fn arena_path_exists_segments(av: &DataValue<'_>, segments: &[PathSegment]) -> bool {
    // Single-segment fast path mirrors `arena_traverse_segments`'s — the
    // dominant `missing` / `missing_some` shape is one-segment paths.
    match segments.len() {
        0 => true,
        1 => step_segment(
            // SAFETY: shrink the lifetime to this function's borrow; the
            // reference never escapes.
            unsafe { &*(av as *const DataValue<'_>) },
            &segments[0],
        )
        .is_some(),
        _ => {
            let mut cur: &DataValue<'_> = av;
            for seg in segments {
                match step_segment(
                    // SAFETY: shrink the inner lifetime to the outer borrow's
                    // lifetime; we never let the resulting reference escape.
                    unsafe { &*(cur as *const DataValue<'_>) },
                    seg,
                ) {
                    Some(next) => cur = next,
                    None => return false,
                }
            }
            true
        }
    }
}

/// Walk a dot-notation `path` on `&'a DataValue<'a>`.
#[inline]
pub(crate) fn arena_access_path_str_ref<'a>(
    av: &'a DataValue<'a>,
    path: &str,
    _arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    if path.is_empty() {
        return Some(av);
    }
    if !path.contains('.') {
        return step_str(av, path);
    }
    let mut cur = av;
    for seg in path.split('.') {
        cur = step_str(cur, seg)?;
    }
    Some(cur)
}

/// Allocation-free path-exists check on `&DataValue`. Used by `missing` /
/// `missing_some` where the leaf value isn't consumed.
#[inline]
pub(crate) fn arena_path_exists_str(av: &DataValue<'_>, path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    let mut cur: &DataValue<'_> = av;
    let walk = |cur: &mut &DataValue<'_>, seg: &str| -> bool {
        // SAFETY: identical to arena_path_exists_segments — never escape.
        match step_str(unsafe { &*(*cur as *const DataValue<'_>) }, seg) {
            Some(next) => {
                *cur = next;
                true
            }
            None => false,
        }
    };
    if !path.contains('.') {
        return walk(&mut cur, path);
    }
    for seg in path.split('.') {
        if !walk(&mut cur, seg) {
            return false;
        }
    }
    true
}

/// Apply a single evaluated path element (string field, numeric index) to an
/// arena value. Mirrors the (deleted) value-mode `apply_path_element_ref` for
/// the multi-arg `val` form where each arg is evaluated separately.
pub(crate) fn arena_apply_path_element<'a>(
    cur: &'a DataValue<'a>,
    elem: &DataValue<'_>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    if let Some(s) = elem.as_str() {
        return arena_access_path_str_ref(cur, s, arena);
    }
    if let Some(i) = elem.as_i64()
        && i >= 0
    {
        let idx = i as usize;
        return match cur {
            DataValue::Array(items) => items
                .get(idx)
                .map(|entry| unsafe { reborrow_slice_entry(entry) }),
            DataValue::Object(_) => arena_access_path_str_ref(cur, &i.to_string(), arena),
            _ => None,
        };
    }
    None
}
