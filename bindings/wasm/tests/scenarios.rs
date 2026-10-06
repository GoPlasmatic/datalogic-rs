#![cfg(target_arch = "wasm32")]

// The cross-binding scenarios in bindings/scenarios/api.json, through the
// WASM API. Every binding runs the same file (see bindings/BINDINGS.md).

use datalogic_wasm::Engine;
use js_sys::{Object, Reflect};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

const SCENARIOS: &str = include_str!("../../scenarios/api.json");

fn engine_for(case: &Value) -> Result<Engine, JsValue> {
    let opts = Object::new();
    let e = &case["engine"];
    if let Some(t) = e["templating"].as_bool() {
        Reflect::set(&opts, &"templating".into(), &JsValue::from_bool(t)).unwrap();
    }
    if let Some(c) = e["template_key_escape"].as_str() {
        Reflect::set(&opts, &"templateKeyEscape".into(), &JsValue::from_str(c)).unwrap();
    }
    if !e["config"].is_null() {
        Reflect::set(
            &opts,
            &"config".into(),
            &JsValue::from_str(&e["config"].to_string()),
        )
        .unwrap();
    }
    if let Some(names) = e["families"].as_array() {
        let families = js_sys::Array::new();
        for name in names {
            families.push(&JsValue::from_str(name.as_str().unwrap()));
        }
        Reflect::set(&opts, &"families".into(), &families).unwrap();
    }
    Engine::new(opts.into())
}

fn error_type(err: JsValue) -> Value {
    let name = Reflect::get(&err, &"name".into()).unwrap();
    json!(["error", name.as_string()])
}

fn run(case: &Value) -> Value {
    let engine = match engine_for(case) {
        Ok(engine) => engine,
        Err(err) => return error_type(err),
    };
    let rule = case["rule"].to_string();
    let data = case["data"].to_string();
    let parse = |s: String| serde_json::from_str::<Value>(&s).unwrap();
    let outcome: Result<Value, JsValue> = (|| match case["call"].as_str().unwrap() {
        "check" => {
            let diags = parse(engine.check(&rule, case["mode"].as_str().map(str::to_string))?);
            Ok(Value::Array(
                diags
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| json!([d["code"], d["pointer"]]))
                    .collect(),
            ))
        }
        "truthy" => Ok(json!(engine.truthy(&case["value"].to_string())?)),
        "facts" => Ok(parse(engine.compile(&rule)?.facts())),
        "trace" => {
            let run = parse(engine.evaluate_with_trace(
                &rule,
                &data,
                case["mode"].as_str().map(str::to_string),
            )?);
            let pointers: std::collections::BTreeSet<&str> = run["pointers"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(_, p)| p.as_str())
                .collect();
            Ok(json!({"result": run["result"], "pointers": pointers}))
        }
        "metered" => {
            let budget = case["budget"].as_f64();
            let out = parse(engine.compile(&rule)?.evaluate_metered(&data, budget)?);
            Ok(out["result"].clone())
        }
        call => {
            let compiled = match call {
                "evaluate" => engine.compile(&rule)?,
                "compile_template" => engine.compile_template(&rule)?,
                "compile_strict" => engine.compile_strict(&rule)?,
                "compile_checked" => engine.compile_checked(&rule)?,
                other => panic!("unknown call {other}"),
            };
            Ok(parse(compiled.evaluate(&data)?))
        }
    })();
    outcome.unwrap_or_else(error_type)
}

#[wasm_bindgen_test]
fn scenarios_pass() {
    let cases: Vec<Value> = serde_json::from_str(SCENARIOS).unwrap();
    let mut ran = 0;
    for case in cases.iter().filter(|c| c.is_object()) {
        let got = run(case);
        let what = format!("{}: {}", case["call"], case["description"]);
        if let Some(err) = case.get("error") {
            assert_eq!(got, json!(["error", err]), "{what}");
        } else if let Some(diags) = case.get("diagnostics") {
            assert_eq!(&got, diags, "{what}");
        } else if let Some(trace) = case.get("trace") {
            assert_eq!(&got, trace, "{what}");
        } else if let Some(facts) = case.get("facts") {
            for (k, v) in facts.as_object().unwrap() {
                assert_eq!(&got[k], v, "{what}: {k}");
            }
        } else {
            assert_eq!(&got, &case["result"], "{what}");
        }
        ran += 1;
    }
    assert!(ran >= 25);
}
