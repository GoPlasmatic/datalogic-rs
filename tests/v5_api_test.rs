//! End-to-end smoke tests for the v5 public surface.
//!
//! Exercises only the v5 entry points (`DataLogicBuilder`, `compile_str`,
//! `evaluate_str`, `evaluate_logic_str`, `evaluate_value`). The old 4.x
//! methods live behind `crate::compat` and have their own coverage in
//! `arena_operator_test.rs` / `error_serialization.rs`.

use bumpalo::Bump;
use datalogic_rs::{DataLogic, DataValue, OwnedDataValue};

#[test]
fn builder_default_engine() {
    let engine = DataLogic::builder().build();
    let result = engine
        .evaluate_logic_str(r#"{"+": [1, 2, 3]}"#, "null")
        .unwrap();
    assert_eq!(result, "6");
}

#[test]
fn evaluate_str_with_variable() {
    let engine = DataLogic::new();
    let compiled = engine.compile_str(r#"{"var": "name"}"#).unwrap();
    let result = engine
        .evaluate_str(&compiled, r#"{"name": "Alice"}"#)
        .unwrap();
    assert_eq!(result, "\"Alice\"");
}

#[test]
fn evaluate_value_arena_path() {
    let engine = DataLogic::new();
    let compiled = engine.compile_str(r#"{">": [{"var": "n"}, 5]}"#).unwrap();
    let arena = Bump::new();
    let data = DataValue::from_str(r#"{"n": 42}"#, &arena).unwrap();
    let data_ref = arena.alloc(data);
    let result = engine.evaluate_value(&compiled, data_ref, &arena).unwrap();
    assert_eq!(result.as_bool(), Some(true));
}

#[test]
fn compile_value_from_owned() {
    let engine = DataLogic::new();
    let owned = OwnedDataValue::from_json(r#"{"==": [1, 1]}"#).unwrap();
    let compiled = engine.compile_value(&owned).unwrap();
    let result = engine.evaluate_str(&compiled, "null").unwrap();
    assert_eq!(result, "true");
}

#[test]
fn complex_object_round_trip() {
    let engine = DataLogic::new();
    let compiled = engine
        .compile_str(r#"{"if": [{">": [{"var": "score"}, 80]}, "pass", "fail"]}"#)
        .unwrap();
    let pass = engine.evaluate_str(&compiled, r#"{"score": 95}"#).unwrap();
    let fail = engine.evaluate_str(&compiled, r#"{"score": 50}"#).unwrap();
    assert_eq!(pass, "\"pass\"");
    assert_eq!(fail, "\"fail\"");
}

#[test]
fn datavalue_object_returned_as_json_string() {
    let engine = DataLogic::new();
    let compiled = engine
        .compile_str(r#"{"merge": [[1, 2], [3, 4]]}"#)
        .unwrap();
    let result = engine.evaluate_str(&compiled, "null").unwrap();
    assert_eq!(result, "[1,2,3,4]");
}
