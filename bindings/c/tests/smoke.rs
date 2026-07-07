//! Smoke tests for the C ABI (v2).
//!
//! These tests link `datalogic_c` as an `rlib` and call the
//! `extern "C"` functions directly. They are not exhaustive — the full
//! operator surface is exercised by `tests/conformance.rs` (which
//! drives the JSONLogic suites through this ABI) and by the core
//! crate's own suite. This file covers the v2 contract mechanics:
//! status codes, error handles, borrowed session results, owned bufs,
//! data handles, typed results, batch, and the callback protocol.

use std::ffi::c_void;

use datalogic_c::*;

// =============== helpers ===============

fn empty_buf() -> Buf {
    Buf {
        ptr: std::ptr::null_mut(),
        len: 0,
        cap: 0,
    }
}

/// Copy an owned Buf's bytes to a String and free it.
unsafe fn take_buf(buf: Buf) -> String {
    assert!(!buf.ptr.is_null(), "expected non-null result buf");
    let s = std::str::from_utf8(unsafe { std::slice::from_raw_parts(buf.ptr, buf.len) })
        .expect("result must be UTF-8")
        .to_owned();
    unsafe { datalogic_buf_free(buf) };
    s
}

/// Copy borrowed (ptr,len) bytes to a String.
unsafe fn copy_out(ptr: *const u8, len: usize) -> String {
    assert!(!ptr.is_null(), "expected non-null borrowed result");
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) })
        .expect("result must be UTF-8")
        .to_owned()
}

/// Read an error handle's (status, message, tag, has_path) and free it.
unsafe fn take_err(err: *mut Error) -> (Status, String, String, bool) {
    assert!(!err.is_null(), "expected an error handle");
    let status = unsafe { datalogic_error_status(err) };
    let mut len = 0usize;
    let msg_ptr = unsafe { datalogic_error_message(err, &mut len) };
    let message = unsafe { copy_out(msg_ptr, len) };
    let tag_ptr = unsafe { datalogic_error_tag(err, &mut len) };
    let tag = unsafe { copy_out(tag_ptr, len) };
    let path_ptr = unsafe { datalogic_error_path_json(err, &mut len) };
    let has_path = !path_ptr.is_null();
    unsafe { datalogic_error_free(err) };
    (status, message, tag, has_path)
}

