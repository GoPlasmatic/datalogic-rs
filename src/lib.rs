//! A high-performance JSON Logic implementation for Rust
//! 
//! This crate provides a way to write portable logic rules as JSON, following the 
//! [JSONLogic specification](http://jsonlogic.com). It offers:
//! 
//! - Full compliance with JSONLogic specification
//! - Thread-safe evaluation of rules
//! - Zero-copy JSON handling
//! - Comprehensive error handling
//!
//! # Quick Example
//! ```rust
//! use datalogic_rs::{JsonLogic, Rule};
//! use serde_json::json;
//!
//! let rule = Rule::from_value(&json!({
//!     "if": [
//!         {">": [{"var": "temp"}, 110]},
//!         "too hot",
//!         "ok"
//!     ]
//! })).unwrap();
//!
//! let data = json!({"temp": 120});
//! let result = JsonLogic::apply(&rule, &data).unwrap();
//! assert_eq!(result, json!("too hot"));
//! ```

mod error;
mod rule;

use error::Error;
use serde_json::Value;
pub use rule::Rule;

/// Result type for JSON Logic operations
pub type JsonLogicResult = Result<Value, Error>;

/// Main entry point for evaluating JSON Logic rules
/// 
/// Provides a thread-safe, zero-copy implementation for evaluating rules against data.
#[derive(Clone)]
pub struct JsonLogic {}

impl Default for JsonLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLogic {
    /// Creates a new JsonLogic evaluator
    pub fn new() -> Self {
        Self {}
    }

    /// Evaluates a rule against the provided data
    ///
    /// ## Arguments
    /// * `rule` - The compiled rule to evaluate
    /// * `data` - The data to evaluate against
    ///
    /// ## Returns
    /// * `JsonLogicResult` containing either the evaluation result or an error
    ///
    /// ## Example
    /// ```rust
    /// # use datalogic_rs::{JsonLogic, Rule};
    /// # use serde_json::json;
    /// let rule = Rule::from_value(&json!({"var": "user.name"})).unwrap();
    /// let data = json!({"user": {"name": "John"}});
    /// let result = JsonLogic::apply(&rule, &data).unwrap();
    /// assert_eq!(result, json!("John"));
    /// ```
    pub fn apply(rule: &Rule, data: &Value) -> JsonLogicResult {
        rule.apply(data)
    }
}
