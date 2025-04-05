//! Main interface for the DataLogic library
//!
//! This module provides the DataLogic struct which is the primary entry point
//! for parsing and evaluating logic expressions.

use crate::arena::DataArena;
use crate::logic::{Logic, Result, evaluate, optimize};
use crate::parser::{ExpressionParser, ParserRegistry};
use crate::value::{DataValue, FromJson, ToJson};
use crate::{LogicError, RuleBuilder};
use serde_json::Value as JsonValue;

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
}

impl DataLogic {
    /// Create a new DataLogic instance with default settings
    pub fn new() -> Self {
        Self {
            arena: DataArena::new(),
            parsers: ParserRegistry::new(),
        }
    }

    /// Create a new DataLogic instance with a specific chunk size for the arena
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            arena: DataArena::with_chunk_size(chunk_size),
            parsers: ParserRegistry::new(),
        }
    }

    /// Get a reference to the internal arena
    ///
    /// This is exposed for advanced usage scenarios, but most users
    /// won't need to access this directly.
    pub fn arena(&self) -> &DataArena {
        &self.arena
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

    /// Get a rule builder for constructing rules programmatically
    pub fn builder(&self) -> RuleBuilder {
        RuleBuilder::new(&self.arena)
    }

    /// Parse a logic expression using the specified parser format
    pub fn parse_logic(&self, source: &str, format: Option<&str>) -> Result<Logic> {
        let token = self.parsers.parse(source, format, &self.arena)?;

        // Apply static optimization
        let optimized_token = optimize(token, &self.arena)?;

        Ok(Logic::new(optimized_token, &self.arena))
    }

    /// Parse a JSON logic expression into a Token
    pub fn parse_logic_json(&self, source: &JsonValue, format: Option<&str>) -> Result<Logic> {
        let token = self.parsers.parse_json(source, format, &self.arena)?;
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
}

impl Default for DataLogic {
    fn default() -> Self {
        Self::new()
    }
}
