//! Shared path-parsing helpers used by `compile_missing`, the var/val
//! compile paths, and other operators that pre-parse string paths at
//! compile time.

use crate::node::{PathSegment, ReduceHint};

/// Convert a single path component into a [`PathSegment`]: a component that
/// parses as `usize` becomes `FieldOrIndex` (usable as an array index or an
/// object key), everything else becomes `Field`.
#[inline]
pub(super) fn str_to_segment(s: &str) -> PathSegment {
    if let Ok(idx) = s.parse::<usize>() {
        PathSegment::FieldOrIndex(s.into(), idx)
    } else {
        PathSegment::Field(s.into())
    }
}

/// Parse a dot-separated path into pre-parsed segments.
pub(super) fn parse_path_segments(path: &str) -> Vec<PathSegment> {
    if path.is_empty() {
        return Vec::new();
    }
    if !path.contains('.') {
        return vec![str_to_segment(path)];
    }
    path.split('.').map(str_to_segment).collect()
}

/// Parse a var path and determine the reduce hint. Recognises the special
/// `current` / `accumulator` prefixes used by the reduce body fast paths.
pub(super) fn parse_var_path(path: &str) -> (ReduceHint, Vec<PathSegment>) {
    if path == "current" {
        (
            ReduceHint::Current,
            vec![PathSegment::Field("current".into())],
        )
    } else if path == "accumulator" {
        (
            ReduceHint::Accumulator,
            vec![PathSegment::Field("accumulator".into())],
        )
    } else if let Some(rest) = path.strip_prefix("current.") {
        let mut segs = vec![PathSegment::Field("current".into())];
        segs.extend(parse_path_segments(rest));
        (ReduceHint::CurrentPath, segs)
    } else if let Some(rest) = path.strip_prefix("accumulator.") {
        let mut segs = vec![PathSegment::Field("accumulator".into())];
        segs.extend(parse_path_segments(rest));
        (ReduceHint::AccumulatorPath, segs)
    } else {
        (ReduceHint::None, parse_path_segments(path))
    }
}
