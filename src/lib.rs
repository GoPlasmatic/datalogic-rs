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
//! - **Extensible**: Add custom operators via the [`Operator`] trait
//! - **Structured templates**: Preserve object structure for dynamic outputs
//!
//! ## Quick Start (one-shot)
//!
//! ```rust
//! use datalogic_rs::Engine;
//!
//! let engine = Engine::new();
//! let result = engine.evaluate_str(
//!     r#"{"==": [{"var": "status"}, "active"]}"#,
//!     r#"{"status": "active"}"#,
//! ).unwrap();
//! assert_eq!(result, "true");
//! ```
//!
//! ## Reusing the arena across many evaluations
//!
//! For high-throughput callers, open a [`Scratch`] handle. It owns a
//! [`bumpalo::Bump`], resets it between calls, and returns owned results so
//! you don't have to juggle arena lifetimes:
//!
//! ```rust
//! use datalogic_rs::Engine;
//!
//! let engine = Engine::new();
//! let compiled = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
//! let mut scratch = engine.scratch();
//!
//! for x in 0..3 {
//!     let payload = format!(r#"{{"x": {}}}"#, x);
//!     let result = scratch.eval_str(&compiled, &payload).unwrap();
//!     assert_eq!(result, (x + 1).to_string());
//! }
//! ```
//!
//! ## Power-user (compile once, evaluate many, zero-copy results)
//!
//! When the result borrow can stay scoped to a caller-managed
//! [`bumpalo::Bump`], skip the deep-clone and use [`Engine::evaluate`]
//! directly. `evaluate` accepts any input shape via [`IntoEvalData`]:
//! `&str`, `&OwnedDataValue`, `&serde_json::Value`, an owned `DataValue<'a>`,
//! or an existing `&'a DataValue<'a>`.
//!
//! ```rust
//! use bumpalo::Bump;
//! use datalogic_rs::Engine;
//!
//! let engine = Engine::new();
//! let compiled = engine.compile(r#"{"==": [{"var": "status"}, "active"]}"#).unwrap();
//!
//! let arena = Bump::new();
//! let result = engine.evaluate(&compiled, r#"{"status": "active"}"#, &arena).unwrap();
//! assert_eq!(result.as_bool(), Some(true));
//! ```
//!
//! ## Architecture
//!
//! The library uses a two-phase approach:
//!
//! 1. **Compilation**: JSON logic is parsed into `Logic` with OpCode dispatch
//! 2. **Evaluation**: Compiled logic is evaluated through arena dispatch — results
//!    are `&'a DataValue<'a>` allocated in a `bumpalo::Bump` for the duration of
//!    one evaluate call.
//!
//! This design enables sharing compiled logic across threads, eliminates
//! repeated parsing overhead, and lets read-through operations like `var`
//! return zero-copy borrows into the caller's input data.

mod arena;
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
mod eval_data;
mod node;
mod opcode;
mod operators;
mod path;
mod scratch;
#[cfg(feature = "trace")]
mod trace;
mod value;

pub use arena::{ContextStack, DataValue, data_to_json_string};
pub use builder::EngineBuilder;
pub use config::{
    DivisionByZeroHandling, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator,
};
/// The `datavalue` crate, re-exported. `datalogic-rs` builds on `datavalue`'s
/// owned and borrowed value types — accessing them through this module makes
/// the dependency explicit at the use site.
pub use datavalue;
pub use engine::Engine;
pub use error::{CustomSource, Error, ErrorKind};
pub use eval_data::IntoEvalData;
pub use node::Logic;
pub use path::PathStep;
pub use scratch::Scratch;
#[cfg(feature = "trace")]
pub use trace::{
    ExecutionStep, ExpressionNode, TraceCollector, TracedResult, TracedRun, TracedSession,
};
#[cfg(feature = "compat")]
pub use value::{owned_from_serde, owned_to_serde};

// `CompiledNode`, `OpCode`, `MetadataHint`, `PathSegment`, `ReduceHint`
// were public in 4.x. They are compile-internal types — `pub(crate)` for
// our own modules, surfaced via `crate::compat` for 4.x callers (with
// deprecation warnings).
#[allow(unused_imports)]
pub(crate) use node::{CompiledNode, MetadataHint, PathSegment, ReduceHint};
pub(crate) use opcode::OpCode;

/// Result type for Engine operations
pub type Result<T> = std::result::Result<T, Error>;

/// Custom operator hook for the [`Engine`].
///
/// Implementations receive args **already evaluated** as borrowed
/// [`DataValue`] references and return a `&'a DataValue<'a>` result
/// allocated in the supplied [`bumpalo::Bump`] arena.
///
/// ## Lifetime
///
/// `'a` is the arena lifetime, tied to the [`bumpalo::Bump`] allocator
/// that lives for the duration of one [`Engine::evaluate`] call. Args
/// borrow from the caller's input and from prior arena allocations; the
/// returned `&'a DataValue<'a>` must be allocated in the arena (or be a
/// preallocated singleton) — never a stack reference.
///
/// ## Example
///
/// ```rust
/// use datalogic_rs::{ContextStack, Operator, DataValue, Engine, Result};
/// use bumpalo::Bump;
///
/// struct DoubleArena;
/// impl Operator for DoubleArena {
///     fn evaluate<'a>(
///         &self,
///         args: &[&'a DataValue<'a>],
///         _ctx: &mut ContextStack<'a>,
///         arena: &'a Bump,
///     ) -> Result<&'a DataValue<'a>> {
///         let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
///         Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
///     }
/// }
///
/// let engine = Engine::builder().add_operator("double", DoubleArena).build();
///
/// let result = engine.evaluate_str(r#"{"double": 21}"#, "null").unwrap();
/// assert_eq!(result, "42");
/// ```
pub trait Operator: Send + Sync {
    /// Evaluate this operator with arena-allocated args and result.
    ///
    /// # Arguments
    ///
    /// * `args` — pre-evaluated args as `&'a DataValue<'a>`. The arena
    ///   dispatcher has already recursed into each arg's expression tree.
    /// * `ctx` — the arena context stack. Most operators won't touch
    ///   this; it's needed only when the operator iterates and pushes
    ///   its own frames (analogous to `filter` / `map`).
    /// * `arena` — the [`bumpalo::Bump`] allocator. Use `arena.alloc(...)`
    ///   for arena values, `arena.alloc_str(...)` for strings.
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        ctx: &mut ContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>>;
}

/// Sealed-trait scaffolding for [`IntoOperatorBox`].
mod operator_box_sealed {
    pub trait Sealed {}
    impl<T: crate::Operator + 'static> Sealed for T {}
    impl Sealed for Box<dyn crate::Operator> {}
}

/// Adapter that lets [`EngineBuilder::add_operator`] accept either a bare
/// `T: Operator` *or* a pre-boxed `Box<dyn Operator>`. **Sealed** — the
/// only two impls are the blanket one for `T: Operator + 'static` and the
/// pass-through for `Box<dyn Operator>`.
pub trait IntoOperatorBox: operator_box_sealed::Sealed {
    /// Coerce `self` into a `Box<dyn Operator>` for storage on the
    /// engine.
    fn into_operator_box(self) -> Box<dyn Operator>;
}

impl<T: Operator + 'static> IntoOperatorBox for T {
    #[inline]
    fn into_operator_box(self) -> Box<dyn Operator> {
        Box::new(self)
    }
}

impl IntoOperatorBox for Box<dyn Operator> {
    #[inline]
    fn into_operator_box(self) -> Box<dyn Operator> {
        self
    }
}
