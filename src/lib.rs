mod error;
mod rule;

use error::Error;
use serde_json::Value;
pub use rule::Rule;

/// # JSONLogic Result Type
/// 
/// This is the primary result type used for JSON Logic evaluations. 
/// All operations return a `Result<Value, Error>` where:
/// - `Value` represents the evaluated result.
/// - `Error` contains failure details if evaluation fails.
pub type JsonLogicResult = Result<Value, Error>;

/// # JSONLogic Evaluator
/// 
/// The main entry point for evaluating JSON Logic rules.
/// 
/// This provides a thread-safe, zero-copy implementation for evaluating 
/// rules against data. Rules are parsed into an internal representation 
/// and evaluated efficiently.
/// 
/// ## Example
/// ```rust
/// use datalogic_rs::{JsonLogic, Rule};
/// use serde_json::json;
/// 
/// let rule = Rule::from_value(&json!({">": [{"var": "score"}, 50]})).unwrap();
/// let data = json!({"score": 75});
/// let result = JsonLogic::apply(&rule, &data).unwrap();
/// assert_eq!(result, json!(true));
/// ```
/// 
/// This will return `true` because `75 > 50`.
#[derive(Clone)]
pub struct JsonLogic;

impl Default for JsonLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLogic {
    /// Creates a new JsonLogic evaluator.
    /// 
    /// This function initializes the JSON Logic processor, allowing 
    /// rules to be evaluated efficiently.
    pub fn new() -> Self {
        Self {}
    }

    /// Evaluates a compiled JSON Logic rule against provided data.
    /// 
    /// ## Arguments
    /// - `rule`: A compiled `Rule` instance
    /// - `data`: The JSON data against which the rule will be evaluated
    /// 
    /// ## Returns
    /// - `JsonLogicResult`: The evaluated output or an error if the rule fails.
    /// 
    /// ## Example
    /// ```rust
    /// use datalogic_rs::{JsonLogic, Rule};
    /// use serde_json::json;
    /// 
    /// let rule = Rule::from_value(&json!({">": [{"var": "age"}, 18]})).unwrap();
    /// let data = json!({"age": 21});
    /// let result = JsonLogic::apply(&rule, &data).unwrap();
    /// assert_eq!(result, json!(true));
    /// ```
    pub fn apply(rule: &Rule, data: &Value) -> JsonLogicResult {
        rule.apply(data)
    }
}
