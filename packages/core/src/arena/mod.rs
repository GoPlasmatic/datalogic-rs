//! Arena allocation infrastructure for zero-clone evaluation.
//!
//! The crate-root re-exports surface what users need: [`crate::DataValue`]
//! and [`crate::operator::ContextStack`]. They appear in
//! [`crate::CustomOperator::evaluate`] signatures and let users implement
//! custom operators that participate in arena dispatch without
//! materializing `serde_json::Value`. For the `DataValue → JSON String`
//! path use `value.to_string()` (driven by `datavalue`'s native `Display`
//! emitter).
//!
//! The arena is acquired and released within a single
//! [`crate::Engine::evaluate`] call; the [`crate::DataValue`] tree borrows
//! from a [`bumpalo::Bump`] plus the caller's input.

pub(crate) mod context;
pub(crate) mod singletons;
pub(crate) mod util;
pub(crate) mod value;

pub use context::ContextStack;
pub(crate) use context::IterGuard;
pub(crate) use util::bvec;
pub use value::DataValue;
pub(crate) use value::{
    coerce_to_number_cfg, data_to_str, truthy_arena, try_coerce_to_integer_cfg,
};
#[cfg(feature = "compat")]
pub(crate) use value::{data_to_value, value_to_data};
