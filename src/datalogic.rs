//! Main interface for the DataLogic library
//!
//! This module provides the DataLogic struct which is the primary entry point
//! for parsing and evaluating logic expressions.

use crate::arena::DataArena;
use crate::arena::{SimpleOperatorAdapter, SimpleOperatorFn};
use crate::logic::{evaluate, optimize, Logic, Result};
use crate::parser::{ExpressionParser, ParserRegistry};
use crate::value::{DataValue, FromJson, ToJson};
use crate::LogicError;
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
///     None
/// ).unwrap();
/// assert_eq!(result.to_string(), "true");
/// ```
pub struct DataLogic {
    arena: DataArena,
    parsers: ParserRegistry,
    preserve_structure: bool,
}

impl DataLogic {
    /// Create a new DataLogic instance with default settings
    pub fn new() -> Self {
        Self {
            arena: DataArena::new(),
            parsers: ParserRegistry::new(),
            preserve_structure: false,
        }
    }

    /// Create a new DataLogic instance with a specific chunk size for the arena
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            arena: DataArena::with_chunk_size(chunk_size),
            parsers: ParserRegistry::new(),
            preserve_structure: false,
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
    ///     None
    /// ).unwrap();
    /// // Returns: {"isEqual": true}
    /// ```
    pub fn with_preserve_structure() -> Self {
        Self {
            arena: DataArena::new(),
            parsers: ParserRegistry::new(),
            preserve_structure: true,
        }
    }

    /// Create a new DataLogic instance with both custom chunk size and structure preservation
    pub fn with_chunk_size_and_preserve_structure(chunk_size: usize) -> Self {
        Self {
            arena: DataArena::with_chunk_size(chunk_size),
            parsers: ParserRegistry::new(),
            preserve_structure: true,
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

    /// Register a parser for a specific expression format
    pub fn register_parser(&mut self, parser: Box<dyn ExpressionParser>) {
        self.parsers.register(parser);
    }

    /// Set the default parser
    pub fn set_default_parser(&mut self, format_name: &str) -> Result<()> {
        self.parsers.set_default(format_name)
    }

    /// Register a custom operator implementation
    ///
    /// This allows users to extend JSONLogic with custom operations.
    /// The implementation should take an array of DataValue objects and return a DataValue result.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::{DataLogic, DataValue, LogicError, Result, CustomOperator};
    /// use datalogic_rs::value::NumberValue;
    /// use std::fmt::Debug;
    /// use datalogic_rs::arena::DataArena;
    ///
    /// // Define a custom operator that multiplies all numbers in the array
    /// #[derive(Debug)]
    /// struct MultiplyAll;
    ///
    /// impl CustomOperator for MultiplyAll {
    ///     fn evaluate<'a>(&self, args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
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
    ///     None
    /// ).unwrap();
    /// assert_eq!(result.as_f64().unwrap(), 24.0);
    /// ```
    pub fn register_custom_operator(&mut self, name: &str, operator: Box<dyn CustomOperator>) {
        self.arena.register_custom_operator(name, operator);
    }

    /// Check if a custom operator is registered
    pub fn has_custom_operator(&self, name: &str) -> bool {
        self.arena.has_custom_operator(name)
    }

    /// Parse a logic expression using the specified parser format
    pub fn parse_logic(&self, source: &str, format: Option<&str>) -> Result<Logic> {
        let token = if self.preserve_structure {
            self.parsers
                .parse_with_preserve(source, format, &self.arena, true)?
        } else {
            self.parsers.parse(source, format, &self.arena)?
        };

        // Apply static optimization
        let optimized_token = optimize(token, &self.arena)?;

        Ok(Logic::new(optimized_token, &self.arena))
    }

    /// Parse a JSON logic expression into a Token
    pub fn parse_logic_json(&self, source: &JsonValue, format: Option<&str>) -> Result<Logic> {
        let token = if self.preserve_structure {
            self.parsers
                .parse_json_with_preserve(source, format, &self.arena, true)?
        } else {
            self.parsers.parse_json(source, format, &self.arena)?
        };
        Ok(Logic::new(token, &self.arena))
    }

    /// Parse a JSON data string into a DataValue
    pub fn parse_data(&self, source: &str) -> Result<DataValue> {
        let json = serde_json::from_str(source).map_err(|e| LogicError::ParseError {
            reason: e.to_string(),
        })?;
        Ok(DataValue::from_json(&json, &self.arena))
    }

    /// Parse a JSON data string into a DataValue
    pub fn parse_data_json(&self, source: &JsonValue) -> Result<DataValue> {
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
    /// let rule = dl.parse_logic(r#"{ ">": [{"var": "temp"}, 100] }"#, None).unwrap();
    /// let data = dl.parse_data(r#"{"temp": 110}"#).unwrap();
    /// let result = dl.evaluate(&rule, &data).unwrap();
    /// assert_eq!(result.to_string(), "true");
    /// ```
    pub fn evaluate<'a>(
        &'a self,
        rule: &'a Logic,
        data: &'a DataValue,
    ) -> Result<&'a DataValue<'a>> {
        // Set both current context and root context to the data
        self.arena.set_root_context(data);
        self.arena
            .set_current_context(data, &DataValue::String("$"));

        // Evaluate the rule with the data as context
        evaluate(rule.root(), &self.arena)
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
    /// * `format` - Optional format name for the parser to use
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
    /// let result = dl.evaluate_json(&logic, &data, None).unwrap();
    /// assert_eq!(result.as_i64().unwrap(), 4);
    /// ```
    pub fn evaluate_json(
        &self,
        logic: &JsonValue,
        data: &JsonValue,
        format: Option<&str>,
    ) -> Result<JsonValue> {
        let rule = self.parse_logic_json(logic, format)?;
        let data_value = self.parse_data_json(data)?;
        let result = self.evaluate(&rule, &data_value)?;
        Ok(result.to_json())
    }

    /// Parse and evaluate in one step, returning a JSON value
    pub fn evaluate_str(
        &self,
        logic_source: &str,
        data_source: &str,
        format: Option<&str>,
    ) -> Result<JsonValue> {
        let rule = self.parse_logic(logic_source, format)?;
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
    ///     None
    /// ).unwrap();
    ///
    /// assert_eq!(result.as_f64().unwrap(), 10.0);
    ///
    /// // Use the custom operator with data context
    /// let result = dl.evaluate_str(
    ///     r#"{"double": []}"#,
    ///     r#"{"value": 7}"#,
    ///     None
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
            .evaluate_json(&json!({"multiply_all": [2, 3, 4]}), &json!({}), None)
            .unwrap();

        assert_eq!(result.as_f64().unwrap(), 24.0);

        // Test with string values
        let result = dl
            .evaluate_str(r#"{"multiply_all": [2, 3, 4]}"#, r#"{}"#, None)
            .unwrap();

        assert_eq!(result.as_f64().unwrap(), 24.0);
    }
}
