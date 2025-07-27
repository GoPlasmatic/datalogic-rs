//! Tests for the Parser Registry and parsers
//!
//! This module contains tests for the parser registry and the included parsers.

use crate::arena::DataArena;
use crate::logic::{ComparisonOp, OperatorType, Token};
use crate::parser::{ExpressionParser, ParserRegistry};
use serde_json::Value as JsonValue;

#[test]
fn test_parser_registry_creation() {
    let registry = ParserRegistry::new();

    // Default parser should be jsonlogic
    let arena = DataArena::new();
    let json_str = r#"{"==": [{"var": "a"}, 42]}"#;

    // Parse with default parser
    let token = registry.parse(json_str, None, &arena).unwrap();

    // Verify the token
    assert!(token.is_operator());
    let (op_type, _) = token.as_operator().unwrap();
    assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
}

#[test]
fn test_parser_registry_with_specified_parser() {
    let registry = ParserRegistry::new();
    let arena = DataArena::new();
    let json_str = r#"{"==": [{"var": "a"}, 42]}"#;

    // Parse with explicitly specified parser
    let token = registry.parse(json_str, Some("jsonlogic"), &arena).unwrap();

    // Verify the token
    assert!(token.is_operator());
    let (op_type, _) = token.as_operator().unwrap();
    assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
}

#[test]
fn test_parser_registry_with_invalid_parser() {
    let registry = ParserRegistry::new();
    let arena = DataArena::new();
    let json_str = r#"{"==": [{"var": "a"}, 42]}"#;

    // Parse with non-existent parser
    let result = registry.parse(json_str, Some("not_exists"), &arena);
    assert!(result.is_err());
}

// This is a mock parser for testing purposes
struct MockParser;

impl ExpressionParser for MockParser {
    fn parse<'a>(&self, _input: &str, arena: &'a DataArena) -> crate::logic::Result<&'a Token<'a>> {
        // Always returns a literal token with the value "mock"
        Ok(arena.alloc(Token::literal(crate::value::DataValue::string(
            arena, "mock",
        ))))
    }

    fn parse_json<'a>(
        &self,
        _input: &JsonValue,
        arena: &'a DataArena,
    ) -> crate::logic::Result<&'a Token<'a>> {
        Ok(arena.alloc(Token::literal(crate::value::DataValue::string(
            arena, "mock",
        ))))
    }

    fn parse_with_preserve<'a>(
        &self,
        _input: &str,
        arena: &'a DataArena,
        _preserve_structure: bool,
    ) -> crate::logic::Result<&'a Token<'a>> {
        // Always returns a literal token with the value "mock"
        Ok(arena.alloc(Token::literal(crate::value::DataValue::string(
            arena, "mock",
        ))))
    }

    fn parse_json_with_preserve<'a>(
        &self,
        _input: &JsonValue,
        arena: &'a DataArena,
        _preserve_structure: bool,
    ) -> crate::logic::Result<&'a Token<'a>> {
        Ok(arena.alloc(Token::literal(crate::value::DataValue::string(
            arena, "mock",
        ))))
    }

    fn format_name(&self) -> &'static str {
        "mock"
    }
}

#[test]
fn test_multiple_parsers() {
    let mut registry = ParserRegistry::new();
    let arena = DataArena::new();

    // Register the mock parser
    registry.register(Box::new(MockParser));

    // Parse with both parsers
    let json_str = r#"{"==": [{"var": "a"}, 42]}"#;

    // JSONLogic parser should return an operator
    let jsonlogic_token = registry.parse(json_str, Some("jsonlogic"), &arena).unwrap();
    assert!(jsonlogic_token.is_operator());

    // Mock parser should return a literal "mock"
    let mock_token = registry.parse(json_str, Some("mock"), &arena).unwrap();
    assert!(mock_token.is_literal());
    assert_eq!(mock_token.as_literal().unwrap().as_str(), Some("mock"));
}
