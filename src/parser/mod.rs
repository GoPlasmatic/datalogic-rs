use crate::arena::DataArena;
use crate::logic::{LogicError, Result, Token};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub mod jsonlogic;
#[cfg(test)]
mod tests;

/// Trait that defines a parser for an expression language
pub trait ExpressionParser: Send + Sync {
    /// Parse the input string into a Token
    fn parse<'a>(&self, input: &str, arena: &'a DataArena) -> Result<&'a Token<'a>>;

    /// Parse the input JSON into a Token
    fn parse_json<'a>(&self, input: &JsonValue, arena: &'a DataArena) -> Result<&'a Token<'a>>;

    /// Parse the input string into a Token with structure preservation option
    fn parse_with_preserve<'a>(
        &self,
        input: &str,
        arena: &'a DataArena,
        preserve_structure: bool,
    ) -> Result<&'a Token<'a>>;

    /// Parse the input JSON into a Token with structure preservation option
    fn parse_json_with_preserve<'a>(
        &self,
        input: &JsonValue,
        arena: &'a DataArena,
        preserve_structure: bool,
    ) -> Result<&'a Token<'a>>;

    /// Get the name of this parser format
    fn format_name(&self) -> &'static str;
}

/// Registry that manages parsers
pub struct ParserRegistry {
    parsers: HashMap<String, Box<dyn ExpressionParser>>,
    default_parser: String,
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserRegistry {
    /// Create a new parser registry with JSONLogic as the default parser
    pub fn new() -> Self {
        let mut registry = Self {
            parsers: HashMap::new(),
            default_parser: "jsonlogic".to_string(),
        };

        // Register the default JSONLogic parser
        registry.register(Box::new(jsonlogic::JsonLogicParser));

        registry
    }

    /// Register a new parser
    pub fn register(&mut self, parser: Box<dyn ExpressionParser>) {
        let name = parser.format_name().to_string();
        self.parsers.insert(name, parser);
    }

    /// Set the default parser
    pub fn set_default(&mut self, format_name: &str) -> Result<()> {
        if self.parsers.contains_key(format_name) {
            self.default_parser = format_name.to_string();
            Ok(())
        } else {
            Err(LogicError::ParseError {
                reason: format!("Unknown parser format: {format_name}"),
            })
        }
    }

    /// Parse an expression using the specified parser, or default if none specified
    pub fn parse<'a>(
        &self,
        input: &str,
        format: Option<&str>,
        arena: &'a DataArena,
    ) -> Result<&'a Token<'a>> {
        let format = format.unwrap_or(&self.default_parser);

        if let Some(parser) = self.parsers.get(format) {
            parser.parse(input, arena)
        } else {
            Err(LogicError::ParseError {
                reason: format!("Unknown parser format: {format}"),
            })
        }
    }

    pub fn parse_json<'a>(
        &self,
        input: &JsonValue,
        format: Option<&str>,
        arena: &'a DataArena,
    ) -> Result<&'a Token<'a>> {
        let format = format.unwrap_or(&self.default_parser);
        let parser = self.parsers.get(format).ok_or(LogicError::ParseError {
            reason: format!("Unknown parser format: {format}"),
        })?;
        parser.parse_json(input, arena)
    }

    /// Parse an expression with structure preservation using the specified parser
    pub fn parse_with_preserve<'a>(
        &self,
        input: &str,
        format: Option<&str>,
        arena: &'a DataArena,
        preserve_structure: bool,
    ) -> Result<&'a Token<'a>> {
        let format = format.unwrap_or(&self.default_parser);

        if let Some(parser) = self.parsers.get(format) {
            parser.parse_with_preserve(input, arena, preserve_structure)
        } else {
            Err(LogicError::ParseError {
                reason: format!("Unknown parser format: {format}"),
            })
        }
    }

    /// Parse a JSON expression with structure preservation using the specified parser
    pub fn parse_json_with_preserve<'a>(
        &self,
        input: &JsonValue,
        format: Option<&str>,
        arena: &'a DataArena,
        preserve_structure: bool,
    ) -> Result<&'a Token<'a>> {
        let format = format.unwrap_or(&self.default_parser);
        let parser = self.parsers.get(format).ok_or(LogicError::ParseError {
            reason: format!("Unknown parser format: {format}"),
        })?;
        parser.parse_json_with_preserve(input, arena, preserve_structure)
    }
}
