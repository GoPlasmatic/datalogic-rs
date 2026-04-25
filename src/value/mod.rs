//! Numeric type used by the arena evaluation path.
//!
//! [`NumberValue`] is also re-exported from the crate root so users
//! implementing [`crate::ArenaOperator`] can construct numeric results
//! without depending on this internal module path.

pub mod number;

pub use number::NumberValue;
