//! Path traversal on `ArenaValue`. Two pairs:
//! - **By segments** (`PathSegment` slice from compiled vars): preferred,
//!   no string parsing.
//! - **By dot-string**: legacy / dynamic paths.
//!
//! Each pair is dedupe'd via a private `_step` helper that does one
//! traversal step; the `_exists` variants are thin `.is_some()` wrappers
//! over the value-returning core.

use super::ArenaValue;
use super::lookup::arena_object_lookup_field;
use crate::node::PathSegment;
use bumpalo::Bump;

/// Reborrow an arena array entry up to the arena's `'a` lifetime. `slice.get(i)`
/// gives `Option<&'short ArenaValue<'a>>` — the cast restores `'a` on the outer
/// reference, which is sound because the slice is `&'a [ArenaValue<'a>]` and
/// nothing reallocates.
#[inline(always)]
unsafe fn reborrow_slice_entry<'a>(entry: &ArenaValue<'a>) -> &'a ArenaValue<'a> {
    unsafe { &*(entry as *const ArenaValue<'a>) }
}

/// Take one traversal step by `PathSegment`. Inlined into both segment-walking
/// helpers below.
#[inline]
fn step_segment<'a>(cur: &'a ArenaValue<'a>, seg: &PathSegment) -> Option<&'a ArenaValue<'a>> {
    match (cur, seg) {
        (ArenaValue::Object(pairs), PathSegment::Field(key)) => {
            arena_object_lookup_field(pairs, key.as_ref())
        }
        (ArenaValue::Array(items), PathSegment::Index(idx)) => {
            items.get(*idx).map(|e| unsafe { reborrow_slice_entry(e) })
        }
        (ArenaValue::Object(pairs), PathSegment::FieldOrIndex(key, _)) => {
            arena_object_lookup_field(pairs, key.as_ref())
        }
        (ArenaValue::Array(items), PathSegment::FieldOrIndex(_, idx)) => {
            items.get(*idx).map(|e| unsafe { reborrow_slice_entry(e) })
        }
        _ => None,
    }
}

/// Take one traversal step by string segment (parses numeric segments as
/// array indices on the fly).
#[inline]
fn step_str<'a>(cur: &'a ArenaValue<'a>, seg: &str) -> Option<&'a ArenaValue<'a>> {
    match cur {
        ArenaValue::Object(pairs) => arena_object_lookup_field(pairs, seg),
        ArenaValue::Array(items) => {
            let idx = seg.parse::<usize>().ok()?;
            items.get(idx).map(|e| unsafe { reborrow_slice_entry(e) })
        }
        _ => None,
    }
}

/// Walk path segments on an `&'a ArenaValue<'a>`. Used by variable-arena
/// lookups. Returns `None` if any segment misses or the value isn't
/// traversable.
pub(crate) fn arena_traverse_segments<'a>(
    av: &'a ArenaValue<'a>,
    segments: &[PathSegment],
    _arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    if segments.is_empty() {
        return Some(av);
    }
    let mut cur = av;
    for seg in segments {
        cur = step_segment(cur, seg)?;
    }
    Some(cur)
}

/// Allocation-free segments-exists check. Companion of [`arena_traverse_segments`]
/// for compile-time-parsed paths where the leaf value isn't consumed.
pub(crate) fn arena_path_exists_segments(av: &ArenaValue<'_>, segments: &[PathSegment]) -> bool {
    if segments.is_empty() {
        return true;
    }
    // Re-bind to a `&'a ArenaValue<'a>`-shaped reference so we can reuse
    // `step_segment`'s lifetime contract. The lifetimes coincide for the
    // duration of this function — we never return a reference.
    let mut cur: &ArenaValue<'_> = av;
    for seg in segments {
        match step_segment(
            // SAFETY: shrink the inner lifetime to the outer borrow's
            // lifetime; we never let the resulting reference escape.
            unsafe { &*(cur as *const ArenaValue<'_>) },
            seg,
        ) {
            Some(next) => cur = next,
            None => return false,
        }
    }
    true
}

/// Walk a dot-notation `path` on `&'a ArenaValue<'a>`.
pub(crate) fn arena_access_path_str_ref<'a>(
    av: &'a ArenaValue<'a>,
    path: &str,
    _arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
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

/// Allocation-free path-exists check on `&ArenaValue`. Used by `missing` /
/// `missing_some` where the leaf value isn't consumed.
pub(crate) fn arena_path_exists_str(av: &ArenaValue<'_>, path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    let mut cur: &ArenaValue<'_> = av;
    let walk = |cur: &mut &ArenaValue<'_>, seg: &str| -> bool {
        // SAFETY: identical to arena_path_exists_segments — never escape.
        match step_str(unsafe { &*(*cur as *const ArenaValue<'_>) }, seg) {
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
#[cfg(feature = "ext-control")]
pub(crate) fn arena_apply_path_element<'a>(
    cur: &'a ArenaValue<'a>,
    elem: &ArenaValue<'_>,
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    if let Some(s) = elem.as_str() {
        return arena_access_path_str_ref(cur, s, arena);
    }
    if let Some(i) = elem.as_i64()
        && i >= 0
    {
        let idx = i as usize;
        return match cur {
            ArenaValue::Array(items) => items
                .get(idx)
                .map(|entry| unsafe { reborrow_slice_entry(entry) }),
            ArenaValue::Object(_) => arena_access_path_str_ref(cur, &i.to_string(), arena),
            _ => None,
        };
    }
    None
}
