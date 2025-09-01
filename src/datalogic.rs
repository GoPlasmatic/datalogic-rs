//! Main interface for the DataLogic library
//!
//! This module provides the DataLogic struct which is the primary entry point
//! for parsing and evaluating logic expressions.

use crate::LogicError;
use crate::arena::DataArena;
use crate::arena::{CustomOperatorRegistry, SimpleOperatorAdapter, SimpleOperatorFn};
use crate::context::EvalContext;
use crate::logic::{Logic, Result, evaluate, optimize};
use crate::parser::{
    parse_jsonlogic, parse_jsonlogic_json, parse_jsonlogic_json_with_preserve,
    parse_jsonlogic_with_preserve,
};
use crate::value::{DataValue, FromJson, ToJson};
use serde_json::Value as JsonValue;

/// Trait for custom JSONLogic operators
pub use crate::arena::CustomOperator;

/// Main interface for the DataLogic library
///
/// # Examples
///
/// ```
/// use datalogic_rs::DataLogic;
///
/// let dl = DataLogic::new();
/// let result = dl.evaluate_str(
///     r#"{ ">": [{"var": "temp"}, 100] }"#,
///     r#"{"temp": 110, "name": "user"}"#,
/// ).unwrap();
/// assert_eq!(result.to_string(), "true");
/// ```
pub struct DataLogic {
    arena: DataArena,
    preserve_structure: bool,
    custom_operators: CustomOperatorRegistry,
}

