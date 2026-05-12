#![cfg(target_arch = "wasm32")]

use datalogic_wasm::{CompiledRule, Engine, evaluate};
use js_sys::{Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_evaluate_simple() {
    let result = evaluate(r#"{"==": [1, 1]}"#, "{}", false).unwrap();
    assert_eq!(result, "true");
}

#[wasm_bindgen_test]
fn test_evaluate_with_data() {
    let result = evaluate(r#"{"var": "x"}"#, r#"{"x": 42}"#, false).unwrap();
    assert_eq!(result, "42");
}

#[wasm_bindgen_test]
fn test_evaluate_arithmetic() {
    let result = evaluate(r#"{"+": [2, 3]}"#, "{}", false).unwrap();
    assert_eq!(result, "5");
}

#[wasm_bindgen_test]
fn test_evaluate_comparison() {
    let result = evaluate(r#"{">": [5, 3]}"#, "{}", false).unwrap();
    assert_eq!(result, "true");
}

#[wasm_bindgen_test]
fn test_evaluate_array_operations() {
    let result = evaluate(
        r#"{"map": [[1, 2, 3], {"*": [{"var": ""}, 2]}]}"#,
        "{}",
        false,
    )
    .unwrap();
    assert_eq!(result, "[2,4,6]");
}

#[wasm_bindgen_test]
fn test_evaluate_conditional() {
    let result = evaluate(
        r#"{"if": [{"var": "active"}, "yes", "no"]}"#,
        r#"{"active": true}"#,
        false,
    )
    .unwrap();
    assert_eq!(result, "\"yes\"");
}

#[wasm_bindgen_test]
fn test_compiled_rule() {
    let rule = CompiledRule::new(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#, false).unwrap();

    let result1 = rule.evaluate(r#"{"a": 1, "b": 2}"#).unwrap();
    assert_eq!(result1, "3");

    let result2 = rule.evaluate(r#"{"a": 10, "b": 20}"#).unwrap();
    assert_eq!(result2, "30");
}

#[wasm_bindgen_test]
fn test_invalid_json_logic() {
    let result = evaluate("not valid json", "{}", false);
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_invalid_data() {
    let result = evaluate(r#"{"var": "x"}"#, "not valid json", false);
    assert!(result.is_err());
}

// =============== Custom operator tests ===============

/// Build an options bag `{ templating, customOperators: { name: fn } }`
/// from a `(name, fn)` slice.
fn build_options(templating: bool, ops: &[(&str, Function)]) -> JsValue {
    let obj = Object::new();
    Reflect::set(
        &obj,
        &JsValue::from_str("templating"),
        &JsValue::from_bool(templating),
    )
    .unwrap();
    if !ops.is_empty() {
        let map = Object::new();
        for (name, f) in ops {
            Reflect::set(&map, &JsValue::from_str(name), f.as_ref()).unwrap();
        }
        Reflect::set(&obj, &JsValue::from_str("customOperators"), map.as_ref()).unwrap();
    }
    obj.into()
}

#[wasm_bindgen_test]
fn test_engine_no_custom_ops() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine.compile(r#"{"+": [1, 2]}"#).unwrap();
    assert_eq!(rule.evaluate("{}").unwrap(), "3");
}

#[wasm_bindgen_test]
fn test_engine_simple_custom_op() {
    // Custom `double` operator — argsJson is "[n]"; return JSON string of n*2.
    let closure = Closure::wrap(Box::new(|args_json: JsValue| -> JsValue {
        let s = args_json.as_string().unwrap_or_default();
        // s is a JSON array string like "[21]"
        let n: f64 = s
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse()
            .unwrap();
        JsValue::from_str(&format!("{}", n * 2.0))
    }) as Box<dyn FnMut(JsValue) -> JsValue>);
    let func: Function = closure.as_ref().unchecked_ref::<Function>().clone();

    let opts = build_options(false, &[("double", func)]);
    let engine = Engine::new(opts).unwrap();
    let rule = engine.compile(r#"{"double": [21]}"#).unwrap();
    assert_eq!(rule.evaluate("{}").unwrap(), "42");
    // Keep the closure alive — wasm-bindgen drops the JS function otherwise.
    closure.forget();
}

#[wasm_bindgen_test]
fn test_engine_custom_op_with_object_return() {
    // `wrap` operator returns an object: `{value: <arg>}`
    let closure = Closure::wrap(Box::new(|args_json: JsValue| -> JsValue {
        let s = args_json.as_string().unwrap_or_default();
        // args_json is e.g. "[\"hi\"]"; pluck out the inner value
        let inner = &s[1..s.len() - 1];
        JsValue::from_str(&format!(r#"{{"value":{}}}"#, inner))
    }) as Box<dyn FnMut(JsValue) -> JsValue>);
    let func: Function = closure.as_ref().unchecked_ref::<Function>().clone();

    let opts = build_options(false, &[("wrap", func)]);
    let engine = Engine::new(opts).unwrap();
    let rule = engine.compile(r#"{"wrap": ["hi"]}"#).unwrap();
    assert_eq!(rule.evaluate("{}").unwrap(), r#"{"value":"hi"}"#);
    closure.forget();
}

#[wasm_bindgen_test]
fn test_engine_custom_op_error_propagates() {
    // Operator throws — should bubble up as an error
    let closure = Closure::wrap(Box::new(|_args_json: JsValue| -> JsValue {
        // Throw by returning a non-string and non-null — that's a binding
        // error path. (Real throws need `throw_val`; this is simpler.)
        JsValue::from_f64(42.0)
    }) as Box<dyn FnMut(JsValue) -> JsValue>);
    let func: Function = closure.as_ref().unchecked_ref::<Function>().clone();

    let opts = build_options(false, &[("bogus", func)]);
    let engine = Engine::new(opts).unwrap();
    let rule = engine.compile(r#"{"bogus": []}"#).unwrap();
    assert!(rule.evaluate("{}").is_err());
    closure.forget();
}
