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

mod compiled;
mod config;
mod constants;
mod context;
mod datetime;
mod engine;
mod error;
mod opcode;
mod operators;
mod trace;
mod value_helpers;

pub use compiled::{CompiledLogic, CompiledNode};
pub use config::{
    DivisionByZeroHandling, EvaluationConfig, NanHandling, NumericCoercionConfig, TruthyEvaluator,
};
pub use context::{ContextFrame, ContextStack};
pub use engine::DataLogic;
pub use error::Error;
pub use opcode::OpCode;
pub use trace::{ExecutionStep, ExpressionNode, TraceCollector, TracedResult};

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
///         _context: &mut ContextStack,
///         _evaluator: &dyn Evaluator,
///     ) -> Result<Value> {
///         if let Some(s) = args.first().and_then(|v| v.as_str()) {
///             Ok(json!(s.to_uppercase()))
///         } else {
///             Err(Error::InvalidArguments("Argument must be a string".to_string()))
///         }
///     }
/// }
/// ```
pub trait Operator: Send + Sync {
    /// Evaluates the operator with the given arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - The evaluated arguments passed to the operator
    /// * `context` - The context stack for accessing data and metadata
    /// * `evaluator` - The evaluator for recursive evaluation of sub-expressions
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
