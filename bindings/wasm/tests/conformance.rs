#![cfg(target_arch = "wasm32")]

//! The JSONLogic conformance suites, driven through the WASM API.
//!
//! The core crate's `test_jsonlogic.rs` runs the suites
//! (`crates/datalogic-rs/tests/suites/`, listed by `index.json`) against
//! the native engine. This runner walks the same files through the
//! exported `Engine` / `Rule` / `Session` / `DataHandle` classes, so every
//! value crosses the JS boundary as a host would see it. Every case runs
//! three ways, and each must agree with the expectation:
//!
//! 1. `engine.compile(rule).evaluate(data)`: JSON text in, JSON text out.
//! 2. `rule.evaluateData(handle)`: a pre-parsed `DataHandle`.
//! 3. `session.evaluateData(rule, handle)`: the same, on a session arena
//!    reused across every case.
//!
//! A `result` case must answer JSON equal to the expected value; an
//! `error` case must throw (here, as through the C ABI, the runner checks
//! that an error surfaced, not its exact shape, which the core runner
//! pins). Cases carry `templating` and `template_key_escape`, which this
//! runner honours with the matching `Engine` options.
//!
//! wasm32 has no file system, so the suites are read through Node's `fs`
//! (`wasm-pack test --node` is how this crate's tests run).

use std::collections::HashMap;

use datalogic_wasm::{DataHandle, Engine, Session};
use js_sys::{Object, Reflect};
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

const SUITES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/datalogic-rs/tests/suites"
);

#[wasm_bindgen(module = "fs")]
extern "C" {
    #[wasm_bindgen(js_name = readFileSync, catch)]
    fn read_file_sync(path: &str, encoding: &str) -> Result<String, JsValue>;
}

fn read(path: &str) -> String {
    read_file_sync(path, "utf8").unwrap_or_else(|e| panic!("cannot read {path}: {e:?}"))
}

/// The engine options a case asks for.
type Flavour = (bool, Option<String>);

/// One engine (and one session on it) per flavour, built on first use.
struct Engines {
    by_flavour: HashMap<Flavour, (Engine, Session)>,
}

impl Engines {
    fn get(&mut self, flavour: &Flavour) -> &mut (Engine, Session) {
        self.by_flavour.entry(flavour.clone()).or_insert_with(|| {
            let opts = Object::new();
            Reflect::set(&opts, &"templating".into(), &JsValue::from_bool(flavour.0)).unwrap();
            if let Some(escape) = &flavour.1 {
                Reflect::set(
                    &opts,
                    &"templateKeyEscape".into(),
                    &JsValue::from_str(escape),
                )
                .unwrap();
            }
            let engine = Engine::new(opts.into()).expect("engine options are valid");
            let session = engine.session();
            (engine, session)
        })
    }
}

/// The three ways one case runs, each as JSON text or the thrown error.
fn run_case(
    engines: &mut Engines,
    flavour: &Flavour,
    rule: &str,
    data: &str,
) -> [(&'static str, Result<String, String>); 3] {
    let describe = |e: JsValue| -> String {
        Reflect::get(&e, &"message".into())
            .ok()
            .and_then(|m| m.as_string())
            .unwrap_or_else(|| format!("{e:?}"))
    };
    let (engine, session) = engines.get(flavour);
    let compiled = match engine.compile(rule) {
        Ok(compiled) => compiled,
        Err(e) => {
            let msg = describe(e);
            return [
                ("evaluate", Err(msg.clone())),
                ("rule.evaluateData", Err(msg.clone())),
                ("session.evaluateData", Err(msg)),
            ];
        }
    };
    let text = compiled.evaluate(data).map_err(describe);
    let (handled, sessioned) = match DataHandle::new(data) {
        Ok(handle) => (
            compiled.evaluate_data(&handle).map_err(describe),
            session.evaluate_data(&compiled, &handle).map_err(describe),
        ),
        Err(e) => {
            let msg = describe(e);
            (Err(msg.clone()), Err(msg))
        }
    };
    [
        ("evaluate", text),
        ("rule.evaluateData", handled),
        ("session.evaluateData", sessioned),
    ]
}

#[wasm_bindgen_test]
fn conformance_suites_pass_through_wasm() {
    let index: Vec<String> = serde_json::from_str(&read(&format!("{SUITES}/index.json")))
        .expect("index.json is a JSON array of file names");
    let mut engines = Engines {
        by_flavour: HashMap::new(),
    };
    let (mut passed, mut failures) = (0usize, Vec::new());

    for file in &index {
        let cases: Value = serde_json::from_str(&read(&format!("{SUITES}/{file}")))
            .unwrap_or_else(|e| panic!("{file} is not JSON: {e}"));
        for (i, case) in cases
            .as_array()
            .expect("a suite is an array")
            .iter()
            .enumerate()
        {
            // Strings are section headers.
            let Some(obj) = case.as_object() else {
                continue;
            };
            let description = obj
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("(no description)");
            let flavour: Flavour = (
                obj.get("templating")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                obj.get("template_key_escape")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            );
            let rule = obj.get("rule").expect("a case has a rule").to_string();
            let data = obj
                .get("data")
                .cloned()
                .unwrap_or(Value::Object(Default::default()))
                .to_string();
            let expected = obj.get("result");
            let expects_error = obj.contains_key("error");
            assert!(
                expects_error || expected.is_some(),
                "{file}[{i}] has neither `result` nor `error`"
            );

            for (path, got) in run_case(&mut engines, &flavour, &rule, &data) {
                let problem = match (got, expected) {
                    (Err(_), _) if expects_error => None,
                    (Ok(text), _) if expects_error => {
                        Some(format!("expected an error, got {text}"))
                    }
                    (Err(msg), Some(want)) => Some(format!("expected {want}, got error: {msg}")),
                    (Ok(text), Some(want)) => match serde_json::from_str::<Value>(&text) {
                        Ok(got) if &got == want => None,
                        Ok(got) => Some(format!("expected {want}, got {got}")),
                        Err(e) => Some(format!("result is not JSON ({e}): {text}")),
                    },
                    (_, None) => unreachable!("checked above"),
                };
                match problem {
                    None => passed += 1,
                    Some(p) => failures.push(format!("  {file}[{i}] {description} [{path}]: {p}")),
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} conformance case(s) failed through WASM:\n{}",
        failures.len(),
        failures.join("\n")
    );
    // Three paths per case; the suites hold well over a thousand cases.
    assert!(passed > 3000, "only {passed} case runs passed");
}
