//! Arena allocation infrastructure for zero-clone evaluation.
//!
//! Public types ([`DataValue`], [`DataContextStack`]) are also re-exported
//! at the crate root for ergonomics. They appear in
//! [`crate::DataOperator::evaluate`] signatures and let users
//! implement custom operators that participate in arena dispatch without
//! materializing `serde_json::Value`.
//!
//! The arena is acquired and released within a single
//! [`crate::DataLogic::evaluate_ref`] / [`crate::DataLogic::evaluate`]
//! call; the [`DataValue`] tree borrows from a [`bumpalo::Bump`] plus the
//! caller's input `&Value`.

pub(crate) mod context;
pub(crate) mod pool;
pub(crate) mod value;

pub use context::DataContextStack;
pub(crate) use context::IterGuard;
#[cfg(feature = "compat")]
#[allow(unused_imports)]
pub(crate) use pool::ArenaGuard;
pub use value::DataValue;
#[cfg(feature = "compat")]
pub(crate) use value::arena_to_value;
#[cfg(feature = "compat")]
pub use value::value_to_arena;
pub(crate) use value::{
    coerce_arena_to_number_cfg, data_to_json_string, is_truthy_arena, to_string_arena,
    try_coerce_arena_to_integer_cfg,
};

/// Allocate an empty `bumpalo::collections::Vec` with `cap` reserved slots.
/// Thin wrapper for the `Vec::with_capacity_in(cap, arena)` boilerplate that
/// appears throughout the operator code.
#[inline]
pub(crate) fn bvec<T>(arena: &bumpalo::Bump, cap: usize) -> bumpalo::collections::Vec<'_, T> {
    bumpalo::collections::Vec::with_capacity_in(cap, arena)
}
