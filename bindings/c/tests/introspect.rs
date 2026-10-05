//! ABI v2 minor 1: per-compile modes, check / compile_checked, operators,
//! truthy, rule facts, metered sessions, builder escape and strict names,
//! and the new error accessors, all driven through the `extern "C"`
//! entry points.

use datalogic_c::*;
use serde_json::Value;

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

#[test]
fn minor_version() {
    assert_eq!(datalogic_abi_version(), 2);
    assert_eq!(datalogic_abi_minor(), 1);
}

#[test]
fn check_and_compile_checked() {
    let engine = datalogic_engine_new(0);
    let rule = br#"{"if": [{"bogus": 1}, {"map": [1]}]}"#;
    let mut out = empty();
    let status = unsafe {
        datalogic_engine_check(
            engine,
            rule.as_ptr(),
            rule.len(),
            0,
            &mut out,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    let diags = take(out);
    assert_eq!(diags[0]["code"], "UnknownOperator");
    assert_eq!(diags[1]["pointer"], "/if/1");

    let mut compiled: *mut Rule = std::ptr::null_mut();
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_engine_compile_checked(engine, rule.as_ptr(), rule.len(), &mut compiled, &mut err)
    };
    assert_eq!(status, Status::Parse);
    let mut len = 0;
    let tag = unsafe { datalogic_error_tag(err, &mut len) };
    assert_eq!(s(tag, len), "CompileError");
    let diags = unsafe { datalogic_error_diagnostics_json(err, &mut len) };
    let diags: Value = serde_json::from_str(&s(diags, len)).unwrap();
    assert_eq!(diags.as_array().unwrap().len(), 2);
    unsafe { datalogic_error_free(err) };

    // An unknown mode is an argument error.
    let mut out = empty();
    let status = unsafe {
        datalogic_engine_check(
            engine,
            rule.as_ptr(),
            rule.len(),
            9,
            &mut out,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::InvalidArg);
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn compile_modes_and_facts() {
    let engine = datalogic_engine_new(0);
    let template = br#"{"user": {"var": "name"}, "n": 1}"#;
    let mut rule: *mut Rule = std::ptr::null_mut();
    let compile = |mode: u32, rule: &mut *mut Rule| unsafe {
        datalogic_engine_compile_mode(
            engine,
            template.as_ptr(),
            template.len(),
            mode,
            rule,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(compile(0, &mut rule), Status::Ok);
    assert_ne!(compile(1, &mut rule), Status::Ok);
    assert_eq!(compile(2, &mut rule), Status::Ok);
    let data = br#"{"name": "ana"}"#;
    let mut out = empty();
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
    assert_eq!(take(out), serde_json::json!({"user": "ana", "n": 1}));

    let mut facts = empty();
    assert_eq!(
        unsafe { datalogic_rule_facts(rule, &mut facts, std::ptr::null_mut()) },
        Status::Ok
    );
    assert_eq!(take(facts)["reads"], serde_json::json!([["name"]]));
    unsafe {
        datalogic_rule_free(rule);
        datalogic_engine_free(engine);
    }
}

#[test]
fn operators_and_truthy() {
    let engine = datalogic_engine_new(0);
    let mut out = empty();
    assert_eq!(
        unsafe { datalogic_engine_operators(engine, &mut out, std::ptr::null_mut()) },
        Status::Ok
    );
    let docs = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/src/operators/operators.json"
    );
    let docs: Value = serde_json::from_str(&std::fs::read_to_string(docs).unwrap()).unwrap();
    assert_eq!(take(out), docs);

    let mut truthy = -1;
    for (json, want) in [("{}", 0), ("[]", 0), (r#"{"a":1}"#, 1), (r#""0""#, 1)] {
        let status = unsafe {
            datalogic_engine_truthy(
                engine,
                json.as_ptr(),
                json.len(),
                &mut truthy,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(status, Status::Ok);
        assert_eq!(truthy, want, "{json}");
    }
    let bad = "nope";
    let status = unsafe {
        datalogic_engine_truthy(
            engine,
            bad.as_ptr(),
            bad.len(),
            &mut truthy,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Parse);
    unsafe { datalogic_engine_free(engine) };
}

#[test]
fn metered_session() {
    let engine = datalogic_engine_new(0);
    let src = br#"{"map": [{"var": "xs"}, {"+": [{"var": ""}, 1]}]}"#;
    let mut rule: *mut Rule = std::ptr::null_mut();
    unsafe {
        datalogic_engine_compile(
            engine,
            src.as_ptr(),
            src.len(),
            &mut rule,
            std::ptr::null_mut(),
        )
    };
    let session = unsafe { datalogic_engine_session(engine) };
    let data = br#"{"xs": [1, 2, 3]}"#;
    let (mut ptr, mut len, mut ops) = (std::ptr::null(), 0usize, 0u64);
    let status = unsafe {
        datalogic_session_evaluate_metered(
            session,
            rule,
            data.as_ptr(),
            data.len(),
            0,
            &mut ptr,
            &mut len,
            &mut ops,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    assert_eq!(s(ptr, len), "[2,3,4]");
    assert!(ops > 0);
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_session_evaluate_metered(
            session,
            rule,
            data.as_ptr(),
            data.len(),
            2,
            &mut ptr,
            &mut len,
            &mut ops,
            &mut err,
        )
    };
    assert_eq!(status, Status::Eval);
    let tag = unsafe { datalogic_error_tag(err, &mut len) };
    assert_eq!(s(tag, len), "BudgetExceeded");
    unsafe {
        datalogic_error_free(err);
        datalogic_session_free(session);
        datalogic_rule_free(rule);
        datalogic_engine_free(engine);
    }
}

#[test]
fn error_node_ids() {
    let engine = datalogic_engine_new(0);
    let rule = br#"{"+": ["a", 1]}"#;
    let mut out = empty();
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_engine_apply(
            engine,
            rule.as_ptr(),
            rule.len(),
            b"null".as_ptr(),
            4,
            &mut out,
            &mut err,
        )
    };
    assert_eq!(status, Status::Eval);
    let mut len = 0;
    let ids = unsafe { datalogic_error_node_ids_json(err, &mut len) };
    let ids: Value = serde_json::from_str(&s(ids, len)).unwrap();
    assert!(!ids.as_array().unwrap().is_empty());
    unsafe {
        datalogic_error_free(err);
        datalogic_engine_free(engine);
    }
}

unsafe extern "C" fn one(
    _args: *const u8,
    _len: usize,
    _user: *mut std::ffi::c_void,
    out: *mut OpResult,
) -> i32 {
    unsafe { datalogic_op_result_set_json(out, b"1".as_ptr(), 1) };
    0
}

#[test]
fn builder_escape_and_strict_names() {
    let builder = datalogic_engine_builder_new();
    unsafe {
        datalogic_engine_builder_set_templating(builder, 1);
        assert_eq!(
            datalogic_engine_builder_set_template_key_escape(
                builder,
                '$' as u32,
                std::ptr::null_mut()
            ),
            Status::Ok
        );
        datalogic_engine_builder_set_strict_operator_names(builder, 1);
    }
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_engine_builder_add_operator(
            builder,
            b"length".as_ptr(),
            6,
            Some(one),
            std::ptr::null_mut(),
            &mut err,
        )
    };
    assert_eq!(status, Status::Eval);
    let mut len = 0;
    let tag = unsafe { datalogic_error_tag(err, &mut len) };
    assert_eq!(s(tag, len), "ConfigurationError");
    unsafe { datalogic_error_free(err) };
    let status = unsafe {
        datalogic_engine_builder_add_operator(
            builder,
            b"uno".as_ptr(),
            3,
            Some(one),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    let engine = unsafe { datalogic_engine_builder_build(builder) };
    let rule = br#"{"$type": {"uno": []}, "k": 2}"#;
    let mut out = empty();
    let status = unsafe {
        datalogic_engine_apply(
            engine,
            rule.as_ptr(),
            rule.len(),
            b"null".as_ptr(),
            4,
            &mut out,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::Ok);
    assert_eq!(take(out), serde_json::json!({"type": 1, "k": 2}));
    unsafe {
        datalogic_engine_builder_free(builder);
        datalogic_engine_free(engine);
    }
}
