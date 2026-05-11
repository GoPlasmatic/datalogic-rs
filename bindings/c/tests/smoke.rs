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
    assert!(path.starts_with('['), "path should be a JSON array, got: {path}");

    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn null_pointers_are_handled_without_segfault() {
    // Free entry points are explicitly NULL-safe.
    unsafe { datalogic_engine_free(std::ptr::null_mut()) };
    unsafe { datalogic_rule_free(std::ptr::null_mut()) };
    unsafe { datalogic_session_free(std::ptr::null_mut()) };
    unsafe { datalogic_string_free(std::ptr::null_mut()) };
    unsafe { datalogic_session_reset(std::ptr::null_mut()) };
    assert_eq!(unsafe { datalogic_session_allocated_bytes(std::ptr::null_mut()) }, 0);

    // Fallible entry points must set last-error and return NULL, not crash.
    let bad = unsafe {
        datalogic_engine_compile(std::ptr::null_mut(), cstr("{}").as_ptr())
    };
    assert!(bad.is_null());
    let msg = unsafe { CStr::from_ptr(datalogic_last_error_message()) }
        .to_str()
        .unwrap();
    assert!(msg.contains("null"));
}
