//! Arena allocation infrastructure for zero-clone evaluation.
//!
//! Public types ([`ArenaValue`], [`ArenaContextStack`]) are also re-exported
//! at the crate root for ergonomics. They appear in
//! [`crate::ArenaOperator::evaluate_arena`] signatures and let users
//! implement custom operators that participate in arena dispatch without
//! materializing `serde_json::Value`.
//!
//! The arena is acquired and released within a single
//! [`crate::DataLogic::evaluate_ref`] / [`crate::DataLogic::evaluate`]
//! call; the [`ArenaValue`] tree borrows from a [`bumpalo::Bump`] plus the
//! caller's input `&Value`.

pub(crate) mod context;
pub(crate) mod pool;
pub(crate) mod value;

pub use context::ArenaContextStack;
pub(crate) use context::IterGuard;
pub(crate) use pool::ArenaGuard;
pub use value::{ArenaValue, value_to_arena};
pub(crate) use value::{
    arena_to_value, arena_to_value_cow, coerce_arena_to_number_cfg, is_truthy_arena,
    to_string_arena, try_coerce_arena_to_integer_cfg,
};

/// Allocate an empty `bumpalo::collections::Vec` with `cap` reserved slots.
/// Thin wrapper for the `Vec::with_capacity_in(cap, arena)` boilerplate that
/// appears throughout the operator code.
#[inline]
pub(crate) fn bvec<T>(arena: &bumpalo::Bump, cap: usize) -> bumpalo::collections::Vec<'_, T> {
    bumpalo::collections::Vec::with_capacity_in(cap, arena)
}
