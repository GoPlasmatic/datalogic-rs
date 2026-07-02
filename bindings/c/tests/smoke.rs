//! Smoke tests for the C ABI.
//!
//! These tests link `datalogic_c` as an `rlib` and call the `extern "C"`
//! functions directly. They are not exhaustive — they exist to catch
//! linkage / null-handling / last-error regressions across releases.
//! The full operator surface is exercised by the core crate's JSONLogic
//! suite, not here.
//!
//! All entry points except [`datalogic_string_free`] /
//! [`datalogic_engine_free`] / [`datalogic_rule_free`] /
//! [`datalogic_session_free`] use the binding's owned-string return
//! contract — every `*mut c_char` we receive must be released through
//! [`datalogic_string_free`], or we leak. Tests follow the contract
//! strictly so leak detectors (miri, ASan) stay clean.

use std::ffi::{CStr, CString};

use datalogic_c::*;

fn cstr(s: &str) -> CString {
    CString::new(s).expect("test input has no NULs")
}

unsafe fn take_string(ptr: *mut std::ffi::c_char) -> String {
    assert!(!ptr.is_null(), "expected non-null result string");
    let s = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .expect("result must be UTF-8")
        .to_owned();
    unsafe { datalogic_string_free(ptr) };
    s
}

#[test]
fn version_is_static_and_matches_cargo_pkg_version() {
    let p = datalogic_version();
    assert!(!p.is_null());
    let v = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
    assert_eq!(v, env!("CARGO_PKG_VERSION"));
    // Calling twice returns the same static pointer.
    assert_eq!(datalogic_version(), p);
}

