//! Internal arena allocation infrastructure (POC scope).
//!
//! This module is `pub(crate)` only — arena types never appear in public APIs.
//! The arena is acquired and released within a single `evaluate()` call; the
//! `ArenaValue` tree borrows from a `bumpalo::Bump` plus the caller's input
//! `Arc<Value>`. See `ARENA_RFC.md` for design rationale.

pub(crate) mod context;
pub(crate) mod pool;
pub(crate) mod value;

pub(crate) use context::ArenaContextStack;
pub(crate) use pool::ArenaGuard;
pub(crate) use value::{
    ArenaValue, arena_to_value, arena_to_value_cow, is_truthy_arena, to_string_arena,
    value_to_arena,
};
