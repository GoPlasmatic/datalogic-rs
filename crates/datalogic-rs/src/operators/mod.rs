//! Operator implementations for the Engine rule engine.
//!
//! This module contains all built-in operator implementations organized by category.
//! Each operator follows a consistent pattern: a function that takes compiled arguments,
//! a context stack, and the engine reference, returning a `Result<Value>`.
//!
//! # Operator → required feature
//!
//! The default build (`features = []`) carries the JSONLogic baseline.
//! Extra operators live behind opt-in features; rules that use them
//! against an engine compiled without the feature error out at compile
//! time as `InvalidOperator("…")`.
//!
//! | Operator(s) | Required feature |
//! |---|---|
//! | `var`, `val` | *baseline* (always available) |
//! | `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=` | *baseline* |
//! | `and`, `or`, `!`, `!!`, `if`, `?:` | *baseline* |
//! | `+`, `-`, `*`, `/`, `%`, `min`, `max` | *baseline* |
//! | `cat`, `substr`, `in` | *baseline* |
//! | `map`, `filter`, `reduce`, `merge`, `all`, `some`, `none` | *baseline* |
//! | `missing`, `missing_some` | *baseline* |
//! | `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split` | `ext-string` |
//! | `sort`, `slice` | `ext-array` |
//! | `abs`, `ceil`, `floor` | `ext-math` |
//! | `exists`, `??`, `switch`/`match`, `type` | `ext-control` |
//! | `try`, `throw` | `error-handling` |
//! | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` | `datetime` |
//! | `fractional`, `sem_ver` ([flagd-compat][flagd]) | `flagd` |
//!
//! [flagd]: https://flagd.dev/reference/custom-operations/
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
//! - **flagd-compat**: `fractional`, `sem_ver` — feature-flagging operators
//!   from the [OpenFeature flagd in-process provider
//!   spec](https://flagd.dev/reference/custom-operations/), implemented to
//!   match the canonical Go evaluator byte-for-byte. Gated on `flagd`.
//!
//! # Dispatch Mechanism
//!
//! Operators are dispatched through the [`OpCode`](crate::OpCode) enum in `opcode.rs`.
//! During compilation, operator names are converted to `OpCode` variants for fast
//! runtime dispatch without string comparisons.

pub(crate) mod truthy;

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
pub(crate) mod error_handling;
#[cfg(feature = "flagd")]
pub(crate) mod flagd;
#[cfg(feature = "ext-control")]
pub(crate) mod inspect;
