mod error;
mod rule;
mod bindings;

pub use error::Error;
use serde_json::Value;

pub use rule::Rule;

use std::sync::{Arc, RwLock};
use std::collections::HashMap;
pub use rule::custom::{CustomOperator, CustomOperatorBox};
use lazy_static::lazy_static;

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
pub struct JsonLogic {
    custom_operators: RwLock<HashMap<String, CustomOperatorBox>>
}

lazy_static! {
    static ref GLOBAL_LOGIC: Arc<JsonLogic> = Arc::new(JsonLogic::new());
}

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
        Self {
            custom_operators: RwLock::new(HashMap::new())
        }
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
        let path = "$";
        rule.apply(data, data, path).map(|cow| cow.into_owned())
    }

    /// Adds a custom operator to the JsonLogic evaluator.
    /// 
    /// This method registers a new custom operator that can be used in JSON Logic rules.
    /// The operator must implement the `CustomOperator` trait.
    /// 
    /// ## Arguments
    /// - `operator`: An implementation of `CustomOperator` trait
    /// 
    /// ## Returns
    /// - `Ok(())` if the operator was successfully added
    /// - `Err(Error)` if an operator with the same name already exists
    /// 
    /// ## Example
    /// ```rust
    /// use datalogic_rs::{JsonLogic, CustomOperator, Error};
    /// use serde_json::{json, Value};
    /// use std::borrow::Cow;
    /// 
    /// struct PowerOperator;
    /// 
    /// impl CustomOperator for PowerOperator {
    ///     fn name(&self) -> &str {
    ///         "pow"
    ///     }
    ///     
    ///     fn apply<'a>(&self, args: &[Value], _context: &'a Value, _root: &'a Value, _path: &str) -> Result<Cow<'a, Value>, Error> {
    ///         if args.len() != 2 {
    ///             return Err(Error::InvalidArguments("pow requires 2 arguments".into()));
    ///         }
    ///         let base = args[0].as_f64().unwrap_or(0.0);
    ///         let exp = args[1].as_f64().unwrap_or(0.0);
    ///         Ok(Cow::Owned(json!(base.powf(exp))))
    ///     }
    /// }
    /// 
    /// let logic = JsonLogic::global();
    /// logic.add_operator(PowerOperator);
    /// ```
    pub fn add_operator<T: CustomOperator + 'static>(&self, operator: T) -> Result<(), Error> {
        let mut operators = self.custom_operators.write().unwrap();
        let name = operator.name().to_string();
        
        if operators.contains_key(&name) {
            return Err(Error::Custom(format!("Operator '{}' already exists", name)));
        }
        
        operators.insert(name, Arc::new(operator));
        Ok(())
    }

    /// Removes a custom operator from the JsonLogic evaluator.
    /// 
    /// This method unregisters a previously added custom operator by its name.
    /// Built-in operators cannot be removed.
    /// 
    /// ## Arguments
    /// - `name`: The name of the operator to remove
    /// 
    /// ## Returns
    /// - `true` if the operator was found and removed
    /// - `false` if no operator with that name exists
    /// 
    /// ## Example
    /// ```rust
    /// use datalogic_rs::JsonLogic;
    /// 
    /// let logic = JsonLogic::global();
    /// if logic.remove_operator("pow") {
    ///     println!("Power operator removed");
    /// }
    /// ```
    pub fn remove_operator(&self, name: &str) -> bool {
        let mut operators = self.custom_operators.write().unwrap();
        operators.remove(name).is_some()
    }

    /// Retrieves a custom operator by name.
    /// 
    /// This is an internal method used during rule evaluation to look up
    /// custom operators. It's marked as `pub(crate)` because it's only needed
    /// by the rule evaluation system.
    /// 
    /// ## Arguments
    /// - `name`: The name of the operator to retrieve
    /// 
    /// ## Returns
    /// - `Some(CustomOperatorBox)` if the operator exists
    /// - `None` if no operator with that name is registered
    pub(crate) fn get_operator(&self, name: &str) -> Option<CustomOperatorBox> {
        let operators = self.custom_operators.read().unwrap();
        operators.get(name).cloned()
    }
    
    /// Returns the global JsonLogic instance.
    /// 
    /// This provides access to the global, thread-safe instance of JsonLogic
    /// that can be used to register and manage custom operators across your application.
    /// 
    /// ## Returns
    /// - `Arc<JsonLogic>`: A thread-safe reference to the global JsonLogic instance
    /// 
    /// ## Example
    /// ```rust
    /// use datalogic_rs::JsonLogic;
    /// 
    /// let logic = JsonLogic::global();
    /// // Use the global instance to register custom operators
    /// ```
    pub fn global() -> Arc<JsonLogic> {
        GLOBAL_LOGIC.clone()
    }
}
