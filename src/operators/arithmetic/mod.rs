//! Arithmetic operators for numeric computations.
//!
//! Submodules:
//! - [`basic`] — `+`, `-`, `*` with overflow promotion to `f64`.
//! - [`div_mod`] — `/` and `%` with config-aware divbyzero handling.
//! - [`min_max`] — `min` and `max` (array reduction + variadic).
//! - [`unary_math`] — `abs` / `ceil` / `floor` (gated on `ext-math`).
//! - [`datetime_arith`] — datetime/duration arithmetic (gated on `datetime`).
//! - [`helpers`] — shared NaN handling, coercion-pair, integer/float fold.
//!
//! ## Overflow handling
//!
//! All arithmetic operators use the same pattern for overflow protection:
//!
//! 1. **Track integer precision**: stay in `i64` while operands fit.
//! 2. **Checked arithmetic**: `checked_add`/`checked_mul` etc.
//! 3. **Overflow promotion**: on overflow, switch to `f64` and continue
//!    accumulating.
//! 4. **Result preservation**: return `i64` when possible, `f64` otherwise.
//!
//! The integer-checked-with-float-fallback pattern is centralised in
//! [`helpers::try_int_op`] for the 2-arg ops and in
//! [`helpers::arena_variadic_fold`] for variadic ops.
//!
//! ## NaN handling
//!
//! When a value cannot be coerced to a number, behavior depends on
//! `NanHandling` config: `ThrowError` (default), `IgnoreValue`,
//! `CoerceToZero`, or `ReturnNull`.

mod basic;
mod div_mod;
mod helpers;
mod min_max;

#[cfg(feature = "datetime")]
mod datetime_arith;
#[cfg(feature = "ext-math")]
mod unary_math;

pub(crate) use basic::{evaluate_add_arena, evaluate_multiply_arena, evaluate_subtract_arena};
pub(crate) use div_mod::{DivOp, arena_div_or_mod};
pub(crate) use min_max::{evaluate_max_arena, evaluate_min_arena};

#[cfg(feature = "ext-math")]
pub(crate) use unary_math::{UnaryMathOp, arena_unary_math};
