//! Arena allocation infrastructure for zero-clone evaluation.
//!
//! The crate-root re-exports surface what users need: [`crate::DataValue`],
//! [`crate::ContextStack`], and [`crate::data_to_json_string`]. They appear
//! in [`crate::Operator::evaluate`] signatures and let users implement custom
//! operators that participate in arena dispatch without materializing
//! `serde_json::Value`.
//!
//! The arena is acquired and released within a single
//! [`crate::Engine::evaluate`] call; the [`crate::DataValue`] tree borrows
//! from a [`bumpalo::Bump`] plus the caller's input.

pub(crate) mod context;
pub(crate) mod pool;
pub(crate) mod value;

pub use context::ContextStack;
pub(crate) use context::IterGuard;
pub use value::{DataValue, data_to_json_string};
#[cfg(feature = "compat")]
pub(crate) use value::{data_to_value, value_to_data};
pub(crate) use value::{
    coerce_to_number_cfg, data_to_str, truthy_arena, try_coerce_to_integer_cfg,
};

/// Allocate an empty `bumpalo::collections::Vec` with `cap` reserved slots.
/// Thin wrapper for the `Vec::with_capacity_in(cap, arena)` boilerplate that
/// appears throughout the operator code.
#[inline]
pub(crate) fn bvec<T>(arena: &bumpalo::Bump, cap: usize) -> bumpalo::collections::Vec<'_, T> {
    bumpalo::collections::Vec::with_capacity_in(cap, arena)
}
