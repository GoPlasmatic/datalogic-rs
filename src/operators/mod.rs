//! Operator implementations for the Engine rule engine.
//!
//! This module contains all built-in operator implementations organized by category.
//! Each operator follows a consistent pattern: a function that takes compiled arguments,
//! a context stack, and the engine reference, returning a `Result<Value>`.
//!
//! # Operator Categories
//!
//! - **Variable Access**: `var`, `val`, `exists` - Access data from context
//! - **Comparison**: `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=` - Compare values
//! - **Logical**: `and`, `or`, `!`, `!!` - Boolean logic operations
//! - **Control Flow**: `if`, `?:`, `??` - Conditional evaluation
//! - **Arithmetic**: `+`, `-`, `*`, `/`, `%`, `min`, `max`, `abs`, `ceil`, `floor`
//! - **String**: `cat`, `substr`, `in`, `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`
//! - **Array**: `map`, `filter`, `reduce`, `merge`, `all`, `some`, `none`, `sort`, `slice`
//! - **DateTime**: `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now`
//! - **Error Handling**: `try`, `throw` - Exception-like error handling
//! - **Type**: `type` - Runtime type inspection
//! - **Missing**: `missing`, `missing_some` - Check for missing fields
//!
//! # Dispatch Mechanism
//!
//! Operators are dispatched through the [`OpCode`](crate::OpCode) enum in `opcode.rs`.
//! During compilation, operator names are converted to `OpCode` variants for fast
//! runtime dispatch without string comparisons.

pub(crate) mod helpers;

// Core - always compiled
pub(crate) mod arithmetic;
pub(crate) mod array;
pub(crate) mod comparison;
pub(crate) mod control;
pub(crate) mod logical;
pub(crate) mod missing;
pub(crate) mod string;
pub(crate) mod variable;

// Feature-gated extended operators
#[cfg(feature = "datetime")]
pub(crate) mod datetime;
#[cfg(feature = "error-handling")]
pub(crate) mod throw;
#[cfg(feature = "error-handling")]
pub(crate) mod try_op;
#[cfg(feature = "ext-control")]
pub(crate) mod type_op;
