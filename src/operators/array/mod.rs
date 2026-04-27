//! Array operators: filter / map / reduce / merge / quantifiers / sort /
//! slice / length.
//!
//! Each operator lives in its own submodule. `helpers` carries the shared
//! infrastructure: `IterSrc`, `ResolvedInput`, `resolve_iter_input` (used by
//! every iterator op), `FastPredicate` (filter/quantifier fast paths), and
//! a few small comparison helpers.

mod helpers;

mod filter;
mod map;
mod merge;
mod quantifiers;
mod reduce;

#[cfg(feature = "ext-string")]
mod length;
#[cfg(feature = "ext-array")]
mod slice;
#[cfg(feature = "ext-array")]
mod sort;

// Operator entry points (consumed by the dispatcher).
pub(crate) use filter::evaluate_filter_arena;
pub(crate) use map::evaluate_map_arena;
pub(crate) use merge::evaluate_merge_arena;
pub(crate) use quantifiers::{evaluate_all_arena, evaluate_none_arena, evaluate_some_arena};
pub(crate) use reduce::evaluate_reduce_arena;

#[cfg(feature = "ext-string")]
pub(crate) use length::evaluate_length_arena;
#[cfg(feature = "ext-array")]
pub(crate) use slice::evaluate_slice_arena;
#[cfg(feature = "ext-array")]
pub(crate) use sort::evaluate_sort_arena;

// Iterator-input infrastructure consumed by `arithmetic` (and other crate
// callers) to compose with array results.
pub use helpers::FastPredicate;
pub(crate) use helpers::{IterArgKind, ResolvedInput, resolve_iter_input};
