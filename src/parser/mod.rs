use crate::arena::DataArena;
use crate::logic::{LogicError, Result, Token};
use std::collections::HashMap;

pub mod jsonata;
pub mod jsonlogic;
#[cfg(test)]
mod tests;

/// Trait that defines a parser for an expression language
pub trait ExpressionParser {
    /// Parse the input string into a Token
    fn parse<'a>(&self, input: &str, arena: &'a DataArena) -> Result<&'a Token<'a>>;

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
                reason: format!("Unknown parser format: {}", format_name),
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
                reason: format!("Unknown parser format: {}", format),
            })
        }
    }
}
