#![cfg(target_arch = "wasm32")]

// No `wasm_bindgen_test_configure!(run_in_browser)` here on purpose: these
// tests use no DOM APIs, and leaving the default (node) configuration is
// what lets CI's `wasm-pack test --node` actually execute them. With the
// browser configuration set, the node runner skips the whole suite.

use datalogic_wasm::{CompiledRule, DataHandle, Engine, evaluate};
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

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
    let rule = CompiledRule::new(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#, false, None).unwrap();

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

// =============== Datetime clock tests ===============

// `now` must read the clock through `js_sys::Date` (chrono's `wasmbind`
// feature, target-gated in the core crate): wasm32-unknown-unknown has no
// `SystemTime`, so without it this call traps with "time not implemented
// on this platform" instead of returning a value or a catchable error.
#[wasm_bindgen_test]
fn test_now_operator_reads_clock() {
    let result = evaluate(r#"{"now": []}"#, "{}", false).unwrap();
    let iso = result.trim_matches('"');
    let ms = js_sys::Date::parse(iso);
    assert!(
        ms.is_finite(),
        "now() must return a parseable timestamp, got {result}"
    );
}

// =============== Structured error tests ===============

#[wasm_bindgen_test]
fn test_rejection_is_real_error_object() {
    let err = evaluate("not valid json", "{}", false).unwrap_err();
    let error: &js_sys::Error = err
        .dyn_ref()
        .expect("rejection must be a real js Error object");

    // `name` carries the stable error-kind tag.
    assert_eq!(String::from(error.name()), "ParseError");
    // `message` is the human-readable Display string.
    let message = String::from(error.message());
    assert!(!message.is_empty());
    assert!(message.contains("Parse error"), "message: {message}");

    // Structured fields ride along as own properties.
    let kind = Reflect::get(&err, &JsValue::from_str("type")).unwrap();
    assert_eq!(kind.as_string().as_deref(), Some("ParseError"));

    // The pre-Error-object JSON payload stays reachable for migration.
    let detail = Reflect::get(&err, &JsValue::from_str("detailJson")).unwrap();
    let detail = detail.as_string().expect("detailJson must be a string");
    assert!(
        detail.contains(r#""type":"ParseError""#),
        "detail: {detail}"
    );
}

#[wasm_bindgen_test]
fn test_runtime_error_carries_structured_fields() {
    let err = evaluate(r#"{"throw": "custom_error"}"#, "{}", false).unwrap_err();
    let error: &js_sys::Error = err.dyn_ref().expect("must be an Error object");
    assert_eq!(String::from(error.name()), "Thrown");

    // The thrown payload arrives as a real JS value, not a JSON string.
    let thrown = Reflect::get(&err, &JsValue::from_str("thrown")).unwrap();
    let thrown_type = Reflect::get(&thrown, &JsValue::from_str("type")).unwrap();
    assert_eq!(thrown_type.as_string().as_deref(), Some("custom_error"));

    // The failing operator is attached too.
    let operator = Reflect::get(&err, &JsValue::from_str("operator")).unwrap();
    assert_eq!(operator.as_string().as_deref(), Some("throw"));
}

#[wasm_bindgen_test]
fn test_input_error_has_stage_property() {
    let rule = CompiledRule::new(r#"{"var": "x"}"#, false, None).unwrap();
    let err = rule.evaluate("not valid json").unwrap_err();
    let error: &js_sys::Error = err.dyn_ref().expect("must be an Error object");
    assert_eq!(String::from(error.name()), "ParseError");

    // Boundary-side input failures say which input was bad.
    let stage = Reflect::get(&err, &JsValue::from_str("stage")).unwrap();
    assert_eq!(stage.as_string().as_deref(), Some("parse-data"));
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

// =============== Session tests ===============

#[wasm_bindgen_test]
fn test_session_reuse_and_allocated_bytes() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine
        .compile(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#)
        .unwrap();
    let mut session = engine.session();

    // Two evaluations on the same session, different data each time.
    assert_eq!(session.evaluate(&rule, r#"{"a": 1, "b": 2}"#).unwrap(), "3");
    assert_eq!(
        session.evaluate(&rule, r#"{"a": 10, "b": 20}"#).unwrap(),
        "30"
    );

    // The arena holds the last call's allocations; reset keeps the chunks
    // (no OS free) so the byte count stays sane, never grows.
    let after_evals = session.allocated_bytes();
    assert!(after_evals > 0);
    session.reset();
    let after_reset = session.allocated_bytes();
    assert!(after_reset <= after_evals);

    // The session stays usable after an explicit reset.
    assert_eq!(session.evaluate(&rule, r#"{"a": 2, "b": 2}"#).unwrap(), "4");
}

#[wasm_bindgen_test]
fn test_session_invalid_data_is_structured_error() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine.compile(r#"{"var": "x"}"#).unwrap();
    let mut session = engine.session();

    let err = session.evaluate(&rule, "not valid json").unwrap_err();
    let error: &js_sys::Error = err.dyn_ref().expect("must be an Error object");
    assert_eq!(String::from(error.name()), "ParseError");

    // A failed call must not poison the session.
    assert_eq!(session.evaluate(&rule, r#"{"x": 7}"#).unwrap(), "7");
}

// =============== DataHandle tests ===============

#[wasm_bindgen_test]
fn test_data_handle_parse_and_allocated_bytes() {
    let handle = DataHandle::new(r#"{"a": [1, 2, 3], "b": "text"}"#).unwrap();
    assert!(handle.allocated_bytes() > 0);
}

// The constructor's failure path must be a catchable, structured Error —
// never a trap.
#[wasm_bindgen_test]
fn test_data_handle_bad_json_is_parse_error() {
    let err = DataHandle::new("{ not json").unwrap_err();
    let error: &js_sys::Error = err.dyn_ref().expect("must be a real js Error object");
    assert_eq!(String::from(error.name()), "ParseError");
    let kind = Reflect::get(&err, &JsValue::from_str("type")).unwrap();
    assert_eq!(kind.as_string().as_deref(), Some("ParseError"));
}

#[wasm_bindgen_test]
fn test_compiled_rule_evaluate_data() {
    let rule = CompiledRule::new(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#, false, None).unwrap();
    let handle = DataHandle::new(r#"{"a": 1, "b": 2}"#).unwrap();

    // Handles are never consumed by evaluation: same handle, many calls.
    assert_eq!(rule.evaluate_data(&handle).unwrap(), "3");
    assert_eq!(rule.evaluate_data(&handle).unwrap(), "3");
}

// One handle is engine-independent: it can feed rules compiled by
// different engines (and the string path in between).
#[wasm_bindgen_test]
fn test_data_handle_reusable_across_engines_and_rules() {
    let handle = DataHandle::new(r#"{"x": 41}"#).unwrap();

    let engine_a = Engine::new(build_options(false, &[])).unwrap();
    let rule_a = engine_a.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
    assert_eq!(rule_a.evaluate_data(&handle).unwrap(), "42");

    let engine_b = Engine::new(build_options(false, &[])).unwrap();
    let rule_b = engine_b.compile(r#"{"-": [{"var": "x"}, 1]}"#).unwrap();
    assert_eq!(rule_b.evaluate_data(&handle).unwrap(), "40");
}

#[wasm_bindgen_test]
fn test_session_evaluate_data() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine
        .compile(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#)
        .unwrap();
    let mut session = engine.session();
    let handle = DataHandle::new(r#"{"a": 10, "b": 20}"#).unwrap();

    // Repeated handle evaluation on one session, interleaved with the
    // string path — the arena reset must not invalidate the handle.
    assert_eq!(session.evaluate_data(&rule, &handle).unwrap(), "30");
    assert_eq!(session.evaluate(&rule, r#"{"a": 1, "b": 1}"#).unwrap(), "2");
    assert_eq!(session.evaluate_data(&rule, &handle).unwrap(), "30");
}

// =============== Typed evaluation tests ===============

#[wasm_bindgen_test]
fn test_session_evaluate_bool() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine.compile(r#"{">=": [{"var": "age"}, 18]}"#).unwrap();
    let mut session = engine.session();

    let adult = DataHandle::new(r#"{"age": 21}"#).unwrap();
    let minor = DataHandle::new(r#"{"age": 16}"#).unwrap();
    assert!(session.evaluate_bool(&rule, &adult).unwrap());
    assert!(!session.evaluate_bool(&rule, &minor).unwrap());
}

// Strictness: a non-boolean result is a TypeMismatch (message wording
// shared with the C ABI binding), not a coercion.
#[wasm_bindgen_test]
fn test_session_evaluate_bool_type_mismatch() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine.compile(r#"{"+": [1, 2]}"#).unwrap();
    let mut session = engine.session();
    let handle = DataHandle::new("{}").unwrap();

    let err = session.evaluate_bool(&rule, &handle).unwrap_err();
    let error: &js_sys::Error = err.dyn_ref().expect("must be an Error object");
    assert_eq!(String::from(error.name()), "TypeMismatch");
    let message = String::from(error.message());
    assert_eq!(message, "result is not a boolean (got number)");
    let kind = Reflect::get(&err, &JsValue::from_str("type")).unwrap();
    assert_eq!(kind.as_string().as_deref(), Some("TypeMismatch"));

    // A mismatch must not poison the session.
    assert_eq!(session.evaluate_number(&rule, &handle).unwrap(), 3.0);
}

#[wasm_bindgen_test]
fn test_session_evaluate_number_and_mismatch() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let mut session = engine.session();
    let handle = DataHandle::new(r#"{"price": 12.5, "name": "x"}"#).unwrap();

    let number_rule = engine.compile(r#"{"var": "price"}"#).unwrap();
    assert_eq!(
        session.evaluate_number(&number_rule, &handle).unwrap(),
        12.5
    );

    let string_rule = engine.compile(r#"{"var": "name"}"#).unwrap();
    let err = session.evaluate_number(&string_rule, &handle).unwrap_err();
    let error: &js_sys::Error = err.dyn_ref().expect("must be an Error object");
    assert_eq!(String::from(error.name()), "TypeMismatch");
    assert_eq!(
        String::from(error.message()),
        "result is not a number (got string)"
    );
}

// evaluateTruthy never type-mismatches: any result collapses through the
// engine's truthiness rules (the `if`/`and`/`or` coercion).
#[wasm_bindgen_test]
fn test_session_evaluate_truthy() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine.compile(r#"{"var": "v"}"#).unwrap();
    let mut session = engine.session();

    for (data, expected) in [
        (r#"{"v": "non-empty"}"#, true),
        (r#"{"v": ""}"#, false),
        (r#"{"v": 0}"#, false),
        (r#"{"v": [1]}"#, true),
        (r#"{"v": null}"#, false),
    ] {
        let handle = DataHandle::new(data).unwrap();
        assert_eq!(
            session.evaluate_truthy(&rule, &handle).unwrap(),
            expected,
            "truthy({data})"
        );
    }
}

// =============== Batch evaluation tests ===============

/// Read `status` off a batch outcome object.
fn outcome_status(outcome: &JsValue) -> String {
    Reflect::get(outcome, &JsValue::from_str("status"))
        .unwrap()
        .as_string()
        .unwrap()
}

/// Read `value` (fulfilled entries) off a batch outcome object.
fn outcome_value(outcome: &JsValue) -> String {
    Reflect::get(outcome, &JsValue::from_str("value"))
        .unwrap()
        .as_string()
        .unwrap()
}

/// Read `reason.<field>` (rejected entries) off a batch outcome object.
fn outcome_reason_field(outcome: &JsValue, field: &str) -> JsValue {
    let reason = Reflect::get(outcome, &JsValue::from_str("reason")).unwrap();
    Reflect::get(&reason, &JsValue::from_str(field)).unwrap()
}

// One rule × many handles; a failing item never fails the call and
// carries the {tag, message, operator?} reason — trap-free throughout.
#[wasm_bindgen_test]
fn test_session_evaluate_batch_mixed_outcomes() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine
        .compile(r#"{"if": [{"var": "fail"}, {"throw": "boom"}, "ok"]}"#)
        .unwrap();
    let mut session = engine.session();

    let handles = Array::new();
    handles.push(&JsValue::from(
        DataHandle::new(r#"{"fail": false}"#).unwrap(),
    ));
    handles.push(&JsValue::from(
        DataHandle::new(r#"{"fail": true}"#).unwrap(),
    ));
    handles.push(&JsValue::from(
        DataHandle::new(r#"{"fail": false}"#).unwrap(),
    ));

    let results = session.evaluate_batch(&rule, handles.as_ref()).unwrap();
    assert_eq!(results.length(), 3);

    let first = results.get(0);
    assert_eq!(outcome_status(&first), "fulfilled");
    assert_eq!(outcome_value(&first), "\"ok\"");

    let second = results.get(1);
    assert_eq!(outcome_status(&second), "rejected");
    assert_eq!(
        outcome_reason_field(&second, "tag").as_string().as_deref(),
        Some("Thrown")
    );
    // `operator` is the *outermost* failing operator — here the `if`
    // wrapping the `throw`, same attribution as the scalar error path.
    assert_eq!(
        outcome_reason_field(&second, "operator")
            .as_string()
            .as_deref(),
        Some("if")
    );
    let message = outcome_reason_field(&second, "message")
        .as_string()
        .unwrap();
    assert!(!message.is_empty());

    let third = results.get(2);
    assert_eq!(outcome_status(&third), "fulfilled");
    assert_eq!(outcome_value(&third), "\"ok\"");
}

// A non-DataHandle element is a per-item InvalidArgument rejection; its
// neighbours still evaluate and the call itself succeeds.
#[wasm_bindgen_test]
fn test_session_evaluate_batch_invalid_element_is_item_error() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine.compile(r#"{"var": "x"}"#).unwrap();
    let mut session = engine.session();

    let handles = Array::new();
    handles.push(&JsValue::from(DataHandle::new(r#"{"x": 1}"#).unwrap()));
    handles.push(&JsValue::from_str("not a handle"));
    handles.push(&JsValue::from(DataHandle::new(r#"{"x": 3}"#).unwrap()));

    let results = session.evaluate_batch(&rule, handles.as_ref()).unwrap();
    assert_eq!(results.length(), 3);
    assert_eq!(outcome_value(&results.get(0)), "1");

    let bad = results.get(1);
    assert_eq!(outcome_status(&bad), "rejected");
    assert_eq!(
        outcome_reason_field(&bad, "tag").as_string().as_deref(),
        Some("InvalidArgument")
    );
    let message = outcome_reason_field(&bad, "message").as_string().unwrap();
    assert!(message.contains("handles[1]"), "message: {message}");

    assert_eq!(outcome_value(&results.get(2)), "3");
}

// A non-array `handles` argument is a call-level error (unlike per-item
// problems).
#[wasm_bindgen_test]
fn test_session_evaluate_batch_non_array_is_call_error() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let rule = engine.compile(r#"{"var": "x"}"#).unwrap();
    let mut session = engine.session();

    let err = session
        .evaluate_batch(&rule, &JsValue::from_str("nope"))
        .unwrap_err();
    let error: &js_sys::Error = err.dyn_ref().expect("must be an Error object");
    assert_eq!(String::from(error.name()), "ParseError");
}

// Many rules × one handle — the rule-set / feature-flag shape. The
// caller's Rule objects survive the call (the batch path must not
// consume them).
#[wasm_bindgen_test]
fn test_session_evaluate_many_mixed_outcomes() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let mut session = engine.session();
    let handle = DataHandle::new(r#"{"n": 21}"#).unwrap();

    let rules = Array::new();
    rules.push(&JsValue::from(
        engine.compile(r#"{"*": [{"var": "n"}, 2]}"#).unwrap(),
    ));
    rules.push(&JsValue::from(
        engine.compile(r#"{"throw": "nope"}"#).unwrap(),
    ));
    rules.push(&JsValue::from(
        engine.compile(r#"{"+": [{"var": "n"}, 1]}"#).unwrap(),
    ));

    for round in 0..2 {
        let results = session.evaluate_many(rules.as_ref(), &handle).unwrap();
        assert_eq!(results.length(), 3, "round {round}");
        assert_eq!(outcome_value(&results.get(0)), "42", "round {round}");
        let failed = results.get(1);
        assert_eq!(outcome_status(&failed), "rejected", "round {round}");
        assert_eq!(
            outcome_reason_field(&failed, "tag").as_string().as_deref(),
            Some("Thrown"),
            "round {round}"
        );
        assert_eq!(outcome_value(&results.get(2)), "22", "round {round}");
    }
}

// Element type confusion (a DataHandle in the rules array) is a per-item
// rejection, and empty inputs produce empty outputs.
#[wasm_bindgen_test]
fn test_session_evaluate_many_wrong_class_and_empty() {
    let engine = Engine::new(build_options(false, &[])).unwrap();
    let mut session = engine.session();
    let handle = DataHandle::new(r#"{"n": 1}"#).unwrap();

    let rules = Array::new();
    rules.push(&JsValue::from(DataHandle::new("{}").unwrap()));
    let results = session.evaluate_many(rules.as_ref(), &handle).unwrap();
    assert_eq!(results.length(), 1);
    let bad = results.get(0);
    assert_eq!(outcome_status(&bad), "rejected");
    assert_eq!(
        outcome_reason_field(&bad, "tag").as_string().as_deref(),
        Some("InvalidArgument")
    );
    let message = outcome_reason_field(&bad, "message").as_string().unwrap();
    assert!(message.contains("rules[0]"), "message: {message}");

    let empty = Array::new();
    let results = session.evaluate_many(empty.as_ref(), &handle).unwrap();
    assert_eq!(results.length(), 0);
}

// =============== Engine config tests ===============

/// Build an options bag `{ config: <config object> }`.
fn build_config_options(config: &Object) -> JsValue {
    let obj = Object::new();
    Reflect::set(&obj, &JsValue::from_str("config"), config.as_ref()).unwrap();
    obj.into()
}

#[wasm_bindgen_test]
fn test_engine_config_strict_preset_changes_behavior() {
    // Default semantics coerce null to 0: {"+": [null, 1]} evaluates to 1.
    let default_engine = Engine::new(build_options(false, &[])).unwrap();
    assert_eq!(
        default_engine
            .eval_str(r#"{"+": [null, 1]}"#, "{}")
            .unwrap(),
        "1"
    );

    // The strict preset rejects the null operand instead.
    let config = Object::new();
    Reflect::set(
        &config,
        &JsValue::from_str("preset"),
        &JsValue::from_str("strict"),
    )
    .unwrap();
    let strict_engine = Engine::new(build_config_options(&config)).unwrap();
    assert!(strict_engine.eval_str(r#"{"+": [null, 1]}"#, "{}").is_err());
}

#[wasm_bindgen_test]
fn test_compiled_rule_accepts_config_string() {
    // Same strict-vs-default split through CompiledRule's optional third
    // parameter, passed as a JSON string this time.
    let strict = CompiledRule::new(
        r#"{"+": [null, 1]}"#,
        false,
        Some(JsValue::from_str(r#"{"preset": "strict"}"#)),
    )
    .unwrap();
    assert!(strict.evaluate("{}").is_err());

    let default_rule = CompiledRule::new(r#"{"+": [null, 1]}"#, false, None).unwrap();
    assert_eq!(default_rule.evaluate("{}").unwrap(), "1");
}

#[wasm_bindgen_test]
fn test_invalid_config_rejects_with_configuration_error() {
    let err = match CompiledRule::new(
        r#"{"+": [1, 2]}"#,
        false,
        Some(JsValue::from_str(r#"{"preset": "bogus"}"#)),
    ) {
        Ok(_) => panic!("bogus preset must be rejected"),
        Err(e) => e,
    };
    let error: &js_sys::Error = err.dyn_ref().expect("must be an Error object");
    assert_eq!(String::from(error.name()), "ConfigurationError");
}
