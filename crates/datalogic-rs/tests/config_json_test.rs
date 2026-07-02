//! Tests for `EvaluationConfig::from_json_str` — the FFI config wire
//! format shared by the language bindings.

#![cfg(feature = "serde_json")]

use datalogic_rs::{Engine, EvaluationConfig};

fn engine_with(json: &str) -> Engine {
    let config = EvaluationConfig::from_json_str(json).expect("config should parse");
    Engine::builder().with_config(config).build()
}

#[test]
fn empty_object_is_default() {
    let engine = engine_with("{}");
    // Default division-by-zero saturates instead of erroring (the float
    // path — an integer dividend over integer zero always errors).
    assert!(engine.eval_str(r#"{"/": [1.5, 0]}"#, "null").is_ok());
}

#[test]
fn preset_strict_rejects_coercion() {
    let engine = engine_with(r#"{"preset": "strict"}"#);
    // Strict mode: null raises instead of coercing to 0. (A numeric
    // string like "1" still parses — reject_non_numeric only rejects
    // fabricated coercions.)
    assert!(engine.eval_str(r#"{"+": [null, 1]}"#, "null").is_err());
}

#[test]
fn preset_safe_arithmetic_ignores_bad_values() {
    let engine = engine_with(r#"{"preset": "safe_arithmetic"}"#);
    let result = engine
        .eval_str(r#"{"+": [1, "skipped", 2]}"#, "null")
        .unwrap();
    assert_eq!(result, "3");
}

#[test]
fn overrides_apply_on_top_of_preset() {
    // Strict preset, but division_by_zero softened back to null.
    let engine = engine_with(r#"{"preset": "strict", "division_by_zero": "return_null"}"#);
    assert_eq!(engine.eval_str(r#"{"/": [1, 0.5]}"#, "null").unwrap(), "2");
    assert_eq!(
        engine.eval_str(r#"{"/": [1.5, 0]}"#, "null").unwrap(),
        "null"
    );
}

#[test]
fn numeric_coercion_partial_object_merges() {
    // Only null_to_zero flipped; empty_string_to_zero keeps its default.
    let engine = engine_with(r#"{"numeric_coercion": {"null_to_zero": false}}"#);
    assert!(engine.eval_str(r#"{"+": [null, 1]}"#, "null").is_err());
    assert_eq!(engine.eval_str(r#"{"+": ["", 1]}"#, "null").unwrap(), "1");
}

#[test]
fn division_by_zero_throw_error() {
    let engine = engine_with(r#"{"division_by_zero": "throw_error"}"#);
    // 1.5 keeps us on the configurable float path.
    assert!(engine.eval_str(r#"{"/": [1.5, 0]}"#, "null").is_err());
}

#[test]
fn truthy_evaluator_strict_boolean() {
    // Under strict-boolean truthiness only null/false are falsy, so 0 is
    // truthy — the opposite of the JavaScript default.
    let engine = engine_with(r#"{"truthy_evaluator": "strict_boolean"}"#);
    assert_eq!(
        engine
            .eval_str(r#"{"if": [0, "yes", "no"]}"#, "null")
            .unwrap(),
        "\"yes\""
    );
    let default_engine = engine_with("{}");
    assert_eq!(
        default_engine
            .eval_str(r#"{"if": [0, "yes", "no"]}"#, "null")
            .unwrap(),
        "\"no\""
    );
}

#[test]
fn arithmetic_nan_handling_return_null() {
    let engine = engine_with(r#"{"arithmetic_nan_handling": "return_null"}"#);
    assert_eq!(
        engine
            .eval_str(r#"{"+": [1, {"map": [[], 1]}]}"#, "null")
            .unwrap(),
        "null"
    );
}

#[test]
fn max_recursion_depth_is_applied() {
    // Depth 1 with a re-entrant custom operator would trip immediately;
    // here we just verify the value parses and the engine still works.
    let engine = engine_with(r#"{"max_recursion_depth": 1}"#);
    assert_eq!(engine.eval_str(r#"{"+": [1, 2]}"#, "null").unwrap(), "3");
}

// --- rejection cases ---

fn parse_err(json: &str) -> String {
    EvaluationConfig::from_json_str(json)
        .expect_err("config should be rejected")
        .to_string()
}

#[test]
fn rejects_non_object() {
    assert!(parse_err("[]").contains("must be a JSON object"));
    assert!(parse_err("not json").contains("not valid JSON"));
}

#[test]
fn rejects_unknown_key() {
    assert!(parse_err(r#"{"divison_by_zero": "return_null"}"#).contains("unknown config key"));
}

#[test]
fn rejects_unknown_enum_value() {
    assert!(parse_err(r#"{"division_by_zero": "explode"}"#).contains("unknown division_by_zero"));
    assert!(parse_err(r#"{"preset": "fast"}"#).contains("unknown preset"));
    assert!(parse_err(r#"{"truthy_evaluator": "ruby"}"#).contains("unknown truthy_evaluator"));
}

#[test]
fn rejects_type_mismatches() {
    assert!(parse_err(r#"{"loose_equality_errors": "yes"}"#).contains("must be a boolean"));
    assert!(parse_err(r#"{"arithmetic_nan_handling": 3}"#).contains("must be a string"));
    assert!(parse_err(r#"{"numeric_coercion": true}"#).contains("must be an object"));
    assert!(parse_err(r#"{"max_recursion_depth": 0}"#).contains("between 1 and"));
    assert!(parse_err(r#"{"max_recursion_depth": -4}"#).contains("between 1 and"));
}

#[test]
fn rejects_removed_undefined_to_zero_key() {
    // The knob never worked and was removed from the Rust API; the wire
    // format treats it like any other unknown key.
    assert!(
        parse_err(r#"{"numeric_coercion": {"undefined_to_zero": true}}"#)
            .contains("unknown numeric_coercion key")
    );
}