impl DataLogic {
    /// Create a new DataLogic instance with default settings
    pub fn new() -> Self {
        Self {
            arena: DataArena::new(),
            preserve_structure: false,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Create a new DataLogic instance with a specific chunk size for the arena
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            arena: DataArena::with_chunk_size(chunk_size),
            preserve_structure: false,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Create a new DataLogic instance with structure preservation enabled
    ///
    /// When enabled, multi-key objects will preserve their structure and evaluate
    /// inner values instead of throwing OperatorNotFoundError.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::DataLogic;
    ///
    /// let dl = DataLogic::with_preserve_structure();
    /// let result = dl.evaluate_str(
    ///     r#"{"isEqual": {"==": [1, 1]}}"#,
    ///     r#"{}"#,
    /// ).unwrap();
    /// // Returns: {"isEqual": true}
    /// ```
    pub fn with_preserve_structure() -> Self {
        Self {
            arena: DataArena::new(),
            preserve_structure: true,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Create a new DataLogic instance with both custom chunk size and structure preservation
    pub fn with_chunk_size_and_preserve_structure(chunk_size: usize) -> Self {
        Self {
            arena: DataArena::with_chunk_size(chunk_size),
            preserve_structure: true,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Get a reference to the internal arena
    ///
    /// This is exposed for advanced usage scenarios, but most users
    /// won't need to access this directly.
    pub fn arena(&self) -> &DataArena {
        &self.arena
    }

    /// Check if structure preservation is enabled
    pub fn preserve_structure(&self) -> bool {
        self.preserve_structure
    }

    /// Reset the internal arena to free memory
    ///
    /// This clears all allocated data from the arena, invalidating any
    /// existing DataValue or Logic instances.
    pub fn reset_arena(&mut self) {
        self.arena.reset();
    }

    /// Register a custom operator implementation
    ///
    /// This allows users to extend JSONLogic with custom operations.
    /// The implementation should take an array of DataValue objects and return a DataValue result.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::{DataLogic, DataValue, LogicError, Result, CustomOperator, EvalContext};
    /// use datalogic_rs::value::NumberValue;
    /// use std::fmt::Debug;
    /// use datalogic_rs::arena::DataArena;
    ///
    /// // Define a custom operator that multiplies all numbers in the array
    /// #[derive(Debug)]
    /// struct MultiplyAll;
    ///
    /// impl CustomOperator for MultiplyAll {
    ///     fn evaluate<'a>(&self, args: &'a [DataValue<'a>], _context: &EvalContext<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    ///         // Default to 1 if no arguments provided
    ///         if args.is_empty() {
    ///             return Ok(arena.alloc(DataValue::Number(NumberValue::from_i64(1))));
    ///         }
    ///
    ///         // Calculate product of all numeric values
    ///         let mut product = 1.0;
    ///         for arg in args {
    ///             if let Some(n) = arg.as_f64() {
    ///                 product *= n;
    ///             }
    ///         }
    ///
    ///         // Return the result
    ///         Ok(arena.alloc(DataValue::Number(NumberValue::from_f64(product))))
    ///     }
    /// }
    ///
    /// let mut dl = DataLogic::new();
    /// dl.register_custom_operator("multiply_all", Box::new(MultiplyAll));
    ///
    /// // Use the custom operator
    /// let result = dl.evaluate_str(
    ///     r#"{"multiply_all": [2, 3, 4]}"#,
    ///     r#"{}"#,
    /// ).unwrap();
    /// assert_eq!(result.as_f64().unwrap(), 24.0);
    /// ```
    pub fn register_custom_operator(&mut self, name: &str, operator: Box<dyn CustomOperator>) {
        self.custom_operators.register(name, operator);
    }

    /// Check if a custom operator is registered
    pub fn has_custom_operator(&self, name: &str) -> bool {
        self.custom_operators.get(name).is_some()
    }

    /// Parse a logic expression from a string
    pub fn parse_logic(&self, source: &str) -> Result<Logic<'_>> {
        let token = if self.preserve_structure {
            parse_jsonlogic_with_preserve(source, &self.arena, true, &self.custom_operators)?
        } else {
            parse_jsonlogic(source, &self.arena, &self.custom_operators)?
        };

        // Apply static optimization
        let optimized_token = optimize(token, &self.arena)?;

        Ok(Logic::new(optimized_token, &self.arena))
    }

    /// Parse a JSON logic expression into a Token
    pub fn parse_logic_json(&self, source: &JsonValue) -> Result<Logic<'_>> {
        let token = if self.preserve_structure {
            parse_jsonlogic_json_with_preserve(source, &self.arena, true, &self.custom_operators)?
        } else {
            parse_jsonlogic_json(source, &self.arena, &self.custom_operators)?
        };
        Ok(Logic::new(token, &self.arena))
    }

    /// Parse a JSON data string into a DataValue
    pub fn parse_data(&self, source: &str) -> Result<DataValue<'_>> {
        let json = serde_json::from_str(source).map_err(|e| LogicError::ParseError {
            reason: e.to_string(),
        })?;
        Ok(DataValue::from_json(&json, &self.arena))
    }

    /// Parse a JSON data string into a DataValue
    pub fn parse_data_json(&self, source: &JsonValue) -> Result<DataValue<'_>> {
        Ok(DataValue::from_json(source, &self.arena))
    }

    /// Evaluate a rule with the provided data
    ///
    /// This method evaluates a logic rule against the given data context.
    /// The data is used as both the current context and the root context for evaluation.
    ///
    /// # Arguments
    ///
    /// * `rule` - The compiled logic rule to evaluate
    /// * `data` - The data to use as context during evaluation
    ///
    /// # Returns
    ///
    /// A Result containing a reference to the evaluation result as a DataValue
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::DataLogic;
    ///
    /// let dl = DataLogic::new();
    /// let rule = dl.parse_logic(r#"{ ">": [{"var": "temp"}, 100] }"#).unwrap();
    /// let data = dl.parse_data(r#"{"temp": 110}"#).unwrap();
    /// let result = dl.evaluate(&rule, &data).unwrap();
    /// assert_eq!(result.to_string(), "true");
    /// ```
    pub fn evaluate<'a>(
        &'a self,
        rule: &'a Logic,
        data: &'a DataValue,
    ) -> Result<&'a DataValue<'a>> {
        // Create evaluation context with the data as root and custom operators
        let context = EvalContext::new(data, &self.custom_operators);

        // Evaluate the rule with the data as context
        evaluate(rule.root(), &context, &self.arena)
    }

    /// Evaluate using JSON values directly
    ///
    /// This method evaluates a logic rule against data, both provided as JSON values.
    /// It parses the logic and data from JSON, evaluates the rule, and returns
    /// the result as a JSON value.
    ///
    /// # Arguments
    ///
    /// * `logic` - The logic rule as a JsonValue
    /// * `data` - The data context as a JsonValue
    ///
    /// # Returns
    ///
    /// A Result containing the evaluation result as a JsonValue
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::DataLogic;
    /// use serde_json::json;
    ///
    /// let dl = DataLogic::new();
    /// let logic = json!({"ceil": 3.14});
    /// let data = json!({});
    /// let result = dl.evaluate_json(&logic, &data).unwrap();
    /// assert_eq!(result.as_i64().unwrap(), 4);
    /// ```
    pub fn evaluate_json(&self, logic: &JsonValue, data: &JsonValue) -> Result<JsonValue> {
        let rule = self.parse_logic_json(logic)?;
        let data_value = self.parse_data_json(data)?;
        let result = self.evaluate(&rule, &data_value)?;
        Ok(result.to_json())
    }

    /// Parse and evaluate in one step, returning a JSON value
    pub fn evaluate_str(&self, logic_source: &str, data_source: &str) -> Result<JsonValue> {
        let rule = self.parse_logic(logic_source)?;
        let data_value = self.parse_data(data_source)?;
        let result = self.evaluate(&rule, &data_value)?;
        Ok(result.to_json())
    }

    /// Register a simple custom operator implementation
    ///
    /// This method provides an easier way to register custom operators
    /// without needing to understand arena-based memory management. The operator
    /// is implemented as a function that takes owned DataValue objects and returns
    /// an owned DataValue result.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::{DataLogic, DataValue, Result};
    ///
    /// // Define a simple operator that doubles a number
    /// fn double<'r>(args: Vec<DataValue<'r>>, data: DataValue<'r>) -> std::result::Result<DataValue<'r>, String> {
    ///     if args.is_empty() {
    ///         // Check data context for value if no args provided
    ///         if let Some(obj) = data.as_object() {
    ///             for (key, val) in obj {
    ///                 if *key == "value" && val.is_number() {
    ///                     if let Some(n) = val.as_f64() {
    ///                         return Ok(DataValue::float(n * 2.0));
    ///                     }
    ///                 }
    ///             }
    ///         }
    ///         return Err("double operator requires at least one argument or 'value' in data".to_string());
    ///     }
    ///     
    ///     if let Some(n) = args[0].as_f64() {
    ///         return Ok(DataValue::float(n * 2.0));
    ///     }
    ///     
    ///     Err("Argument must be a number".to_string())
    /// }
    ///
    /// let mut dl = DataLogic::new();
    ///
    /// // Register the simple operator
    /// dl.register_simple_operator("double", double);
    ///
    /// // Use the custom operator in a rule with explicit argument
    /// let result = dl.evaluate_str(
    ///     r#"{"double": 5}"#,
    ///     r#"{}"#,
    /// ).unwrap();
    ///
    /// assert_eq!(result.as_f64().unwrap(), 10.0);
    ///
    /// // Use the custom operator with data context
    /// let result = dl.evaluate_str(
    ///     r#"{"double": []}"#,
    ///     r#"{"value": 7}"#,
    /// ).unwrap();
    ///
    /// assert_eq!(result.as_f64().unwrap(), 14.0);
    /// ```
    pub fn register_simple_operator(&mut self, name: &str, function: SimpleOperatorFn) {
        let adapter = SimpleOperatorAdapter::new(name, function);
        self.register_custom_operator(name, Box::new(adapter));
    }
}

impl Default for DataLogic {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;
    use crate::value::{DataValue, NumberValue};
    use serde_json::json;

