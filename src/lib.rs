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
pub mod eval_mode;
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
pub use context::{ContextFrame, ContextStack};
pub use engine::DataLogic;
pub use error::{Error, StructuredError};
#[cfg(feature = "trace")]
pub use eval_mode::Traced;
pub use eval_mode::{Mode, Plain};
pub use node::{CompiledLogic, CompiledNode, MetadataHint, PathSegment, ReduceHint};
pub use opcode::OpCode;
#[cfg(feature = "trace")]
pub use trace::{ExecutionStep, ExpressionNode, TraceCollector, TracedResult};
pub use value::NumberValue;

use serde_json::Value;

/// Result type for DataLogic operations
pub type Result<T> = std::result::Result<T, Error>;

/// Trait for recursive evaluation of logic expressions.
///
/// This trait is implemented by the `DataLogic` engine and used internally
/// by operators that need to recursively evaluate sub-expressions.
///
/// # Example
///
/// The `if` operator uses this trait to evaluate its condition and branches:
///
/// ```rust,ignore
/// let condition_result = evaluator.evaluate(&condition, context)?;
/// if is_truthy(&condition_result) {
///     evaluator.evaluate(&then_branch, context)
/// } else {
///     evaluator.evaluate(&else_branch, context)
/// }
/// ```
pub trait Evaluator {
    /// Evaluates a logic expression within the given context.
    ///
    /// # Arguments
    ///
    /// * `logic` - The JSON logic expression to evaluate
    /// * `context` - The context stack containing data and metadata
    ///
    /// # Returns
    ///
    /// The evaluated result as a JSON value, or an error if evaluation fails.
    fn evaluate(&self, logic: &Value, context: &mut ContextStack) -> Result<Value>;
}

/// Trait for implementing custom operators.
///
/// Custom operators extend the functionality of the DataLogic engine by
/// providing domain-specific logic. Operators must be thread-safe (`Send + Sync`).
///
/// # ⚠️ Arguments are UNEVALUATED
///
/// `args` contains **raw JSONLogic expressions**, not already-evaluated values.
/// For example, given `{"my_op": [{"var": "x"}, 5]}`, `args[0]` is the literal
/// JSON `{"var": "x"}` — not the value of `x`. You must call
/// `evaluator.evaluate(&args[i], context)` to resolve each argument.
///
/// This matches how built-in operators work and lets you control evaluation
/// order (e.g. for short-circuiting or conditional branches). If you simply
/// forward a raw `Value::Object` as a result, it will not be interpreted as
/// logic — it will be returned as-is.
///
/// # Example
///
/// ```rust
/// use datalogic_rs::{Operator, ContextStack, Evaluator, Result, Error};
/// use serde_json::{json, Value};
///
/// struct UpperCaseOperator;
///
/// impl Operator for UpperCaseOperator {
///     fn evaluate(
///         &self,
///         args: &[Value],
///         context: &mut ContextStack,
///         evaluator: &dyn Evaluator,
///     ) -> Result<Value> {
///         // Evaluate the argument first — it may be a `var` reference,
///         // a nested expression, or a literal.
///         let arg = args.first().ok_or_else(|| {
///             Error::InvalidArguments("upper requires 1 argument".to_string())
///         })?;
///         let value = evaluator.evaluate(arg, context)?;
///
///         match value.as_str() {
///             Some(s) => Ok(json!(s.to_uppercase())),
///             None => Err(Error::InvalidArguments(
///                 "Argument must evaluate to a string".to_string(),
///             )),
///         }
///     }
/// }
/// ```
///
/// See `examples/custom_operator.rs` for more patterns (variadic args,
/// short-circuiting, forwarding to built-ins).
pub trait Operator: Send + Sync {
    /// Evaluates the operator with the given arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - The **unevaluated** JSONLogic expressions passed to the
    ///   operator. Call `evaluator.evaluate(&args[i], context)` to resolve
    ///   each one.
    /// * `context` - The context stack for accessing data and metadata
    /// * `evaluator` - The evaluator used to recursively evaluate `args`
    ///   and any other sub-expressions
    ///
    /// # Returns
    ///
    /// The result of the operator evaluation, or an error if the operation fails.
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value>;
}

/// Arena-mode custom operator — opt-in zero-clone variant of [`Operator`].
///
/// Implementations receive args **already evaluated** as borrowed
/// [`ArenaValue`] references and return a `&'a ArenaValue<'a>` result.
/// This avoids the per-call promotion to owned [`serde_json::Value`] that
/// the legacy [`Operator`] trait incurs at the arena dispatch boundary.
///
/// ## When to use
///
/// Reach for [`ArenaOperator`] when your custom op is invoked **inside a
/// hot iteration body** (`filter` / `map` / `reduce` predicate) over a
/// sizable input — the clone overhead compounds there. For operators
/// called once per evaluation, the legacy [`Operator`] trait is simpler
/// and the perf difference is negligible.
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
///
/// ## Coexistence with [`Operator`]
///
/// You can register both forms under different names. If both are
/// registered under the **same** name, the arena form takes precedence
/// — the legacy form is never reached for that name.
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
