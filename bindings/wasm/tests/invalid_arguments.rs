#![cfg(target_arch = "wasm32")]

// Pins how this binding refuses a bad argument today. The bindings do not
// agree (a budget of 0 is a `ParseError` here, `InvalidArguments` in Node,
// `InvalidArgument` in Python and the engine's own budget through the C
// ABI), so the cross-binding scenarios cannot hold these cases; each
// binding pins its own until one spelling is chosen.

use datalogic_wasm::Engine;
use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

fn name_and_stage(err: JsValue) -> (String, Option<String>) {
    let get = |k: &str| {
        Reflect::get(&err, &k.into())
            .ok()
            .and_then(|v| v.as_string())
    };
    (get("name").unwrap_or_default(), get("stage"))
}

fn engine_with(key: &str, value: JsValue) -> Result<Engine, JsValue> {
    let opts = Object::new();
    Reflect::set(&opts, &"templating".into(), &JsValue::TRUE).unwrap();
    Reflect::set(&opts, &key.into(), &value).unwrap();
    Engine::new(opts.into())
}

#[wasm_bindgen_test]
fn a_bad_budget_is_a_parse_error() {
    let engine = Engine::new(JsValue::UNDEFINED).unwrap();
    let rule = engine.compile(r#"{"+": [1, 2]}"#).unwrap();
    for budget in [0.0, -1.0, 1.5, f64::NAN, f64::INFINITY] {
        let err = rule.evaluate_metered("null", Some(budget)).unwrap_err();
        assert_eq!(
            name_and_stage(err),
            ("ParseError".to_string(), Some("parse-budget".to_string())),
            "{budget}"
        );
        let err = engine
            .eval_metered(r#"{"+": [1, 2]}"#, "null", Some(budget))
            .unwrap_err();
        assert_eq!(name_and_stage(err).0, "ParseError");
    }
    // An omitted budget falls back to the engine's.
    let out: serde_json::Value =
        serde_json::from_str(&rule.evaluate_metered("null", None).unwrap()).unwrap();
    assert_eq!(out["result"], 3);
}

#[wasm_bindgen_test]
fn a_long_template_key_escape_is_a_parse_error() {
    for escape in ["", "ab"] {
        let err = engine_with("templateKeyEscape", JsValue::from_str(escape))
            .err()
            .unwrap();
        assert_eq!(
            name_and_stage(err),
            ("ParseError".to_string(), Some("parse-options".to_string()))
        );
    }
}

#[wasm_bindgen_test]
fn an_unknown_mode_is_a_parse_error() {
    let engine = Engine::new(JsValue::UNDEFINED).unwrap();
    let err = engine
        .check(r#"{"var": "a"}"#, Some("loose".to_string()))
        .unwrap_err();
    assert_eq!(
        name_and_stage(err),
        ("ParseError".to_string(), Some("parse-mode".to_string()))
    );
    let err = engine
        .evaluate_with_trace(r#"{"var": "a"}"#, "{}", Some("loose".to_string()))
        .unwrap_err();
    assert_eq!(name_and_stage(err).0, "ParseError");
}

#[wasm_bindgen_test]
fn an_unknown_config_key_or_family_is_a_configuration_error() {
    let err = engine_with("config", JsValue::from_str(r#"{"bogus": 1}"#))
        .err()
        .unwrap();
    assert_eq!(name_and_stage(err).0, "ConfigurationError");
    let families = Array::of1(&JsValue::from_str("Strings"));
    let err = engine_with("families", families.into()).err().unwrap();
    assert_eq!(name_and_stage(err).0, "ConfigurationError");
}

#[wasm_bindgen_test]
fn a_traced_result_keeps_its_key_order() {
    let engine = Engine::new(JsValue::UNDEFINED).unwrap();
    let text = engine
        .evaluate_with_trace(
            r#"{"var": "o"}"#,
            r#"{"o": {"z": 1, "a": {"y": 2, "b": 3}}}"#,
            None,
        )
        .unwrap();
    assert!(
        text.starts_with(r#"{"result":{"z":1,"a":{"y":2,"b":3}},"#),
        "{text}"
    );
}
