//! Shared path-parsing helpers used by `compile_missing`, the var/val
//! compile paths, and other operators that pre-parse string paths at
//! compile time.

use crate::node::{PathSegment, ReduceHint};

/// Parse a dot-separated path into pre-parsed segments.
pub(super) fn parse_path_segments(path: &str) -> Vec<PathSegment> {
    if path.is_empty() {
        return Vec::new();
    }
    if !path.contains('.') {
        if let Ok(idx) = path.parse::<usize>() {
            return vec![PathSegment::FieldOrIndex(path.into(), idx)];
        }
        return vec![PathSegment::Field(path.into())];
    }
    path.split('.')
        .map(|part| {
            if let Ok(idx) = part.parse::<usize>() {
                PathSegment::FieldOrIndex(part.into(), idx)
            } else {
                PathSegment::Field(part.into())
            }
        })
        .collect()
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
