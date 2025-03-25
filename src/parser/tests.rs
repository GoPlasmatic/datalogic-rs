//! Tests for the Parser Registry and parsers
//!
//! This module contains tests for the parser registry and the included parsers.

#[cfg(test)]
mod tests {
    use crate::arena::DataArena;
    use crate::logic::{ComparisonOp, OperatorType, Token};
    use crate::parser::{ExpressionParser, ParserRegistry};
    use crate::parser::jsonata::JsonataParser;
    use crate::parser::jsonlogic::JsonLogicParser;

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
    fn test_register_custom_parser() {
        let mut registry = ParserRegistry::new();
        let arena = DataArena::new();
        
        // Register the JSONata parser
        registry.register(Box::new(JsonataParser));
        
        // Parse with JSONata parser (should fail as it's not implemented yet)
        let result = registry.parse("a > 10", Some("jsonata"), &arena);
        assert!(result.is_err());
        
        // Parse with JSONLogic parser should still work
        let json_str = r#"{"==": [{"var": "a"}, 42]}"#;
        let token = registry.parse(json_str, Some("jsonlogic"), &arena).unwrap();
        
        // Verify the token
        assert!(token.is_operator());
        let (op_type, _) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
    }
    
    #[test]
    fn test_change_default_parser() {
        let mut registry = ParserRegistry::new();
        
        // Register the JSONata parser
        registry.register(Box::new(JsonataParser));
        
        // Change default parser to JSONata
        let result = registry.set_default("jsonata");
        assert!(result.is_ok());
        
        // Try to set default to non-existent parser
        let result = registry.set_default("not_exists");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_parser_interface() {
        // Test the JsonLogicParser
        let jsonlogic_parser = JsonLogicParser;
        assert_eq!(jsonlogic_parser.format_name(), "jsonlogic");
        
        // Test the JsonataParser
        let jsonata_parser = JsonataParser;
        assert_eq!(jsonata_parser.format_name(), "jsonata");
        
        // Parse with both parsers
        let arena = DataArena::new();
        
        // JSONLogic parsing should succeed
        let json_str = r#"{"==": [{"var": "a"}, 42]}"#;
        let jsonlogic_result = jsonlogic_parser.parse(json_str, &arena);
        assert!(jsonlogic_result.is_ok());
        
        // JSONata parsing should fail (as it's not implemented yet)
        let jsonata_str = "a == 42";
        let jsonata_result = jsonata_parser.parse(jsonata_str, &arena);
        assert!(jsonata_result.is_err());
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
            Ok(arena.alloc(Token::literal(crate::value::DataValue::string(arena, "mock"))))
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
} 