    #[derive(Debug)]
    struct MultiplyAll;

    impl CustomOperator for MultiplyAll {
        fn evaluate<'a>(
            &self,
            args: &'a [DataValue<'a>],
            _context: &crate::context::EvalContext<'a>,
            arena: &'a DataArena,
        ) -> Result<&'a DataValue<'a>> {
            // Default to 1 if no arguments provided
            if args.is_empty() {
                return Ok(arena.alloc(DataValue::Number(NumberValue::from_i64(1))));
            }

            // Calculate product of all numeric values
            let mut product = 1.0;
            for arg in args {
                if let Some(n) = arg.as_f64() {
                    product *= n;
                }
            }

            // Return the result
            Ok(arena.alloc(DataValue::Number(NumberValue::from_f64(product))))
        }
    }

    #[test]
    fn test_custom_operator() {
        let mut dl = DataLogic::new();

        // Register custom operator
        dl.register_custom_operator("multiply_all", Box::new(MultiplyAll));

        // Test with JSON values
        let result = dl
            .evaluate_json(&json!({"multiply_all": [2, 3, 4]}), &json!({}))
            .unwrap();

        assert_eq!(result.as_f64().unwrap(), 24.0);

        // Test with string values
        let result = dl
            .evaluate_str(r#"{"multiply_all": [2, 3, 4]}"#, r#"{}"#)
            .unwrap();

        assert_eq!(result.as_f64().unwrap(), 24.0);
    }
}
