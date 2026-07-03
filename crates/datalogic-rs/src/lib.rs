#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
// Enable the `#[doc(cfg(...))]` attribute on docs.rs builds so feature-gated
// public items render with a "Available on crate feature X only" badge. The
// matching `--cfg docsrs` is passed by `[package.metadata.docs.rs]` in
// Cargo.toml; on a regular stable build this attribute is inert.
#![cfg_attr(docsrs, feature(doc_cfg))]

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
//! - **59 built-in operators**: Complete JSONLogic compatibility plus extensions
//! - **Arena-allocated evaluation**: Results live in a `bumpalo::Bump` arena and can borrow directly into caller input for zero-copy paths
//! - **Extensible**: Add custom operators via the [`CustomOperator`] trait
//! - **Structured templates**: Preserve object structure for dynamic outputs
//!
//! ## Quick Start (one-shot)
//!
//! ```rust
//! use datalogic_rs::Engine;
//!
//! let engine = Engine::new();
//! let result = engine.eval_str(
//!     r#"{"==": [{"var": "status"}, "active"]}"#,
//!     r#"{"status": "active"}"#,
//! ).unwrap();
//! assert_eq!(result, "true");
//! ```
//!
//! ## Reusing the arena across many evaluations
//!
//! For high-throughput callers, open a [`Session`] handle. It owns a
//! [`bumpalo::Bump`], resets it between calls, and returns owned results so
//! you don't have to juggle arena lifetimes:
//!
//! ```rust
//! use datalogic_rs::Engine;
//!
//! let engine = Engine::new();
//! let compiled = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
//! let mut session = engine.session();
//!
//! for x in 0..3 {
//!     let payload = format!(r#"{{"x": {}}}"#, x);
//!     let result = session.eval_str(&compiled, &payload).unwrap();
//!     assert_eq!(result, (x + 1).to_string());
//!     // The session does not auto-reset; bound peak memory by
//!     // resetting between iterations (constant-time, reuses chunks).
//!     session.reset();
//! }
//! ```
//!
//! ## Power-user (compile once, evaluate many, zero-copy results)
//!
//! When the result borrow can stay scoped to a caller-managed
//! [`bumpalo::Bump`], skip the deep-clone and use [`Engine::evaluate`]
//! directly. `evaluate` accepts any input shape via [`EvalInput`]:
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
mod arena_ext;
mod builder;
mod compile;
mod config;
mod engine;
mod error;
mod eval_input;
mod logic_input;
mod node;
mod node_serialize;
mod opcode;
pub mod operator;
mod operators;
mod parsed_data;
mod path;
mod result_output;
#[cfg(feature = "serde_json")]
mod serde_bridge;
mod session;
mod top_level;
#[cfg(feature = "trace")]
mod trace;

pub use arena::DataValue;
pub use arena_ext::ArenaExt;
pub use builder::EngineBuilder;
/// The [`bumpalo`] arena allocator, re-exported.
///
/// `Engine::evaluate` and the [`CustomOperator`] trait both take a
/// `&'a bumpalo::Bump` parameter, so callers need a way to construct
/// arenas. Re-exporting locks the major version of `bumpalo` to whatever
/// `datalogic-rs` itself depends on — pair with `use datalogic_rs::bumpalo`
/// instead of an independent `bumpalo` dep to avoid major-version skew.
pub use bumpalo;
pub use config::{
    DivisionByZeroHandling, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator,
};
/// The `datavalue` crate, re-exported. `datalogic-rs` builds on `datavalue`'s
/// owned and borrowed value types — accessing them through this module makes
/// the dependency explicit at the use site.
///
/// # Working with `DataValue`
///
/// Evaluation returns [`DataValue`] (re-exported at the crate root and
/// also reachable as `datalogic_rs::datavalue::DataValue`). It's an
/// arena-allocated JSON-shaped value tree borrowed from a
/// [`bumpalo::Bump`]. The accessors most callers reach for live in this
/// re-exported crate:
///
/// - **Type predicates** — `.is_null()`, `.is_bool()`, `.is_number()`,
///   `.is_string()`, `.is_array()`, `.is_object()`.
/// - **Owned readers** — `.as_bool()`, `.as_i64()`, `.as_f64()`,
///   `.as_str()`, `.as_array()`, `.as_object()`. Each returns
///   `Option<…>`; the `None` case is "wrong variant," not a runtime error.
/// - **Indexing** — `value["key"]` / `value[idx]` returns `&DataValue`
///   (or the `Null` singleton on miss, matching `serde_json::Value`).
///
/// # Owned vs borrowed
///
/// [`DataValue<'a>`](datavalue::DataValue) borrows from a `Bump`;
/// [`OwnedDataValue`](datavalue::OwnedDataValue) is the heap-owned
/// counterpart. Use the owned form when you need to outlive the arena —
/// caching a result, returning across an `await`, sending across a
/// channel. Convert via `borrowed.to_owned()` and `owned.to_arena(&bump)`.
///
/// # Crossing the `serde_json` boundary
///
/// Conversions to / from `serde_json::Value` are gated behind the
/// `serde_json` feature (kept off by default so the crate has zero
/// external dependencies in the minimal build). With `serde_json`
/// enabled, pass a `&serde_json::Value` (or any `&T: Serialize`) into
/// any `eval*` method via [`EvalInput`] / [`IntoLogic`], and ask for a
/// `serde_json::Value` (or any `T: DeserializeOwned`) back via
// `Engine::eval_into` / `Session::eval_into` are gated behind
// `serde_json`; link them when the feature is on, otherwise emit them
// as code text so default-features `cargo doc` doesn't break.
#[cfg_attr(
    feature = "serde_json",
    doc = "[`Engine::eval_into`] / [`Session::eval_into`]. For the `DataValue"
)]
#[cfg_attr(
    not(feature = "serde_json"),
    doc = "`Engine::eval_into` / `Session::eval_into`. For the `DataValue"
)]
/// → JSON String` path use the standard `value.to_string()`, which is
/// what [`Engine::eval_str`] uses internally.
pub use datavalue;
pub use engine::Engine;
pub use error::{CustomErrorSource, Error, ErrorKind};
pub use eval_input::{EvalInput, OwnedInput};
pub use logic_input::IntoLogic;
pub use node::Logic;
pub use parsed_data::ParsedData;
pub use path::PathStep;
pub use result_output::FromDataValue;
pub use session::Session;
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub use top_level::eval_into;
pub use top_level::{compile, eval, eval_str};
#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
pub use trace::{ExecutionStep, ExpressionNode, TracedRun, TracedSession};

