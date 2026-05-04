//! # datalogic-rs
//!
//! A high-performance, thread-safe Rust implementation of JSONLogic.
//!
//! ## Overview
//!
//! `datalogic-rs` provides a powerful rule evaluation engine that compiles JSONLogic
//! expressions into optimized, reusable structures that can be evaluated across
//! multiple threads with zero overhead.
//!
//! ## Key Features
//!
//! - **Compilation-based optimization**: Parse once, evaluate many times
//! - **Thread-safe by design**: Share compiled logic across threads with `Arc`
//! - **50+ built-in operators**: Complete JSONLogic compatibility plus extensions
//! - **Arena-allocated evaluation**: Results live in a `bumpalo::Bump` arena and can borrow directly into caller input for zero-copy paths
//! - **Extensible**: Add custom operators via the [`DataOperator`] trait
//! - **Structured templates**: Preserve object structure for dynamic outputs
//!
//! ## Quick Start (one-shot)
//!
//! ```rust
//! use datalogic_rs::DataLogic;
//!
//! let engine = DataLogic::new();
//! let result = engine.evaluate_str(
//!     r#"{"==": [{"var": "status"}, "active"]}"#,
//!     r#"{"status": "active"}"#,
//! ).unwrap();
//! assert_eq!(result, "true");
//! ```
//!
//! ## Power-user (compile once, evaluate many)
//!
//! ```rust
//! use bumpalo::Bump;
//! use datalogic_rs::{DataLogic, DataValue};
//!
//! let engine = DataLogic::new();
//! let compiled = engine.compile(r#"{"==": [{"var": "status"}, "active"]}"#).unwrap();
//!
//! let arena = Bump::new();
//! let data = DataValue::from_str(r#"{"status": "active"}"#, &arena).unwrap();
//! let result = engine.evaluate(&compiled, arena.alloc(data), &arena).unwrap();
//! assert_eq!(result.as_bool(), Some(true));
//! ```
//!
//! ## Architecture
//!
//! The library uses a two-phase approach:
//!
//! 1. **Compilation**: JSON logic is parsed into `CompiledLogic` with OpCode dispatch
//! 2. **Evaluation**: Compiled logic is evaluated through arena dispatch — results
//!    are `&'a DataValue<'a>` allocated in a `bumpalo::Bump` for the duration of
//!    one evaluate call.
//!
//! This design enables sharing compiled logic across threads, eliminates
//! repeated parsing overhead, and lets read-through operations like `var`
//! return zero-copy borrows into the caller's input data.

pub mod arena;
mod builder;
#[cfg(feature = "compat")]
pub mod compat;
mod compile;
mod config;
mod constants;
#[cfg(feature = "datetime")]
mod datetime;
mod engine;
mod error;
mod node;
mod opcode;
mod operators;
#[cfg(feature = "trace")]
mod trace;
mod value;
mod value_helpers;

pub use arena::{DataContextStack, DataValue};
pub use builder::DataLogicBuilder;
pub use config::{
    DivisionByZeroHandling, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator,
};
#[cfg(feature = "datetime")]
pub use datavalue::{DataDateTime, DataDuration};
pub use datavalue::{NumberValue, OwnedDataValue};
pub use engine::DataLogic;
pub use error::{Error, StructuredError};
pub use node::CompiledLogic;
#[cfg(feature = "trace")]
pub use trace::{ExecutionStep, ExpressionNode, TraceCollector, TracedResult};

// `CompiledNode`, `OpCode`, `MetadataHint`, `PathSegment`, `ReduceHint`
// were public in 4.x. They are compile-internal types — `pub(crate)` for
// our own modules, surfaced via `crate::compat` for 4.x callers (with
// deprecation warnings).
#[allow(unused_imports)]
pub(crate) use node::{CompiledNode, MetadataHint, PathSegment, ReduceHint};
pub(crate) use opcode::OpCode;

/// Result type for DataLogic operations
pub type Result<T> = std::result::Result<T, Error>;

/// Custom operator for the DataLogic engine.
///
/// Implementations receive args **already evaluated** as borrowed
/// [`DataValue`] references and return a `&'a DataValue<'a>` result
/// allocated in the supplied [`bumpalo::Bump`] arena.
///
/// ## Lifetime
///
/// `'a` is the arena lifetime, tied to the [`bumpalo::Bump`] allocator
/// that lives for the duration of one [`DataLogic::evaluate_ref`] /
/// [`DataLogic::evaluate`] call. Args borrow from the caller's input and
/// from prior arena allocations; the returned `&'a DataValue<'a>` must
/// be allocated in the arena (or be a preallocated singleton) — never a
/// stack reference.
///
/// ## Example
///
/// ```rust
/// use datalogic_rs::{DataContextStack, DataOperator, DataValue, DataLogic, Result};
/// use bumpalo::Bump;
///
/// struct DoubleArena;
/// impl DataOperator for DoubleArena {
///     fn evaluate<'a>(
///         &self,
///         args: &[&'a DataValue<'a>],
///         _actx: &mut DataContextStack<'a>,
///         arena: &'a Bump,
///     ) -> Result<&'a DataValue<'a>> {
///         let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
///         Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
///     }
/// }
///
/// let mut engine = DataLogic::new();
/// engine.add_operator("double".into(), Box::new(DoubleArena));
///
/// let result = engine.evaluate_str(r#"{"double": 21}"#, "null").unwrap();
/// assert_eq!(result, "42");
/// ```
pub trait DataOperator: Send + Sync {
    /// Evaluate this operator with arena-allocated args and result.
    ///
    /// # Arguments
    ///
    /// * `args` — pre-evaluated args as `&'a DataValue<'a>`. The arena
    ///   dispatcher has already recursed into each arg's expression tree.
    /// * `actx` — the arena context stack. Most operators won't touch
    ///   this; it's needed only when the operator iterates and pushes
    ///   its own frames (analogous to `filter` / `map`).
    /// * `arena` — the [`bumpalo::Bump`] allocator. Use `arena.alloc(...)`
    ///   for arena values, `arena.alloc_str(...)` for strings.
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        actx: &mut DataContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>>;
}