unsafe fn compile(engine: *mut Engine, rule: &str) -> *mut Rule {
    let mut out: *mut Rule = std::ptr::null_mut();
    let status = unsafe {
        datalogic_engine_compile(
            engine,
            rule.as_ptr(),
            rule.len(),
            &mut out,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok, "compile failed for {rule}");
    assert!(!out.is_null());
    out
}

unsafe fn parse_data(json: &str) -> *mut Data {
    let mut out: *mut Data = std::ptr::null_mut();
    let status =
        unsafe { datalogic_data_parse(json.as_ptr(), json.len(), &mut out, std::ptr::null_mut()) };
    assert_eq!(status, Status::Ok, "data parse failed for {json}");
    assert!(!out.is_null());
    out
}

// =============== meta ===============

#[test]
fn abi_version_is_2_and_version_matches_cargo() {
    assert_eq!(datalogic_abi_version(), 2);
    assert_eq!(datalogic_abi_version(), DATALOGIC_ABI_VERSION);

    let p = datalogic_version();
    assert!(!p.is_null());
    let v = unsafe { std::ffi::CStr::from_ptr(p) }.to_str().unwrap();
    assert_eq!(v, env!("CARGO_PKG_VERSION"));
    // Calling twice returns the same static pointer.
    assert_eq!(datalogic_version(), p);
}

// =============== one-shot / owned-buf paths ===============

#[test]
fn apply_one_shot_returns_owned_buf() {
    let engine = datalogic_engine_new(0);
    let rule = r#"{"+":[1,2]}"#;
    let data = "{}";
    let mut out = empty_buf();
    let status = unsafe {
        datalogic_engine_apply(
            engine,
            rule.as_ptr(),
            rule.len(),
            data.as_ptr(),
            data.len(),
            &mut out,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    assert_eq!(unsafe { take_buf(out) }, "3");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn compile_then_rule_evaluate_reuses_rule() {
    let engine = datalogic_engine_new(0);
    let rule = unsafe { compile(engine, r#"{"var":"x"}"#) };

    for x in [1, 7, 42] {
        let data = format!(r#"{{"x":{x}}}"#);
        let mut out = empty_buf();
        let status = unsafe {
            datalogic_rule_evaluate(
                rule,
                data.as_ptr(),
                data.len(),
                &mut out,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(status, Status::Ok);
        assert_eq!(unsafe { take_buf(out) }, x.to_string());
    }

    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

// =============== session / borrowed-result paths ===============

#[test]
fn session_evaluate_returns_borrowed_bytes() {
    let engine = datalogic_engine_new(0);
    let rule = unsafe { compile(engine, r#"{"*":[{"var":"x"},2]}"#) };
    let session = unsafe { datalogic_engine_session(engine) };
    assert!(!session.is_null());

    for x in [3, 5, 8] {
        let data = format!(r#"{{"x":{x}}}"#);
        let mut ptr: *const u8 = std::ptr::null();
        let mut len = 0usize;
        let status = unsafe {
            datalogic_session_evaluate(
                session,
                rule,
                data.as_ptr(),
                data.len(),
                &mut ptr,
                &mut len,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(status, Status::Ok);
        // Copy out before the next call — the v2 borrow contract.
        assert_eq!(unsafe { copy_out(ptr, len) }, (x * 2).to_string());
    }

    let bytes = unsafe { datalogic_session_allocated_bytes(session) };
    assert!(bytes > 0, "arena should hold the most recent call's memory");

    unsafe { datalogic_session_reset(session) };
    unsafe { datalogic_session_free(session) };
    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn session_failure_leaves_out_params_untouched() {
    let engine = datalogic_engine_new(0);
    let rule = unsafe { compile(engine, r#"{"throw":"boom"}"#) };
    let session = unsafe { datalogic_engine_session(engine) };

    static SENTINEL: u8 = 0;
    let mut ptr: *const u8 = &SENTINEL;
    let mut len = 777usize;
    let data = "{}";
    let status = unsafe {
        datalogic_session_evaluate(
            session,
            rule,
            data.as_ptr(),
            data.len(),
            &mut ptr,
            &mut len,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Eval);
    assert!(std::ptr::eq(ptr, &SENTINEL), "out_ptr must be untouched");
    assert_eq!(len, 777, "out_len must be untouched");

    unsafe { datalogic_session_free(session) };
    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn session_rejects_rule_from_other_engine() {
    let engine_a = datalogic_engine_new(0);
    let engine_b = datalogic_engine_new(0);
    let rule_b = unsafe { compile(engine_b, r#"{"var":"x"}"#) };
    let session_a = unsafe { datalogic_engine_session(engine_a) };

    let data = r#"{"x":1}"#;
    let mut ptr: *const u8 = std::ptr::null();
    let mut len = 0usize;
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_session_evaluate(
            session_a,
            rule_b,
            data.as_ptr(),
            data.len(),
            &mut ptr,
            &mut len,
            &mut err,
        )
    };
    assert_eq!(status, Status::InvalidArg);
    let (estatus, message, tag, _) = unsafe { take_err(err) };
    assert_eq!(estatus, Status::InvalidArg);
    assert_eq!(tag, "InvalidArgument");
    assert!(message.contains("different engine"), "got: {message}");

    unsafe { datalogic_session_free(session_a) };
    unsafe { datalogic_rule_free(rule_b) };
    unsafe { datalogic_engine_free(engine_a) };
    unsafe { datalogic_engine_free(engine_b) };
}

// =============== data handles ===============

#[test]
fn data_handle_evaluates_through_all_paths() {
    let engine = datalogic_engine_new(0);
    let rule = unsafe { compile(engine, r#"{">=":[{"var":"user.age"},18]}"#) };
    let data = unsafe { parse_data(r#"{"user":{"age":34}}"#) };
    assert!(unsafe { datalogic_data_allocated_bytes(data) } > 0);

    // Session path.
    let session = unsafe { datalogic_engine_session(engine) };
    for _ in 0..3 {
        let mut ptr: *const u8 = std::ptr::null();
        let mut len = 0usize;
        let status = unsafe {
            datalogic_session_evaluate_data(
                session,
                rule,
                data,
                &mut ptr,
                &mut len,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(status, Status::Ok);
        assert_eq!(unsafe { copy_out(ptr, len) }, "true");
    }

    // Session-less path (pooled arena, owned buf).
    let mut out = empty_buf();
    let status =
        unsafe { datalogic_rule_evaluate_data(rule, data, &mut out, std::ptr::null_mut()) };
    assert_eq!(status, Status::Ok);
    assert_eq!(unsafe { take_buf(out) }, "true");

    unsafe { datalogic_session_free(session) };
    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_data_free(data) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn data_handle_is_engine_independent() {
    let engine_a = datalogic_engine_new(0);
    let engine_b = datalogic_engine_new(0);
    let rule_a = unsafe { compile(engine_a, r#"{"var":"x"}"#) };
    let rule_b = unsafe { compile(engine_b, r#"{"+":[{"var":"x"},1]}"#) };
    let data = unsafe { parse_data(r#"{"x":41}"#) };

    let mut out = empty_buf();
    assert_eq!(
        unsafe { datalogic_rule_evaluate_data(rule_a, data, &mut out, std::ptr::null_mut()) },
        Status::Ok
    );
    assert_eq!(unsafe { take_buf(out) }, "41");

    let mut out = empty_buf();
    assert_eq!(
        unsafe { datalogic_rule_evaluate_data(rule_b, data, &mut out, std::ptr::null_mut()) },
        Status::Ok
    );
    assert_eq!(unsafe { take_buf(out) }, "42");

    unsafe { datalogic_data_free(data) };
    unsafe { datalogic_rule_free(rule_a) };
    unsafe { datalogic_rule_free(rule_b) };
    unsafe { datalogic_engine_free(engine_a) };
    unsafe { datalogic_engine_free(engine_b) };
}

#[test]
fn data_parse_error_reports_parse_status() {
    let mut out: *mut Data = std::ptr::null_mut();
    let mut err: *mut Error = std::ptr::null_mut();
    let bad = "{ not json";
    let status = unsafe { datalogic_data_parse(bad.as_ptr(), bad.len(), &mut out, &mut err) };
    assert_eq!(status, Status::Parse);
    assert!(out.is_null());
    let (estatus, message, tag, _) = unsafe { take_err(err) };
    assert_eq!(estatus, Status::Parse);
    assert_eq!(tag, "ParseError");
    assert!(!message.is_empty());
}

// =============== typed results ===============

#[test]
fn typed_evaluations_extract_scalars() {
    let engine = datalogic_engine_new(0);
    let session = unsafe { datalogic_engine_session(engine) };
    let data = unsafe { parse_data(r#"{"age":34,"score":7.5,"items":[1]}"#) };

    // bool
    let rule = unsafe { compile(engine, r#"{">=":[{"var":"age"},18]}"#) };
    let mut b: i32 = -1;
    assert_eq!(
        unsafe {
            datalogic_session_evaluate_bool(session, rule, data, &mut b, std::ptr::null_mut())
        },
        Status::Ok
    );
    assert_eq!(b, 1);
    unsafe { datalogic_rule_free(rule) };

    // i64
    let rule = unsafe { compile(engine, r#"{"+":[{"var":"age"},8]}"#) };
    let mut i: i64 = -1;
    assert_eq!(
        unsafe {
            datalogic_session_evaluate_i64(session, rule, data, &mut i, std::ptr::null_mut())
        },
        Status::Ok
    );
    assert_eq!(i, 42);
    unsafe { datalogic_rule_free(rule) };

    // f64
    let rule = unsafe { compile(engine, r#"{"*":[{"var":"score"},2]}"#) };
    let mut f: f64 = -1.0;
    assert_eq!(
        unsafe {
            datalogic_session_evaluate_f64(session, rule, data, &mut f, std::ptr::null_mut())
        },
        Status::Ok
    );
    assert!((f - 15.0).abs() < 1e-9);
    unsafe { datalogic_rule_free(rule) };

    // truthy: a non-empty array coerces to true, and never mismatches.
    let rule = unsafe { compile(engine, r#"{"var":"items"}"#) };
    let mut t: i32 = -1;
    assert_eq!(
        unsafe {
            datalogic_session_evaluate_truthy(session, rule, data, &mut t, std::ptr::null_mut())
        },
        Status::Ok
    );
    assert_eq!(t, 1);
    unsafe { datalogic_rule_free(rule) };

    // Type mismatch: array is not a bool.
    let rule = unsafe { compile(engine, r#"{"var":"items"}"#) };
    let mut b2: i32 = -1;
    let mut err: *mut Error = std::ptr::null_mut();
    assert_eq!(
        unsafe { datalogic_session_evaluate_bool(session, rule, data, &mut b2, &mut err) },
        Status::TypeMismatch
    );
    assert_eq!(b2, -1, "out must be untouched on mismatch");
    let (estatus, message, tag, _) = unsafe { take_err(err) };
    assert_eq!(estatus, Status::TypeMismatch);
    assert_eq!(tag, "TypeMismatch");
    assert!(message.contains("array"), "got: {message}");
    unsafe { datalogic_rule_free(rule) };

    unsafe { datalogic_data_free(data) };
    unsafe { datalogic_session_free(session) };
    unsafe { datalogic_engine_free(engine) };
}

// =============== batch ===============

#[test]
fn batch_one_rule_many_datas_with_item_failures() {
    let engine = datalogic_engine_new(0);
    let rule = unsafe { compile(engine, r#"{"+":[{"var":"x"},1]}"#) };
    let session = unsafe { datalogic_engine_session(engine) };

    let good_a = unsafe { parse_data(r#"{"x":1}"#) };
    let bad = unsafe { parse_data(r#"{"x":"nope"}"#) }; // + on a non-numeric string errors
    let good_b = unsafe { parse_data(r#"{"x":41}"#) };
    let datas: [*const Data; 4] = [good_a, bad, std::ptr::null(), good_b];

    let mut results = [const {
        Slice {
            ptr: std::ptr::null(),
            len: 0,
        }
    }; 4];
    let mut statuses = [Status::Internal; 4];
    let status = unsafe {
        datalogic_session_evaluate_batch(
            session,
            rule,
            datas.as_ptr(),
            datas.len(),
            results.as_mut_ptr(),
            statuses.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok, "item failures never fail the call");

    assert_eq!(statuses[0], Status::Ok);
    assert_eq!(unsafe { copy_out(results[0].ptr, results[0].len) }, "2");

    assert_eq!(statuses[1], Status::Eval);
    let item_err: serde_json::Value =
        serde_json::from_str(&unsafe { copy_out(results[1].ptr, results[1].len) }).unwrap();
    assert!(item_err["tag"].is_string());
    assert!(item_err["message"].is_string());

    assert_eq!(statuses[2], Status::InvalidArg);
    let item_err: serde_json::Value =
        serde_json::from_str(&unsafe { copy_out(results[2].ptr, results[2].len) }).unwrap();
    assert_eq!(item_err["tag"], "InvalidArgument");

    assert_eq!(statuses[3], Status::Ok);
    assert_eq!(unsafe { copy_out(results[3].ptr, results[3].len) }, "42");

    unsafe { datalogic_data_free(good_a) };
    unsafe { datalogic_data_free(bad) };
    unsafe { datalogic_data_free(good_b) };
    unsafe { datalogic_session_free(session) };
    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn batch_slices_survive_result_buffer_growth() {
    // Large string results force the shared result buffer to reallocate
    // mid-loop; the returned slices must all still be correct (the
    // implementation materialises pointers only after the last write).
    let engine = datalogic_engine_new(0);
    let rule = unsafe { compile(engine, r#"{"var":"s"}"#) };
    let session = unsafe { datalogic_engine_session(engine) };

    const N: usize = 8;
    let payloads: Vec<String> = (0..N)
        .map(|i| {
            let filler = "x".repeat(600 + i);
            format!(r#"{{"s":"{filler}"}}"#)
        })
        .collect();
    let handles: Vec<*mut Data> = payloads.iter().map(|p| unsafe { parse_data(p) }).collect();
    let datas: Vec<*const Data> = handles.iter().map(|h| *h as *const Data).collect();

    let mut results = vec![
        Slice {
            ptr: std::ptr::null(),
            len: 0
        };
        N
    ];
    let mut statuses = vec![Status::Internal; N];
    let status = unsafe {
        datalogic_session_evaluate_batch(
            session,
            rule,
            datas.as_ptr(),
            N,
            results.as_mut_ptr(),
            statuses.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    for (i, (slice, status)) in results.iter().zip(&statuses).enumerate() {
        assert_eq!(*status, Status::Ok);
        let got = unsafe { copy_out(slice.ptr, slice.len) };
        let want = format!(r#""{}""#, "x".repeat(600 + i));
        assert_eq!(got, want, "batch item {i} corrupted by buffer growth");
    }

    for h in handles {
        unsafe { datalogic_data_free(h) };
    }
    unsafe { datalogic_session_free(session) };
    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn evaluate_many_rules_against_one_payload() {
    let engine = datalogic_engine_new(0);
    let other_engine = datalogic_engine_new(0);
    let session = unsafe { datalogic_engine_session(engine) };
    let data = unsafe { parse_data(r#"{"age":34,"country":"US"}"#) };

    let adult = unsafe { compile(engine, r#"{">=":[{"var":"age"},18]}"#) };
    let senior = unsafe { compile(engine, r#"{">=":[{"var":"age"},65]}"#) };
    let foreign = unsafe { compile(other_engine, r#"{"var":"age"}"#) };
    let rules: [*const Rule; 3] = [adult, senior, foreign];

    let mut results = [const {
        Slice {
            ptr: std::ptr::null(),
            len: 0,
        }
    }; 3];
    let mut statuses = [Status::Internal; 3];
    let status = unsafe {
        datalogic_session_evaluate_many(
            session,
            rules.as_ptr(),
            rules.len(),
            data,
            results.as_mut_ptr(),
            statuses.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    assert_eq!(statuses[0], Status::Ok);
    assert_eq!(unsafe { copy_out(results[0].ptr, results[0].len) }, "true");
    assert_eq!(statuses[1], Status::Ok);
    assert_eq!(unsafe { copy_out(results[1].ptr, results[1].len) }, "false");
    assert_eq!(statuses[2], Status::InvalidArg, "foreign-engine rule");

    unsafe { datalogic_data_free(data) };
    unsafe { datalogic_rule_free(adult) };
    unsafe { datalogic_rule_free(senior) };
    unsafe { datalogic_rule_free(foreign) };
    unsafe { datalogic_session_free(session) };
    unsafe { datalogic_engine_free(engine) };
    unsafe { datalogic_engine_free(other_engine) };
}

// =============== errors ===============

#[test]
fn parse_error_carries_status_and_tag() {
    let engine = datalogic_engine_new(0);
    let bad_rule = "not-valid-json{{";
    let mut out: *mut Rule = std::ptr::null_mut();
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_engine_compile(
            engine,
            bad_rule.as_ptr(),
            bad_rule.len(),
            &mut out,
            &mut err,
        )
    };
    assert_eq!(status, Status::Parse);
    assert!(out.is_null());
    let (estatus, message, tag, _) = unsafe { take_err(err) };
    assert_eq!(estatus, Status::Parse);
    assert_eq!(tag, "ParseError");
    assert!(!message.is_empty());
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn evaluate_error_carries_tag_and_path() {
    let engine = datalogic_engine_new(0);
    let rule = unsafe { compile(engine, r#"{"throw":"boom"}"#) };
    let data = "{}";
    let mut out = empty_buf();
    let mut err: *mut Error = std::ptr::null_mut();
    let status =
        unsafe { datalogic_rule_evaluate(rule, data.as_ptr(), data.len(), &mut out, &mut err) };
    assert_eq!(status, Status::Eval);
    let (estatus, _message, tag, has_path) = unsafe { take_err(err) };
    assert_eq!(estatus, Status::Eval);
    assert_eq!(tag, "Thrown");
    assert!(
        has_path,
        "path JSON should be resolvable with &Logic in scope"
    );
    unsafe { datalogic_rule_free(rule) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn null_err_out_param_skips_capture() {
    let engine = datalogic_engine_new(0);
    let bad_rule = "nope{{";
    let mut out: *mut Rule = std::ptr::null_mut();
    // err == NULL must be fully supported (and leak nothing).
    let status = unsafe {
        datalogic_engine_compile(
            engine,
            bad_rule.as_ptr(),
            bad_rule.len(),
            &mut out,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Parse);
    assert!(out.is_null());
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn null_pointers_are_handled_without_segfault() {
    // Free entry points are explicitly NULL-safe.
    unsafe { datalogic_engine_free(std::ptr::null_mut()) };
    unsafe { datalogic_rule_free(std::ptr::null_mut()) };
    unsafe { datalogic_session_free(std::ptr::null_mut()) };
    unsafe { datalogic_traced_session_free(std::ptr::null_mut()) };
    unsafe { datalogic_data_free(std::ptr::null_mut()) };
    unsafe { datalogic_error_free(std::ptr::null_mut()) };
    unsafe { datalogic_buf_free(empty_buf()) };
    unsafe { datalogic_session_reset(std::ptr::null_mut()) };
    assert_eq!(
        unsafe { datalogic_session_allocated_bytes(std::ptr::null()) },
        0
    );
    assert_eq!(
        unsafe { datalogic_data_allocated_bytes(std::ptr::null()) },
        0
    );

    // Fallible entry points must return InvalidArg with an error handle.
    let mut out: *mut Rule = std::ptr::null_mut();
    let mut err: *mut Error = std::ptr::null_mut();
    let rule = "{}";
    let status = unsafe {
        datalogic_engine_compile(
            std::ptr::null(),
            rule.as_ptr(),
            rule.len(),
            &mut out,
            &mut err,
        )
    };
    assert_eq!(status, Status::InvalidArg);
    let (_, message, tag, _) = unsafe { take_err(err) };
    assert_eq!(tag, "InvalidArgument");
    assert!(message.contains("null"), "got: {message}");

    // NULL engine -> NULL session/traced-session handles.
    assert!(unsafe { datalogic_engine_session(std::ptr::null()) }.is_null());
    assert!(unsafe { datalogic_engine_traced_session(std::ptr::null()) }.is_null());
}

// =============== engine builder + custom operators (callback v2) ===============

/// Test callback: `[n]` -> `n*2` via set_json.
unsafe extern "C" fn double_op(
    args_json: *const u8,
    args_len: usize,
    _user_data: *mut c_void,
    out: *mut OpResult,
) -> i32 {
    let args = std::str::from_utf8(unsafe { std::slice::from_raw_parts(args_json, args_len) })
        .expect("args are UTF-8");
    let inner: f64 = args[1..args.len() - 1].trim().parse().expect("one number");
    let result = format!("{}", inner * 2.0);
    unsafe { datalogic_op_result_set_json(out, result.as_ptr(), result.len()) };
    0
}

/// Test callback that fails with a message.
unsafe extern "C" fn boom_op(
    _args_json: *const u8,
    _args_len: usize,
    _user_data: *mut c_void,
    out: *mut OpResult,
) -> i32 {
    let msg = "custom-failure";
    unsafe { datalogic_op_result_set_error(out, msg.as_ptr(), msg.len()) };
    1
}

/// Test callback that reads `user_data` (an i64 pointer) and adds it.
unsafe extern "C" fn add_user_data_op(
    args_json: *const u8,
    args_len: usize,
    user_data: *mut c_void,
    out: *mut OpResult,
) -> i32 {
    let bias = unsafe { *(user_data as *const i64) };
    let args = std::str::from_utf8(unsafe { std::slice::from_raw_parts(args_json, args_len) })
        .expect("args are UTF-8");
    let inner: i64 = args[1..args.len() - 1].trim().parse().expect("one int");
    let result = format!("{}", inner + bias);
    unsafe { datalogic_op_result_set_json(out, result.as_ptr(), result.len()) };
    0
}

/// Test callback that succeeds without setting a result -> JSON null.
unsafe extern "C" fn silent_op(
    _args_json: *const u8,
    _args_len: usize,
    _user_data: *mut c_void,
    _out: *mut OpResult,
) -> i32 {
    0
}

unsafe fn build_with_operator(
    name: &str,
    cb: DatalogicOpFn,
    user_data: *mut c_void,
) -> *mut Engine {
    let b = datalogic_engine_builder_new();
    let status = unsafe {
        datalogic_engine_builder_add_operator(
            b,
            name.as_ptr(),
            name.len(),
            cb,
            user_data,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    let engine = unsafe { datalogic_engine_builder_build(b) };
    assert!(!engine.is_null());
    unsafe { datalogic_engine_builder_free(b) };
    engine
}

unsafe fn apply_str(
    engine: *mut Engine,
    rule: &str,
    data: &str,
) -> Result<String, (Status, String, String)> {
    let mut out = empty_buf();
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_engine_apply(
            engine,
            rule.as_ptr(),
            rule.len(),
            data.as_ptr(),
            data.len(),
            &mut out,
            &mut err,
        )
    };
    if status == Status::Ok {
        Ok(unsafe { take_buf(out) })
    } else {
        let (estatus, message, tag, _) = unsafe { take_err(err) };
        assert_eq!(estatus, status);
        Err((status, message, tag))
    }
}

#[test]
fn builder_with_custom_operator_evaluates() {
    let engine = unsafe { build_with_operator("double", Some(double_op), std::ptr::null_mut()) };
    let got = unsafe { apply_str(engine, r#"{"double":[21]}"#, "{}") }.unwrap();
    assert_eq!(got, "42");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_custom_operator_error_propagates() {
    let engine = unsafe { build_with_operator("boom", Some(boom_op), std::ptr::null_mut()) };
    let (status, message, _tag) = unsafe { apply_str(engine, r#"{"boom":[]}"#, "{}") }.unwrap_err();
    assert_eq!(status, Status::Eval);
    assert!(message.contains("custom-failure"), "got: {message}");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_custom_operator_receives_user_data() {
    let bias: i64 = 100;
    let engine = unsafe {
        build_with_operator(
            "addbias",
            Some(add_user_data_op),
            &bias as *const i64 as *mut c_void,
        )
    };
    let got = unsafe { apply_str(engine, r#"{"addbias":[7]}"#, "{}") }.unwrap();
    assert_eq!(got, "107");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_custom_operator_without_result_yields_null() {
    let engine = unsafe { build_with_operator("silent", Some(silent_op), std::ptr::null_mut()) };
    let got = unsafe { apply_str(engine, r#"{"silent":[1,2]}"#, "{}") }.unwrap();
    assert_eq!(got, "null");
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_rejects_null_callback() {
    let b = datalogic_engine_builder_new();
    let name = "nope";
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_engine_builder_add_operator(
            b,
            name.as_ptr(),
            name.len(),
            None,
            std::ptr::null_mut(),
            &mut err,
        )
    };
    assert_eq!(status, Status::InvalidArg);
    let (_, _, tag, _) = unsafe { take_err(err) };
    assert_eq!(tag, "InvalidArgument");
    let engine = unsafe { datalogic_engine_builder_build(b) };
    assert!(!engine.is_null(), "failed add leaves the builder usable");
    unsafe { datalogic_engine_free(engine) };
    unsafe { datalogic_engine_builder_free(b) };
}

#[test]
fn builder_set_config_json_strict_preset_takes_effect() {
    // Default config: `{"+": [null, 1]}` coerces null to 0 and returns 1.
    let engine = datalogic_engine_new(0);
    let got = unsafe { apply_str(engine, r#"{"+":[null,1]}"#, "{}") }.unwrap();
    assert_eq!(got, "1");
    unsafe { datalogic_engine_free(engine) };

    // Strict preset: the same rule rejects the non-numeric null.
    let b = datalogic_engine_builder_new();
    let config = r#"{"preset":"strict"}"#;
    let status = unsafe {
        datalogic_engine_builder_set_config_json(
            b,
            config.as_ptr(),
            config.len(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    let engine = unsafe { datalogic_engine_builder_build(b) };
    unsafe { datalogic_engine_builder_free(b) };

    let (status, message, _tag) =
        unsafe { apply_str(engine, r#"{"+":[null,1]}"#, "{}") }.unwrap_err();
    assert_eq!(status, Status::Eval);
    assert!(!message.is_empty());
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn builder_set_config_json_rejects_bad_input() {
    let b = datalogic_engine_builder_new();

    // Malformed JSON -> Parse.
    let bad = "not-json{{";
    let mut err: *mut Error = std::ptr::null_mut();
    let status =
        unsafe { datalogic_engine_builder_set_config_json(b, bad.as_ptr(), bad.len(), &mut err) };
    let (estatus, message, _, _) = unsafe { take_err(err) };
    assert_eq!(status, estatus);
    assert!(!message.is_empty());

    // Unknown enum value -> error naming the typo.
    let bogus = r#"{"preset":"bogus"}"#;
    let mut err: *mut Error = std::ptr::null_mut();
    let _ = unsafe {
        datalogic_engine_builder_set_config_json(b, bogus.as_ptr(), bogus.len(), &mut err)
    };
    let (_, message, _, _) = unsafe { take_err(err) };
    assert!(message.contains("bogus"), "got: {message}");

    // NULL builder -> InvalidArg.
    let good = r#"{"preset":"strict"}"#;
    let status = unsafe {
        datalogic_engine_builder_set_config_json(
            std::ptr::null_mut(),
            good.as_ptr(),
            good.len(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::InvalidArg);

    // A failed set_config_json leaves the builder usable.
    let engine = unsafe { datalogic_engine_builder_build(b) };
    assert!(!engine.is_null());
    unsafe { datalogic_engine_free(engine) };
    unsafe { datalogic_engine_builder_free(b) };
}

#[test]
fn builder_build_twice_returns_null() {
    let b = datalogic_engine_builder_new();
    let engine = unsafe { datalogic_engine_builder_build(b) };
    assert!(!engine.is_null());
    let again = unsafe { datalogic_engine_builder_build(b) };
    assert!(again.is_null(), "second build must fail");
    unsafe { datalogic_engine_free(engine) };
    unsafe { datalogic_engine_builder_free(b) };
}

// =============== traced session ===============

#[test]
fn traced_session_evaluate_returns_result_and_steps() {
    let engine = datalogic_engine_new(0);
    let session = unsafe { datalogic_engine_traced_session(engine) };
    assert!(!session.is_null());

    let rule = r#"{"+":[{"var":"x"},1]}"#;
    let data = r#"{"x":41}"#;
    let mut out = empty_buf();
    let status = unsafe {
        datalogic_traced_session_evaluate(
            session,
            rule.as_ptr(),
            rule.len(),
            data.as_ptr(),
            data.len(),
            &mut out,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    let json = unsafe { take_buf(out) };
    let v: serde_json::Value = serde_json::from_str(&json).expect("traced run is JSON");
    assert_eq!(v["result"], serde_json::json!(42));
    assert!(v["steps"].is_array());
    assert!(!v["steps"].as_array().unwrap().is_empty());
    assert!(v["expression_tree"].is_object());
    assert!(v["error"].is_null());

    unsafe { datalogic_traced_session_free(session) };
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn traced_session_surfaces_runtime_error_in_payload() {
    let engine = datalogic_engine_new(0);
    let session = unsafe { datalogic_engine_traced_session(engine) };

    let rule = r#"{"throw":"boom"}"#;
    let data = "{}";
    let mut out = empty_buf();
    let status = unsafe {
        datalogic_traced_session_evaluate(
            session,
            rule.as_ptr(),
            rule.len(),
            data.as_ptr(),
            data.len(),
            &mut out,
            std::ptr::null_mut(),
        )
    };
    // Traced eval ALWAYS returns a JSON payload — engine errors live in
    // the payload's `error` field, not in the status.
    assert_eq!(status, Status::Ok);
    let json = unsafe { take_buf(out) };
    let v: serde_json::Value = serde_json::from_str(&json).expect("traced run is JSON");
    assert!(v["result"].is_null());
    let err = v["error"].as_str().expect("error message present");
    assert!(err.to_lowercase().contains("boom") || err.to_lowercase().contains("throw"));
    assert!(v["structured_error"].is_object());

    unsafe { datalogic_traced_session_free(session) };
    unsafe { datalogic_engine_free(engine) };
}

// =============== flagd-feature operators ===============

#[test]
fn flagd_sem_ver_operator_is_available() {
    let engine = datalogic_engine_new(0);
    let got = unsafe { apply_str(engine, r#"{"sem_ver":["1.2.3","<","2.0.0"]}"#, "{}") }.unwrap();
    assert_eq!(got, "true");
    unsafe { datalogic_engine_free(engine) };
}
