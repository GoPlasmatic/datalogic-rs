#![cfg(target_arch = "wasm32")]

use datalogic_wasm::{CompiledRule, evaluate};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_evaluate_simple() {
    let result = evaluate(r#"{"==": [1, 1]}"#, "{}").unwrap();
    assert_eq!(result, "true");
}

#[wasm_bindgen_test]
fn test_evaluate_with_data() {
    let result = evaluate(r#"{"var": "x"}"#, r#"{"x": 42}"#).unwrap();
    assert_eq!(result, "42");
}

#[wasm_bindgen_test]
fn test_evaluate_arithmetic() {
    let result = evaluate(r#"{"+": [2, 3]}"#, "{}").unwrap();
    assert_eq!(result, "5");
}

#[wasm_bindgen_test]
fn test_evaluate_comparison() {
    let result = evaluate(r#"{">": [5, 3]}"#, "{}").unwrap();
    assert_eq!(result, "true");
}

#[wasm_bindgen_test]
fn test_evaluate_array_operations() {
    let result = evaluate(r#"{"map": [[1, 2, 3], {"*": [{"var": ""}, 2]}]}"#, "{}").unwrap();
    assert_eq!(result, "[2,4,6]");
}

#[wasm_bindgen_test]
fn test_evaluate_conditional() {
    let result = evaluate(
        r#"{"if": [{"var": "active"}, "yes", "no"]}"#,
        r#"{"active": true}"#,
    )
    .unwrap();
    assert_eq!(result, "\"yes\"");
}

#[wasm_bindgen_test]
fn test_compiled_rule() {
    let rule = CompiledRule::new(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#).unwrap();

    let result1 = rule.evaluate(r#"{"a": 1, "b": 2}"#).unwrap();
    assert_eq!(result1, "3");

    let result2 = rule.evaluate(r#"{"a": 10, "b": 20}"#).unwrap();
    assert_eq!(result2, "30");
}

#[wasm_bindgen_test]
fn test_invalid_json_logic() {
    let result = evaluate("not valid json", "{}");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_invalid_data() {
    let result = evaluate(r#"{"var": "x"}"#, "not valid json");
    assert!(result.is_err());
}