#[test]
fn apply_one_shot_returns_json_result() {
    let engine = datalogic_engine_new(0);
    let rule = cstr(r#"{"+":[1,2]}"#);
    let data = cstr("{}");
    let out = unsafe { datalogic_engine_apply(engine, rule.as_ptr(), data.as_ptr()) };
    let s = unsafe { take_string(out) };
    assert_eq!(s, "3");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn compile_then_evaluate_reuses_rule() {
    let engine = datalogic_engine_new(0);
    let rule_src = cstr(r#"{"var":"x"}"#);
    let rule = unsafe { datalogic_engine_compile(engine, rule_src.as_ptr()) };
    assert!(!rule.is_null());

    for x in [1, 7, 42] {
        let data = cstr(&format!(r#"{{"x":{x}}}"#));
        let out = unsafe { datalogic_rule_evaluate(rule, data.as_ptr()) };
        let s = unsafe { take_string(out) };
        assert_eq!(s, x.to_string());
    }

    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn session_evaluate_reuses_arena() {
    let engine = datalogic_engine_new(0);
    let rule_src = cstr(r#"{"*":[{"var":"x"},2]}"#);
    let rule = unsafe { datalogic_engine_compile(engine, rule_src.as_ptr()) };
    let session = unsafe { datalogic_engine_session(engine) };

    for x in [3, 5, 8] {
        let data = cstr(&format!(r#"{{"x":{x}}}"#));
        let out = unsafe { datalogic_session_evaluate(session, rule, data.as_ptr()) };
        let s = unsafe { take_string(out) };
        assert_eq!(s, (x * 2).to_string());
    }

    // After several evaluations the arena should have *some* allocations
    // (even after the reset-at-start-of-call, the most recent call's
    // working memory is still in the arena).
    let bytes = unsafe { datalogic_session_allocated_bytes(session) };
    assert!(bytes > 0, "arena should hold the most recent call's memory");

    unsafe { datalogic_session_reset(session) };
    unsafe { datalogic_session_free(session) };
    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn parse_error_returns_null_and_sets_last_error() {
    datalogic_last_error_clear();

    let engine = datalogic_engine_new(0);
    let bad_rule = cstr("not-valid-json{{");
    let result = unsafe { datalogic_engine_compile(engine, bad_rule.as_ptr()) };
    assert!(result.is_null());

    let msg_ptr = datalogic_last_error_message();
    assert!(!msg_ptr.is_null(), "last error message should be populated");
    let msg = unsafe { CStr::from_ptr(msg_ptr) }.to_str().unwrap();
    assert!(!msg.is_empty());

    let tag = unsafe { CStr::from_ptr(datalogic_last_error_type()) }
        .to_str()
        .unwrap();
    assert_eq!(tag, "ParseError");

    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn evaluate_error_sets_operator_and_path() {
    datalogic_last_error_clear();

    let engine = datalogic_engine_new(0);
    // `throw` is a control-flow operator that raises a Thrown error.
    let rule_src = cstr(r#"{"throw":"boom"}"#);
    let rule = unsafe { datalogic_engine_compile(engine, rule_src.as_ptr()) };
    assert!(!rule.is_null());
    let data = cstr("{}");
    let out = unsafe { datalogic_rule_evaluate(rule, data.as_ptr()) };
    assert!(out.is_null(), "throw should fail evaluation");

    let tag = unsafe { CStr::from_ptr(datalogic_last_error_type()) }
        .to_str()
        .unwrap();
    // Stable tag for runtime `throw`. If the engine renames the tag this
    // test surfaces it.
    assert_eq!(tag, "Thrown");

    // Path is available because the binding had `&Logic` at the point
    // of failure.
    let path_ptr = datalogic_last_error_path_json();
    assert!(!path_ptr.is_null(), "path JSON should be populated");
    let path = unsafe { CStr::from_ptr(path_ptr) }.to_str().unwrap();
    assert!(
        path.starts_with('['),
        "path should be a JSON array, got: {path}"
    );

    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

// =============== Custom operator builder ===============

/// Test callback: `[n]` -> `n*2` as a JSON string.
unsafe extern "C" fn double_op(
    args_json: *const std::ffi::c_char,
    _user_data: *mut std::ffi::c_void,
    _error_out: *mut *mut std::ffi::c_char,
) -> *mut std::ffi::c_char {
    let args = unsafe { CStr::from_ptr(args_json) }.to_str().unwrap();
    // args is `"[n]"` — strip brackets and parse.
    let inner: f64 = args[1..args.len() - 1].trim().parse().unwrap();
    let result = format!("{}", inner * 2.0);
    // Allocate with libc malloc so the binding's libc free works.
    let cs = CString::new(result).unwrap();
    // Use `Box::into_raw` of a `String`'s bytes won't survive libc free;
    // we instead `strdup` via libc.
    unsafe extern "C" {
        fn strdup(s: *const std::ffi::c_char) -> *mut std::ffi::c_char;
    }
    unsafe { strdup(cs.as_ptr()) }
}

/// Test callback that always errors.
unsafe extern "C" fn boom_op(
    _args_json: *const std::ffi::c_char,
    _user_data: *mut std::ffi::c_void,
    error_out: *mut *mut std::ffi::c_char,
) -> *mut std::ffi::c_char {
    unsafe extern "C" {
        fn strdup(s: *const std::ffi::c_char) -> *mut std::ffi::c_char;
    }
    let msg = cstr("custom-failure");
    unsafe { *error_out = strdup(msg.as_ptr()) };
    std::ptr::null_mut()
}

/// Test callback that reads `user_data` (an i64 pointer) and adds it.
unsafe extern "C" fn add_user_data_op(
    args_json: *const std::ffi::c_char,
    user_data: *mut std::ffi::c_void,
    _error_out: *mut *mut std::ffi::c_char,
) -> *mut std::ffi::c_char {
    let bias = unsafe { *(user_data as *const i64) };
    let args = unsafe { CStr::from_ptr(args_json) }.to_str().unwrap();
    let inner: i64 = args[1..args.len() - 1].trim().parse().unwrap();
    let result = CString::new(format!("{}", inner + bias)).unwrap();
    unsafe extern "C" {
        fn strdup(s: *const std::ffi::c_char) -> *mut std::ffi::c_char;
    }
    unsafe { strdup(result.as_ptr()) }
}

#[test]
fn builder_with_custom_operator_evaluates() {
    let b = datalogic_engine_builder_new();
    let name = cstr("double");
    unsafe {
        let rc = datalogic_engine_builder_add_operator(
            b,
            name.as_ptr(),
            Some(double_op),
            std::ptr::null_mut(),
        );
        assert_eq!(rc, 0);
    }
    let engine = unsafe { datalogic_engine_builder_build(b) };
    assert!(!engine.is_null());
    unsafe { datalogic_engine_builder_free(b) };

    let rule = cstr(r#"{"double":[21]}"#);
    let data = cstr("{}");
    let out = unsafe { datalogic_engine_apply(engine, rule.as_ptr(), data.as_ptr()) };
    let s = unsafe { take_string(out) };
    assert_eq!(s, "42");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_custom_operator_error_propagates() {
    datalogic_last_error_clear();
    let b = datalogic_engine_builder_new();
    let name = cstr("boom");
    unsafe {
        datalogic_engine_builder_add_operator(
            b,
            name.as_ptr(),
            Some(boom_op),
            std::ptr::null_mut(),
        );
    }
    let engine = unsafe { datalogic_engine_builder_build(b) };
    unsafe { datalogic_engine_builder_free(b) };

    let rule = cstr(r#"{"boom":[]}"#);
    let data = cstr("{}");
    let out = unsafe { datalogic_engine_apply(engine, rule.as_ptr(), data.as_ptr()) };
    assert!(out.is_null(), "boom should fail evaluation");
    let msg = unsafe { CStr::from_ptr(datalogic_last_error_message()) }
        .to_str()
        .unwrap();
    assert!(msg.contains("custom-failure"), "got message: {msg}");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_custom_operator_receives_user_data() {
    let b = datalogic_engine_builder_new();
    let name = cstr("addbias");
    let bias: i64 = 100;
    unsafe {
        datalogic_engine_builder_add_operator(
            b,
            name.as_ptr(),
            Some(add_user_data_op),
            &bias as *const i64 as *mut std::ffi::c_void,
        );
    }
    let engine = unsafe { datalogic_engine_builder_build(b) };
    unsafe { datalogic_engine_builder_free(b) };

    let rule = cstr(r#"{"addbias":[7]}"#);
    let data = cstr("{}");
    let out = unsafe { datalogic_engine_apply(engine, rule.as_ptr(), data.as_ptr()) };
    let s = unsafe { take_string(out) };
    assert_eq!(s, "107");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_set_templating_takes_effect() {
    let b = datalogic_engine_builder_new();
    unsafe { datalogic_engine_builder_set_templating(b, 1) };
    let engine = unsafe { datalogic_engine_builder_build(b) };
    unsafe { datalogic_engine_builder_free(b) };
    assert!(!engine.is_null());
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_set_config_json_strict_preset_takes_effect() {
    // Default config: `{"+": [null, 1]}` coerces null to 0 and returns 1.
    let engine = datalogic_engine_new(0);
    let rule = cstr(r#"{"+":[null,1]}"#);
    let data = cstr("{}");
    let out = unsafe { datalogic_engine_apply(engine, rule.as_ptr(), data.as_ptr()) };
    let s = unsafe { take_string(out) };
    assert_eq!(s, "1");
    unsafe { datalogic_engine_free(engine) };

    // Strict preset: the same rule rejects the non-numeric null.
    datalogic_last_error_clear();
    let b = datalogic_engine_builder_new();
    let config = cstr(r#"{"preset":"strict"}"#);
    let rc = unsafe { datalogic_engine_builder_set_config_json(b, config.as_ptr()) };
    assert_eq!(rc, 0);
    let engine = unsafe { datalogic_engine_builder_build(b) };
    assert!(!engine.is_null());
    unsafe { datalogic_engine_builder_free(b) };

    let out = unsafe { datalogic_engine_apply(engine, rule.as_ptr(), data.as_ptr()) };
    assert!(out.is_null(), "strict config should reject null operand");
    let msg = unsafe { CStr::from_ptr(datalogic_last_error_message()) }
        .to_str()
        .unwrap();
    assert!(!msg.is_empty());
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_set_config_json_rejects_bad_input() {
    // Malformed JSON -> -1 + last-error message.
    datalogic_last_error_clear();
    let b = datalogic_engine_builder_new();
    let bad = cstr("not-json{{");
    let rc = unsafe { datalogic_engine_builder_set_config_json(b, bad.as_ptr()) };
    assert_eq!(rc, -1);
    let msg = unsafe { CStr::from_ptr(datalogic_last_error_message()) }
        .to_str()
        .unwrap();
    assert!(!msg.is_empty());

    // Unknown enum value -> -1 (the shared parser fails loudly on typos).
    let bogus = cstr(r#"{"preset":"bogus"}"#);
    let rc = unsafe { datalogic_engine_builder_set_config_json(b, bogus.as_ptr()) };
    assert_eq!(rc, -1);
    let msg = unsafe { CStr::from_ptr(datalogic_last_error_message()) }
        .to_str()
        .unwrap();
    assert!(msg.contains("bogus"), "got message: {msg}");

    // NULL config pointer -> -1.
    let rc = unsafe { datalogic_engine_builder_set_config_json(b, std::ptr::null()) };
    assert_eq!(rc, -1);

    // NULL builder pointer -> -1.
    let good = cstr(r#"{"preset":"strict"}"#);
    let rc =
        unsafe { datalogic_engine_builder_set_config_json(std::ptr::null_mut(), good.as_ptr()) };
    assert_eq!(rc, -1);

    // A failed set_config_json leaves the builder usable: it still builds
    // (with whatever config it had) rather than being poisoned.
    let engine = unsafe { datalogic_engine_builder_build(b) };
    assert!(!engine.is_null());
    unsafe { datalogic_engine_free(engine) };
    unsafe { datalogic_engine_builder_free(b) };
}

#[test]
fn builder_build_twice_returns_null_and_sets_error() {
    let b = datalogic_engine_builder_new();
    let engine = unsafe { datalogic_engine_builder_build(b) };
    assert!(!engine.is_null());
    let again = unsafe { datalogic_engine_builder_build(b) };
    assert!(again.is_null(), "second build must fail");
    unsafe { datalogic_engine_free(engine) };
    unsafe { datalogic_engine_builder_free(b) };
}

#[test]
fn null_pointers_are_handled_without_segfault() {
    // Free entry points are explicitly NULL-safe.
    unsafe { datalogic_engine_free(std::ptr::null_mut()) };
    unsafe { datalogic_rule_free(std::ptr::null_mut()) };
    unsafe { datalogic_session_free(std::ptr::null_mut()) };
    unsafe { datalogic_traced_session_free(std::ptr::null_mut()) };
    unsafe { datalogic_string_free(std::ptr::null_mut()) };
    unsafe { datalogic_session_reset(std::ptr::null_mut()) };
    assert_eq!(
        unsafe { datalogic_session_allocated_bytes(std::ptr::null_mut()) },
        0
    );

    // Fallible entry points must set last-error and return NULL, not crash.
    let bad = unsafe { datalogic_engine_compile(std::ptr::null_mut(), cstr("{}").as_ptr()) };
    assert!(bad.is_null());
    let msg = unsafe { CStr::from_ptr(datalogic_last_error_message()) }
        .to_str()
        .unwrap();
    assert!(msg.contains("null"));
}

// =============== Traced session ===============

#[test]
fn traced_session_evaluate_returns_result_and_steps() {
    let engine = datalogic_engine_new(0);
    let session = unsafe { datalogic_engine_traced_session(engine) };
    assert!(!session.is_null());

    let rule = cstr(r#"{"+":[{"var":"x"},1]}"#);
    let data = cstr(r#"{"x":41}"#);
    let out = unsafe { datalogic_traced_session_evaluate(session, rule.as_ptr(), data.as_ptr()) };
    let json = unsafe { take_string(out) };
    // Wire shape: {"result": ..., "expression_tree": ..., "steps": [...]}
    let v: serde_json::Value = serde_json::from_str(&json).expect("traced run is JSON");
    assert_eq!(v["result"], serde_json::json!(42));
    assert!(v["steps"].is_array(), "steps should be an array");
    assert!(
        !v["steps"].as_array().unwrap().is_empty(),
        "steps should not be empty for a non-trivial rule"
    );
    assert!(v["expression_tree"].is_object());
    assert!(v["error"].is_null());

    unsafe { datalogic_traced_session_free(session) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn traced_session_surfaces_runtime_error_in_payload() {
    let engine = datalogic_engine_new(0);
    let session = unsafe { datalogic_engine_traced_session(engine) };

    let rule = cstr(r#"{"throw":"boom"}"#);
    let data = cstr("{}");
    let out = unsafe { datalogic_traced_session_evaluate(session, rule.as_ptr(), data.as_ptr()) };
    // Traced eval ALWAYS returns a JSON payload — engine errors live in
    // the payload's `error` field, not as a NULL return.
    let json = unsafe { take_string(out) };
    let v: serde_json::Value = serde_json::from_str(&json).expect("traced run is JSON");
    assert!(v["result"].is_null());
    let err = v["error"].as_str().expect("error message present");
    assert!(err.to_lowercase().contains("boom") || err.to_lowercase().contains("throw"));
    assert!(v["structured_error"].is_object());

    unsafe { datalogic_traced_session_free(session) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn traced_session_evaluate_with_null_pointers_sets_error() {
    datalogic_last_error_clear();
    let engine = datalogic_engine_new(0);
    let session = unsafe { datalogic_engine_traced_session(engine) };
    let data = cstr("{}");
    let out =
        unsafe { datalogic_traced_session_evaluate(session, std::ptr::null(), data.as_ptr()) };
    assert!(out.is_null());
    let msg = unsafe { CStr::from_ptr(datalogic_last_error_message()) }
        .to_str()
        .unwrap();
    assert!(msg.contains("rule_json"), "got: {msg}");
    unsafe { datalogic_traced_session_free(session) };
    unsafe { datalogic_engine_free(engine) };
}

// =============== flagd-feature operators (sem_ver, fractional) ===============

#[test]
fn flagd_sem_ver_operator_is_available() {
    // Smoke-test that the C ABI exposes the flagd feature's sem_ver op.
    let engine = datalogic_engine_new(0);
    let rule = cstr(r#"{"sem_ver":["1.2.3","<","2.0.0"]}"#);
    let data = cstr("{}");
    let out = unsafe { datalogic_engine_apply(engine, rule.as_ptr(), data.as_ptr()) };
    let s = unsafe { take_string(out) };
    assert_eq!(s, "true");
    unsafe { datalogic_engine_free(engine) };
}
