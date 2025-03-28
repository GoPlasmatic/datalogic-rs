//! Main interface for the DataLogic library
//!
//! This module provides the DataLogic struct which is the primary entry point
//! for parsing and evaluating logic expressions.

use crate::arena::DataArena;
use crate::logic::{evaluate, optimize, Logic, Result};
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

    /// Parse a JSON data string into a DataValue
    pub fn parse_data(&self, source: &str) -> Result<DataValue> {
        let json = serde_json::from_str(source).map_err(|e| LogicError::ParseError {
            reason: e.to_string(),
        })?;
        Ok(DataValue::from_json(&json, &self.arena))
    }

    /// Evaluate a rule with the provided data
    pub fn evaluate<'a>(
        &'a self,
        rule: &'a Logic,
        data: &'a DataValue,
    ) -> Result<&'a DataValue<'a>> {
        evaluate(rule.root(), data, &self.arena)
    }

    /// Parse and evaluate in one step, returning JSON
    pub fn apply(
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

    /// Parse and evaluate in one step, returning a DataValue
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
