//! Tests for the JSONLogic parser
//!
//! This module contains tests for the JSONLogic parser functions.

use crate::arena::{CustomOperatorRegistry, DataArena};
use crate::logic::{ComparisonOp, OperatorType};
use crate::parser::{
    parse_jsonlogic, parse_jsonlogic_json, parse_jsonlogic_json_with_preserve,
    parse_jsonlogic_with_preserve,
};
use serde_json::{Value as JsonValue, json};
use std::sync::LazyLock;

// Static empty operator registry for tests
static EMPTY_OPERATORS: LazyLock<CustomOperatorRegistry> =
    LazyLock::new(CustomOperatorRegistry::new);

#[test]
fn test_parse_jsonlogic_string() {
    let arena = DataArena::new();
    let json_str = r#"{"==": [{"var": "a"}, 42]}"#;

    // Parse the JSONLogic expression
    let token = parse_jsonlogic(json_str, &arena, &EMPTY_OPERATORS).unwrap();

    // Verify the token
    assert!(token.is_operator());
    let (op_type, _) = token.as_operator().unwrap();
    assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
}

#[test]
fn test_parse_jsonlogic_json_value() {
    let arena = DataArena::new();
    let json_value: JsonValue = json!({"==": [{"var": "a"}, 42]});

    // Parse the JSONLogic expression from JsonValue
    let token = parse_jsonlogic_json(&json_value, &arena, &EMPTY_OPERATORS).unwrap();

    // Verify the token
    assert!(token.is_operator());
    let (op_type, _) = token.as_operator().unwrap();
    assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
}

#[test]
fn test_parse_jsonlogic_with_preserve() {
    let arena = DataArena::new();

    // Test with preserve_structure = false (should error on multi-key object)
    let json_str = r#"{"result": true, "count": 3}"#;
    let result = parse_jsonlogic_with_preserve(json_str, &arena, false, &EMPTY_OPERATORS);
    assert!(result.is_err());

    // Test with preserve_structure = true (should create structured object)
    let token = parse_jsonlogic_with_preserve(json_str, &arena, true, &EMPTY_OPERATORS).unwrap();
    assert!(token.is_structured_object());
}

#[test]
fn test_parse_jsonlogic_json_with_preserve() {
    let arena = DataArena::new();
    let json_value: JsonValue = json!({"result": {"==": [1, 1]}, "count": {"+": [1, 2]}});

    // Test with preserve_structure = false (should error)
    let result = parse_jsonlogic_json_with_preserve(&json_value, &arena, false, &EMPTY_OPERATORS);
    assert!(result.is_err());

    // Test with preserve_structure = true (should create structured object)
    let token =
        parse_jsonlogic_json_with_preserve(&json_value, &arena, true, &EMPTY_OPERATORS).unwrap();
    assert!(token.is_structured_object());
}

#[test]
fn test_parse_invalid_json() {
    let arena = DataArena::new();
    let invalid_json = r#"{"==": [{"var": "a"}, 42"#; // Missing closing braces

    let result = parse_jsonlogic(invalid_json, &arena, &EMPTY_OPERATORS);
    assert!(result.is_err());
}

#[test]
fn test_parse_literals() {
    let arena = DataArena::new();

    // Test parsing various literals
    let test_cases = vec![
        (r#"null"#, true),
        (r#"true"#, true),
        (r#"42"#, true),
        (r#"3.14"#, true),
        (r#""hello""#, true),
        (r#"[1, 2, 3]"#, true),
    ];

    for (json_str, should_be_literal) in test_cases {
        let token = parse_jsonlogic(json_str, &arena, &EMPTY_OPERATORS).unwrap();
        assert_eq!(
            token.is_literal(),
            should_be_literal,
            "Failed for: {}",
            json_str
        );
    }
}

#[test]
fn test_parse_operators() {
    let arena = DataArena::new();

    // Test parsing various operators
    let test_cases = vec![
        r#"{"==": [1, 1]}"#,
        r#"{"+": [1, 2, 3]}"#,
        r#"{"and": [true, false]}"#,
        r#"{"if": [true, "yes", "no"]}"#,
        r#"{"var": "name"}"#,
    ];

    for json_str in test_cases {
        let token = parse_jsonlogic(json_str, &arena, &EMPTY_OPERATORS).unwrap();
        assert!(
            token.is_operator() || token.is_variable(),
            "Failed for: {}",
            json_str
        );
    }
}
