//! Examples of using the DataLogic library
//!
//! This module contains examples of how to use the DataLogic library with different parsers.

use crate::parser::jsonata::JsonataParser;
use crate::DataLogic;
use serde_json::json;

/// Example of using the default JSONLogic parser
pub fn jsonlogic_example() -> serde_json::Value {
    // Create a new DataLogic instance (uses JSONLogic by default)
    let logic = DataLogic::new();

    // Data to evaluate against
    let data = r#"{"user": {"name": "John", "score": 75, "verified": true}}"#;

    // Rule in JSONLogic format
    let rule = r#"{"and": [
        {">": [{"var": "user.score"}, 70]},
        {"var": "user.verified"}
    ]}"#;

    // Parse and evaluate the rule
    match logic.apply(rule, data, None) {
        Ok(result) => result,
        Err(e) => json!({"error": e.to_string()}),
    }
}

/// Example of using multiple parsers
pub fn multiple_parsers_example() -> serde_json::Value {
    // Create a new DataLogic instance
    let mut logic = DataLogic::new();

    // Register the JSONata parser (currently a placeholder)
    logic.register_parser(Box::new(JsonataParser));

    // Data to evaluate against
    let data = r#"{"user": {"name": "John", "score": 75, "verified": true}}"#;

    // Rule in JSONLogic format
    let jsonlogic_rule = r#"{"and": [
        {">": [{"var": "user.score"}, 70]},
        {"var": "user.verified"}
    ]}"#;

    // Rule in JSONata format (syntax is different)
    // Note: This is just for demonstration, as the JSONata parser is not yet implemented
    let jsonata_rule = "user.score > 70 and user.verified";

    // Results collection
    let mut results = json!({});

    // Try both parsers
    match logic.apply(jsonlogic_rule, data, Some("jsonlogic")) {
        Ok(result) => {
            results["jsonlogic"] = result;
        }
        Err(e) => {
            results["jsonlogic_error"] = json!(e.to_string());
        }
    }

    match logic.apply(jsonata_rule, data, Some("jsonata")) {
        Ok(result) => {
            results["jsonata"] = result;
        }
        Err(e) => {
            results["jsonata_error"] = json!(e.to_string());
        }
    }

    results
}
