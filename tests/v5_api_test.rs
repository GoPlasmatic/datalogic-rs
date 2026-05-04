//! End-to-end smoke tests for the v5 public surface.
//!
//! Exercises only the v5 entry points (`DataLogicBuilder`, `compile`,
//! `evaluate`, `evaluate_str`, `evaluate_value`). The 4.x compat methods
//! live behind `crate::compat` and have their own coverage in
//! `arena_operator_test.rs` / `error_serialization.rs`.

use bumpalo::Bump;
use datalogic_rs::{DataLogic, DataValue};

#[test]
fn builder_default_engine() {
    let engine = DataLogic::builder().build();
    let result = engine.evaluate_str(r#"{"+": [1, 2, 3]}"#, "null").unwrap();
    assert_eq!(result, "6");
}

#[test]
fn evaluate_str_one_shot_with_variable() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_str(r#"{"var": "name"}"#, r#"{"name": "Alice"}"#)
        .unwrap();
    assert_eq!(result, "\"Alice\"");
}

#[test]
fn evaluate_arena_path() {
    let engine = DataLogic::new();
    let compiled = engine.compile(r#"{">": [{"var": "n"}, 5]}"#).unwrap();
    let arena = Bump::new();
    let data = DataValue::from_str(r#"{"n": 42}"#, &arena).unwrap();
    let result = engine
        .evaluate(&compiled, arena.alloc(data), &arena)
        .unwrap();
    assert_eq!(result.as_bool(), Some(true));
}

#[test]
fn compile_then_evaluate_str_round_trip() {
    let engine = DataLogic::new();
    let compiled = engine.compile(r#"{"==": [1, 1]}"#).unwrap();
    let arena = Bump::new();
    let data = DataValue::from_str("null", &arena).unwrap();
    let result = engine
        .evaluate(&compiled, arena.alloc(data), &arena)
        .unwrap();
    assert_eq!(result.as_bool(), Some(true));
}

#[test]
fn compile_once_evaluate_many_arena_reuse() {
    let engine = DataLogic::new();
    let compiled = engine
        .compile(r#"{"if": [{">": [{"var": "score"}, 80]}, "pass", "fail"]}"#)
        .unwrap();

    let mut arena = Bump::new();
    for (input, expected) in [(r#"{"score": 95}"#, "pass"), (r#"{"score": 50}"#, "fail")] {
        let data = DataValue::from_str(input, &arena).unwrap();
        let result = engine
            .evaluate(&compiled, arena.alloc(data), &arena)
            .unwrap();
        assert_eq!(result.as_str(), Some(expected));
        arena.reset();
    }
}

#[test]
fn datavalue_object_returned_as_json_string() {
    let engine = DataLogic::new();
    let result = engine
        .evaluate_str(r#"{"merge": [[1, 2], [3, 4]]}"#, "null")
        .unwrap();
    assert_eq!(result, "[1,2,3,4]");
}

#[test]
fn evaluate_value_serde_one_shot() {
    use serde_json::json;
    let engine = DataLogic::new();
    let logic = json!({"+": [{"var": "a"}, {"var": "b"}]});
    let data = json!({"a": 2, "b": 3});
    let result = engine.evaluate_value(&logic, &data).unwrap();
    assert_eq!(result, json!(5));
}
