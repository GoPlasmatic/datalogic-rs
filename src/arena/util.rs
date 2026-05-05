//! Small allocation/iteration helpers used by the operator code.

/// Allocate an empty `bumpalo::collections::Vec` with `cap` reserved slots.
/// Thin wrapper for the `Vec::with_capacity_in(cap, arena)` boilerplate that
/// appears throughout the operator code.
#[inline]
pub(crate) fn bvec<T>(arena: &bumpalo::Bump, cap: usize) -> bumpalo::collections::Vec<'_, T> {
    bumpalo::collections::Vec::with_capacity_in(cap, arena)
}
