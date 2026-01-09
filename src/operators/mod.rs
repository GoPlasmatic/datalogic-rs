//! Operator implementations for the DataLogic rule engine.
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
//! - **Special**: `preserve` - Structure preservation for templating
//!
//! # Dispatch Mechanism
//!
//! Operators are dispatched through the [`OpCode`](crate::OpCode) enum in `opcode.rs`.
//! During compilation, operator names are converted to `OpCode` variants for fast
//! runtime dispatch without string comparisons.

pub mod helpers;

pub mod abs;
pub mod arithmetic;
pub mod array;
pub mod ceil;
pub mod comparison;
pub mod control;
pub mod datetime;
pub mod floor;
pub mod logical;
pub mod missing;
pub mod preserve;
pub mod string;
pub mod string_ops;
pub mod throw;
pub mod try_op;
pub mod type_op;
pub mod variable;
