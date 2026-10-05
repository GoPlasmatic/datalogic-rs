//! The cross-binding scenarios in bindings/scenarios/api.json, through the
//! C ABI. Every binding runs the same file (see bindings/BINDINGS.md); the
//! Go, JVM, .NET and PHP wrappers run it over this same ABI.

use datalogic_c::*;
use serde_json::{Value, json};

const SCENARIOS: &str = include_str!("../../scenarios/api.json");

struct Owned {
    engine: *mut Engine,
}

impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { datalogic_engine_free(self.engine) };
    }
}

fn engine_for(case: &Value) -> Owned {
    let opts = &case["engine"];
    let b = datalogic_engine_builder_new();
    unsafe {
        if opts["templating"].as_bool() == Some(true) {
            datalogic_engine_builder_set_templating(b, 1);
        }
        if let Some(c) = opts["template_key_escape"].as_str() {
            let cp = c.chars().next().unwrap() as u32;
            assert_eq!(
                datalogic_engine_builder_set_template_key_escape(b, cp, std::ptr::null_mut()),
                Status::Ok
            );
        }
        if !opts["config"].is_null() {
            let cfg = opts["config"].to_string();
            assert_eq!(
                datalogic_engine_builder_set_config_json(
                    b,
                    cfg.as_ptr(),
                    cfg.len(),
                    std::ptr::null_mut()
                ),
                Status::Ok
            );
        }
        let engine = datalogic_engine_builder_build(b);
        datalogic_engine_builder_free(b);
        Owned { engine }
    }
}

fn s(ptr: *const u8, len: usize) -> String {
    if ptr.is_null() {
        return String::new();
    }
    String::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()).unwrap()
}

fn take(buf: Buf) -> Value {
    let text = s(buf.ptr, buf.len);
    unsafe { datalogic_buf_free(buf) };
    serde_json::from_str(&text).unwrap()
}

fn empty() -> Buf {
    Buf {
        ptr: std::ptr::null_mut(),
        len: 0,
        cap: 0,
    }
}

/// `Err(error type)` from a failed call's error handle.
fn tag(err: *mut Error) -> String {
    let mut len = 0;
    let t = s(unsafe { datalogic_error_tag(err, &mut len) }, len);
    unsafe { datalogic_error_free(err) };
    t
}

fn mode(case: &Value) -> u32 {
    match case["mode"].as_str() {
        None | Some("engine") => 0,
        Some("strict") => 1,
        Some("template") => 2,
        Some(other) => panic!("mode {other}"),
    }
}

fn compile(engine: *const Engine, call: &str, rule: &str) -> Result<*mut Rule, String> {
    let mut out: *mut Rule = std::ptr::null_mut();
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        match call {
            "compile_template" => datalogic_engine_compile_mode(
                engine,
                rule.as_ptr(),
                rule.len(),
                2,
                &mut out,
                &mut err,
            ),
            "compile_strict" => datalogic_engine_compile_mode(
                engine,
                rule.as_ptr(),
                rule.len(),
                1,
                &mut out,
                &mut err,
            ),
            "compile_checked" => datalogic_engine_compile_checked(
                engine,
                rule.as_ptr(),
                rule.len(),
                &mut out,
                &mut err,
            ),
            _ => datalogic_engine_compile(engine, rule.as_ptr(), rule.len(), &mut out, &mut err),
        }
    };
    if status == Status::Ok {
        Ok(out)
    } else {
        Err(tag(err))
    }
}

fn run(case: &Value) -> Result<Value, String> {
    let owned = engine_for(case);
    let engine = owned.engine;
    let rule = case["rule"].to_string();
    let data = case["data"].to_string();
    let call = case["call"].as_str().unwrap();
    let mut err: *mut Error = std::ptr::null_mut();
    match call {
        "check" => {
            let mut out = empty();
            let st = unsafe {
                datalogic_engine_check(
                    engine,
                    rule.as_ptr(),
                    rule.len(),
                    mode(case),
                    &mut out,
                    &mut err,
                )
            };
            if st != Status::Ok {
                return Err(tag(err));
            }
            let diags = take(out);
            Ok(Value::Array(
                diags
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| json!([d["code"], d["pointer"]]))
                    .collect(),
            ))
        }
        "truthy" => {
            let v = case["value"].to_string();
            let mut out = 0;
            let st =
                unsafe { datalogic_engine_truthy(engine, v.as_ptr(), v.len(), &mut out, &mut err) };
            if st != Status::Ok {
                return Err(tag(err));
            }
            Ok(json!(out != 0))
        }
        "facts" => {
            let r = compile(engine, "evaluate", &rule)?;
            let mut out = empty();
            let st = unsafe { datalogic_rule_facts(r, &mut out, &mut err) };
            unsafe { datalogic_rule_free(r) };
            if st != Status::Ok {
                return Err(tag(err));
            }
            Ok(take(out))
        }
        "metered" => {
            let r = compile(engine, "evaluate", &rule)?;
            let session = unsafe { datalogic_engine_session(engine) };
            let (mut ptr, mut len, mut ops) = (std::ptr::null(), 0usize, 0u64);
            let budget = case["budget"].as_u64().unwrap();
            let st = unsafe {
                datalogic_session_evaluate_metered(
                    session,
                    r,
                    data.as_ptr(),
                    data.len(),
                    budget,
                    &mut ptr,
                    &mut len,
                    &mut ops,
                    &mut err,
                )
            };
            let out = if st == Status::Ok {
                Ok(serde_json::from_str(&s(ptr, len)).unwrap())
            } else {
                Err(tag(err))
            };
            unsafe {
                datalogic_session_free(session);
                datalogic_rule_free(r);
            }
            out
        }
        call => {
            let r = compile(engine, call, &rule)?;
            let mut out = empty();
            let st = unsafe {
                datalogic_rule_evaluate(r, data.as_ptr(), data.len(), &mut out, &mut err)
            };
            unsafe { datalogic_rule_free(r) };
            if st != Status::Ok {
                return Err(tag(err));
            }
            Ok(take(out))
        }
    }
}

#[test]
fn scenarios_pass() {
    let cases: Vec<Value> = serde_json::from_str(SCENARIOS).unwrap();
    let mut ran = 0;
    for case in cases.iter().filter(|c| c.is_object()) {
        let what = format!("{}: {}", case["call"], case["description"]);
        let got = run(case);
        if let Some(err) = case.get("error") {
            assert_eq!(got.err().as_deref(), err.as_str(), "{what}");
        } else {
            let got = got.unwrap_or_else(|e| panic!("{what}: {e}"));
            if let Some(diags) = case.get("diagnostics") {
                assert_eq!(&got, diags, "{what}");
            } else if let Some(facts) = case.get("facts") {
                for (k, v) in facts.as_object().unwrap() {
                    assert_eq!(&got[k], v, "{what}: {k}");
                }
            } else {
                assert_eq!(&got, &case["result"], "{what}");
            }
        }
        ran += 1;
    }
    assert!(ran >= 25);
}
