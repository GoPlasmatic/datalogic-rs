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

pub mod context;
pub(crate) mod pool;
pub mod value;

pub use context::ArenaContextStack;
pub(crate) use pool::ArenaGuard;
pub use value::ArenaValue;
pub(crate) use value::{
    arena_to_value, arena_to_value_cow, is_truthy_arena, to_string_arena, value_to_arena,
};