// `CompiledNode`, `OpCode`, `MetadataHint`, `PathSegment`, `ReduceHint` were
// public in 4.x. They are compile-internal in v5; consumers reach for them
// via `crate::node::*` / `crate::opcode::*` directly.
pub(crate) use node::CompiledNode;
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
/// use datalogic_rs::{CustomOperator, DataValue, Engine, Result, operator::EvalContext};
/// use bumpalo::Bump;
///
/// struct DoubleArena;
/// impl CustomOperator for DoubleArena {
///     fn evaluate<'a>(
///         &self,
///         args: &[&'a DataValue<'a>],
///         _ctx: &mut EvalContext<'_, 'a>,
///         arena: &'a Bump,
///     ) -> Result<&'a DataValue<'a>> {
///         let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
///         Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
///     }
/// }
///
/// let engine = Engine::builder().add_operator("double", DoubleArena).build();
///
/// let result = engine.eval_str(r#"{"double": 21}"#, "null").unwrap();
/// assert_eq!(result, "42");
/// ```
///
/// ## Stability
///
/// This trait is the headline extension point of the crate and is
/// intentionally not sealed. Within the **5.x series** the only changes
/// that will be made to this trait are *default-method additions* — no
/// new required methods, no signature changes to [`Self::evaluate`], no
/// lifetime restructuring. Implementations written against 5.0 will
/// compile against every 5.x release without modification. Any breaking
/// change here requires a 6.0 bump.
///
/// The opaque types in the signature ([`crate::DataValue`],
/// [`operator::EvalContext`], [`bumpalo::Bump`]) may evolve internally
/// without breaking this contract, since their public surface is the
/// stable boundary.
pub trait CustomOperator: Send + Sync {
    /// Evaluate this operator with arena-allocated args and result.
    ///
    /// # Arguments
    ///
    /// * `args` — pre-evaluated args as `&'a DataValue<'a>`. The arena
    ///   dispatcher has already recursed into each arg's expression tree.
    /// * `ctx` — opaque view into the engine's evaluation context. Most
    ///   operators ignore this; it exposes [`operator::EvalContext::root_input`]
    ///   and [`operator::EvalContext::depth`] for the rare case where an
    ///   operator's behaviour depends on the surrounding context.
    /// * `arena` — the [`bumpalo::Bump`] allocator. Use `arena.alloc(...)`
    ///   for arena values, `arena.alloc_str(...)` for strings. For the
    ///   common case of returning a typed `DataValue` result, prefer the
    ///   one-call helpers on [`ArenaExt`] (`arena.f64(n)`,
    ///   `arena.string(s)`, `arena.bool(b)`, …) — they are zero-cost
    ///   over the manual form and short-circuit to preallocated
    ///   singletons for `null`, booleans, small ints, and empty
    ///   string/array/object.
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        ctx: &mut operator::EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>>;
}

// `Box<dyn CustomOperator>` itself implements `CustomOperator` by
// delegating to the inner trait object. This collapses what used to be
// two separate registration methods on `EngineBuilder` (`add_operator`
// for typed operators, `add_operator_box` for pre-boxed trait objects)
// into a single entry point: `EngineBuilder::add_operator(name, op)`
// accepts either a typed `T: CustomOperator + 'static` or a
// `Box<dyn CustomOperator>` produced by a runtime registry.
impl CustomOperator for Box<dyn CustomOperator> {
    #[inline]
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        ctx: &mut operator::EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        (**self).evaluate(args, ctx, arena)
    }
}
