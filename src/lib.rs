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
//! - **Zero-copy operations**: Minimize allocations with `Cow` types
//! - **Extensible**: Add custom operators via the `Operator` trait
//! - **Structured templates**: Preserve object structure for dynamic outputs
//!
//! ## Quick Start
//!
//! ```rust
//! use datalogic_rs::DataLogic;
//! use serde_json::json;
//!
//! let engine = DataLogic::new();
//!
//! // Compile your logic once
//! let logic = json!({"==": [{"var": "status"}, "active"]});
//! let compiled = engine.compile(&logic).unwrap();
//!
//! // Evaluate with different data
//! let data = json!({"status": "active"});
//! let result = engine.evaluate_owned(&compiled, data).unwrap();
//! assert_eq!(result, json!(true));
//! ```
//!
//! ## Architecture
//!
//! The library uses a two-phase approach:
//!
//! 1. **Compilation**: JSON logic is parsed into `CompiledLogic` with OpCode dispatch
//! 2. **Evaluation**: Compiled logic is evaluated against data using direct dispatch
//!
//! This design enables sharing compiled logic across threads and eliminates
//! repeated parsing overhead.

pub mod arena;
mod compile;
mod config;
mod constants;
mod context;
#[cfg(feature = "datetime")]
mod datetime;
mod engine;
mod error;
mod node;
mod opcode;
mod operators;
#[cfg(feature = "trace")]
mod trace;
pub mod value;
mod value_helpers;

pub use arena::{ArenaContextStack, ArenaValue};
pub use config::{
    DivisionByZeroHandling, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator,
};
pub(crate) use context::ContextStack;
pub use engine::DataLogic;
pub use error::{Error, StructuredError};
pub use node::{CompiledLogic, CompiledNode, MetadataHint, PathSegment, ReduceHint};
pub use opcode::OpCode;
#[cfg(feature = "trace")]
pub use trace::{ExecutionStep, ExpressionNode, TraceCollector, TracedResult};
pub use value::NumberValue;

/// Result type for DataLogic operations
pub type Result<T> = std::result::Result<T, Error>;

/// Custom operator for the DataLogic engine.
///
/// Implementations receive args **already evaluated** as borrowed
/// [`ArenaValue`] references and return a `&'a ArenaValue<'a>` result
/// allocated in the supplied [`bumpalo::Bump`] arena.
///
/// ## Lifetime
///
/// `'a` is the arena lifetime, tied to the [`bumpalo::Bump`] allocator
/// that lives for the duration of one [`DataLogic::evaluate_ref`] /
/// [`DataLogic::evaluate`] call. Args borrow from the caller's input and
/// from prior arena allocations; the returned `&'a ArenaValue<'a>` must
/// be allocated in the arena (or be a preallocated singleton) — never a
/// stack reference.
///
/// ## Example
///
/// ```rust
/// use datalogic_rs::{ArenaContextStack, ArenaOperator, ArenaValue, DataLogic, Result};
/// use bumpalo::Bump;
/// use serde_json::json;
///
/// struct DoubleArena;
/// impl ArenaOperator for DoubleArena {
///     fn evaluate_arena<'a>(
///         &self,
///         args: &[&'a ArenaValue<'a>],
///         _actx: &mut ArenaContextStack<'a>,
///         arena: &'a Bump,
///     ) -> Result<&'a ArenaValue<'a>> {
///         let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
///         Ok(arena.alloc(ArenaValue::from_f64(n * 2.0)))
///     }
/// }
///
/// let mut engine = DataLogic::new();
/// engine.add_arena_operator("double".into(), Box::new(DoubleArena));
///
/// let logic = json!({"double": 21});
/// let compiled = engine.compile(&logic).unwrap();
/// let result = engine.evaluate_ref(&compiled, &json!({})).unwrap();
/// assert_eq!(result, json!(42));
/// ```
pub trait ArenaOperator: Send + Sync {
    /// Evaluate this operator with arena-allocated args and result.
    ///
    /// # Arguments
    ///
    /// * `args` — pre-evaluated args as `&'a ArenaValue<'a>`. The arena
    ///   dispatcher has already recursed into each arg's expression tree.
    /// * `actx` — the arena context stack. Most operators won't touch
    ///   this; it's needed only when the operator iterates and pushes
    ///   its own frames (analogous to `filter` / `map`).
    /// * `arena` — the [`bumpalo::Bump`] allocator. Use `arena.alloc(...)`
    ///   for arena values, `arena.alloc_str(...)` for strings.
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a ArenaValue<'a>],
        actx: &mut ArenaContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a ArenaValue<'a>>;
}
