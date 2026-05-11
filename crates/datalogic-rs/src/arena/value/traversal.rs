//! Path traversal on `DataValue`. Two pairs:
//! - **By segments** (`PathSegment` slice from compiled vars): preferred,
//!   no string parsing.
//! - **By dot-string**: legacy / dynamic paths.
//!
//! Each pair is dedupe'd via a private `_step` helper that does one
//! traversal step; the `_exists` variants are thin `.is_some()` wrappers
//! over the value-returning core.

use super::DataValue;
use super::lookup::object_lookup_field;
use crate::node::PathSegment;

/// Take one traversal step by `PathSegment`. Tight loop body — must always
/// inline so the cross-module callers in `variable.rs` see a flat walk.
#[inline(always)]
fn step_segment<'a>(cur: &'a DataValue<'a>, seg: &PathSegment) -> Option<&'a DataValue<'a>> {
    match (cur, seg) {
        (&DataValue::Object(pairs), PathSegment::Field(key)) => {
            object_lookup_field(pairs, key.as_ref())
        }
        (&DataValue::Array(items), PathSegment::Index(idx)) => items.get(*idx),
        (&DataValue::Object(pairs), PathSegment::FieldOrIndex(key, _)) => {
            object_lookup_field(pairs, key.as_ref())
        }
        (&DataValue::Array(items), PathSegment::FieldOrIndex(_, idx)) => items.get(*idx),
        _ => None,
    }
}

/// Take one traversal step by string segment (parses numeric segments as
/// array indices on the fly). Tight loop body — `inline(always)` for the
/// same reason as `step_segment`.
#[inline(always)]
fn step_str<'a>(cur: &'a DataValue<'a>, seg: &str) -> Option<&'a DataValue<'a>> {
    match *cur {
        DataValue::Object(pairs) => object_lookup_field(pairs, seg),
        DataValue::Array(items) => {
            let idx = seg.parse::<usize>().ok()?;
            items.get(idx)
        }
        _ => None,
    }
}

/// Walk path segments on an `&'a DataValue<'a>`. Used by variable-arena
/// lookups. Returns `None` if any segment misses or the value isn't
/// traversable.
#[inline(always)]
pub(crate) fn traverse_segments<'a>(
    av: &'a DataValue<'a>,
    segments: &[PathSegment],
) -> Option<&'a DataValue<'a>> {
    // Single-segment fast path: 76% of paths in real workloads are length-1
    // (`{var: "x"}`-style). Avoids the loop's range bookkeeping and lets
    // step_segment + object_lookup_field collapse into the caller.
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

/// Allocation-free segments-exists check. Companion of [`traverse_segments`]
/// for compile-time-parsed paths where the leaf value isn't consumed.
#[inline(always)]
pub(crate) fn path_exists_segments(av: &DataValue<'_>, segments: &[PathSegment]) -> bool {
    // Single-segment fast path mirrors `traverse_segments`'s — the
    // dominant `missing` / `missing_some` shape is one-segment paths.
    match segments.len() {
        0 => true,
        1 => step_segment(av, &segments[0]).is_some(),
        _ => {
            let mut cur: &DataValue<'_> = av;
            for seg in segments {
                match step_segment(cur, seg) {
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
pub(crate) fn access_path_str_ref<'a>(
    av: &'a DataValue<'a>,
    path: &str,
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
pub(crate) fn path_exists_str<'a>(av: &'a DataValue<'a>, path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    if !path.contains('.') {
        return step_str(av, path).is_some();
    }
    let mut cur: &'a DataValue<'a> = av;
    for seg in path.split('.') {
        match step_str(cur, seg) {
            Some(next) => cur = next,
            None => return false,
        }
    }
    true
}

/// Apply a single evaluated path element (string field, numeric index) to an
/// arena value. Mirrors the (deleted) value-mode `apply_path_element_ref` for
/// the multi-arg `val` form where each arg is evaluated separately.
pub(crate) fn apply_path_element<'a>(
    cur: &'a DataValue<'a>,
    elem: &DataValue<'_>,
) -> Option<&'a DataValue<'a>> {
    if let Some(s) = elem.as_str() {
        return access_path_str_ref(cur, s);
    }
    if let Some(i) = elem.as_i64() {
        if i >= 0 {
            let idx = i as usize;
            return match *cur {
                DataValue::Array(items) => items.get(idx),
                DataValue::Object(_) => access_path_str_ref(cur, &i.to_string()),
                _ => None,
            };
        }
    }
    None
}
