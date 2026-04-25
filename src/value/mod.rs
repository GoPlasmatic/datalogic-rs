//! Internal value types for the arena evaluation path.
//!
//! These types are `pub(crate)` only — they never appear in public APIs.
//! See `ARENA_RFC.md` and the migration plan for context.

pub(crate) mod number;

pub(crate) use number::NumberValue;
