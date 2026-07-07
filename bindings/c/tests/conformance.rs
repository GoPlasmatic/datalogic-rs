//! JSONLogic conformance suite, driven through the C ABI (v2) itself.
//!
//! The core crate's `test_jsonlogic.rs` runner exercises the engine
//! through the native Rust API; this runner walks the *same* suites
//! (`crates/datalogic-rs/tests/suites/`, discovered via `index.json`)
//! but marshals every rule/data/result across the C boundary via the
//! `extern "C"` entry points, exactly as a Go/PHP/JVM/.NET consumer
//! would.
//!
//! Every case runs through **two** v2 paths and both must agree with
//! the expectation:
//!
//! 1. `datalogic_engine_apply` — the one-shot string path (owned buf).
//! 2. `datalogic_engine_compile` + `datalogic_data_parse` +
//!    `datalogic_session_evaluate_data` — the parse-once hot path
//!    (borrowed result), which gives the data-handle machinery
//!    full-suite coverage.
//!
//! Semantics per test case:
//! - `result` cases: both paths must return `DATALOGIC_STATUS_OK` with
//!   JSON that parses `serde_json::Value`-equal to the expected value.
//! - `error` cases: both paths must return a non-OK status with a
//!   non-empty error message. (The C ABI collapses the expected error
//!   *object* into status + tag + message, so unlike the core runner we
//!   assert "an error surfaced", not its exact shape.)
//!
//! The `flagd/` suites run unconditionally: `datalogic-c`'s Cargo.toml
//! hard-enables the `flagd` feature on the core crate.

use datalogic_c::*;
use serde_json::{Value, json};

/// Suites live in the core crate; resolve relative to this crate's
/// manifest so the test is independent of the harness cwd.
const SUITES_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/datalogic-rs/tests/suites"
);

fn empty_buf() -> Buf {
    Buf {
        ptr: std::ptr::null_mut(),
        len: 0,
        cap: 0,
    }
}

unsafe fn take_buf(buf: Buf) -> String {
    let s = std::str::from_utf8(unsafe { std::slice::from_raw_parts(buf.ptr, buf.len) })
        .expect("result must be UTF-8")
        .to_owned();
    unsafe { datalogic_buf_free(buf) };
    s
}

/// Read message from an error handle and free it; empty string for NULL.
unsafe fn take_err_message(err: *mut Error) -> String {
    if err.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    let ptr = unsafe { datalogic_error_message(err, &mut len) };
    let msg = if ptr.is_null() {
        String::new()
    } else {
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(ptr, len) }).into_owned()
    };
    unsafe { datalogic_error_free(err) };
    msg
}

/// Path 1: one-shot string apply.
unsafe fn eval_apply(engine: *mut Engine, rule: &str, data: &str) -> Result<String, String> {
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
        let msg = unsafe { take_err_message(err) };
        Err(if msg.is_empty() {
            format!("(no message, status {status:?})")
        } else {
            msg
        })
    }
}

/// Path 2: compile + data handle + session (borrowed result).
unsafe fn eval_session_data(
    engine: *mut Engine,
    session: *mut Session,
    rule: &str,
    data: &str,
) -> Result<String, String> {
    // Compile.
    let mut rule_h: *mut Rule = std::ptr::null_mut();
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_engine_compile(engine, rule.as_ptr(), rule.len(), &mut rule_h, &mut err)
    };
    if status != Status::Ok {
        return Err(unsafe { take_err_message(err) });
    }

    // Parse the payload once.
    let mut data_h: *mut Data = std::ptr::null_mut();
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe { datalogic_data_parse(data.as_ptr(), data.len(), &mut data_h, &mut err) };
    if status != Status::Ok {
        unsafe { datalogic_rule_free(rule_h) };
        return Err(unsafe { take_err_message(err) });
    }

    // Evaluate via the borrowed-result hot path.
    let mut ptr: *const u8 = std::ptr::null();
    let mut len = 0usize;
    let mut err: *mut Error = std::ptr::null_mut();
    let status = unsafe {
        datalogic_session_evaluate_data(session, rule_h, data_h, &mut ptr, &mut len, &mut err)
    };
    let outcome = if status == Status::Ok {
        // Copy out before anything else touches the session.
        Ok(String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(ptr, len) }).into_owned())
    } else {
        let msg = unsafe { take_err_message(err) };
        Err(if msg.is_empty() {
            format!("(no message, status {status:?})")
        } else {
            msg
        })
    };

    unsafe { datalogic_data_free(data_h) };
    unsafe { datalogic_rule_free(rule_h) };
    outcome
}

struct SuiteOutcome {
    passed: usize,
    failed: usize,
}

#[test]
fn conformance_suites_pass_through_c_abi() {
    let index_path = format!("{SUITES_ROOT}/index.json");
    let index_contents = std::fs::read_to_string(&index_path)
        .unwrap_or_else(|e| panic!("failed to read {index_path}: {e}"));
    let index: Vec<String> =
        serde_json::from_str(&index_contents).expect("index.json is a JSON array of file names");

    // Engines are stateless across evaluations — share one per
    // templating mode; sessions are single-threaded but so is this test.
    let engine_plain = datalogic_engine_new(0);
    let engine_templating = datalogic_engine_new(1);
    assert!(!engine_plain.is_null() && !engine_templating.is_null());
    let session_plain = unsafe { datalogic_engine_session(engine_plain) };
    let session_templating = unsafe { datalogic_engine_session(engine_templating) };
    assert!(!session_plain.is_null() && !session_templating.is_null());

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for suite_file in &index {
        let path = format!("{SUITES_ROOT}/{suite_file}");
        if !std::path::Path::new(&path).exists() {
            // Mirror the core runner: a stale index entry is a warning,
            // not a failure, so the index can stay ahead of checkouts.
            println!("WARNING: skipping {suite_file} (file not found)");
            continue;
        }

        let outcome = run_suite(
            suite_file,
            &path,
            (engine_plain, session_plain),
            (engine_templating, session_templating),
            &mut failures,
        );
        println!(
            "{suite_file}: {} passed, {} failed",
            outcome.passed, outcome.failed
        );
        total_passed += outcome.passed;
        total_failed += outcome.failed;
    }

    unsafe { datalogic_session_free(session_plain) };
    unsafe { datalogic_session_free(session_templating) };
    unsafe { datalogic_engine_free(engine_plain) };
    unsafe { datalogic_engine_free(engine_templating) };

    println!("\nTOTAL (via C ABI v2, both paths): {total_passed} passed, {total_failed} failed");
    assert!(
        total_failed == 0,
        "{total_failed} conformance case(s) failed through the C ABI:\n{}",
        failures.join("\n")
    );
    assert!(total_passed > 0, "no conformance cases ran");
}

fn run_suite(
    suite_file: &str,
    path: &str,
    plain: (*mut Engine, *mut Session),
    templating: (*mut Engine, *mut Session),
    failures: &mut Vec<String>,
) -> SuiteOutcome {
    let contents =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let cases: Value =
        serde_json::from_str(&contents).unwrap_or_else(|e| panic!("failed to parse {path}: {e}"));
    let cases = cases.as_array().expect("suite file is a JSON array");

    let mut outcome = SuiteOutcome {
        passed: 0,
        failed: 0,
    };

    let fail = |failures: &mut Vec<String>,
                outcome: &mut SuiteOutcome,
                index: usize,
                description: &str,
                detail: String| {
        outcome.failed += 1;
        failures.push(format!("  {suite_file}[{index}] {description}: {detail}"));
    };

    for (index, case) in cases.iter().enumerate() {
        // String entries are section headers, not cases.
        if case.is_string() {
            continue;
        }
        let obj = case
            .as_object()
            .unwrap_or_else(|| panic!("{suite_file}[{index}] is neither string nor object"));

        let description = obj
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("No description");
        let rule = obj
            .get("rule")
            .unwrap_or_else(|| panic!("{suite_file}[{index}] missing 'rule'"));
        let data = obj.get("data").cloned().unwrap_or(json!({}));
        let use_templating = obj
            .get("templating")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let expects_error = obj.contains_key("error");
        let expected_result = obj.get("result");
        if !expects_error && expected_result.is_none() {
            panic!("{suite_file}[{index}] missing 'result' or 'error'");
        }

        let (engine, session) = if use_templating { templating } else { plain };
        let rule_str = rule.to_string();
        let data_str = data.to_string();

        let outcomes = [
            ("apply", unsafe { eval_apply(engine, &rule_str, &data_str) }),
            ("session_data", unsafe {
                eval_session_data(engine, session, &rule_str, &data_str)
            }),
        ];

        for (path_name, got) in outcomes {
            if expects_error {
                match got {
                    Err(msg) if !msg.is_empty() => outcome.passed += 1,
                    Err(_) => fail(
                        failures,
                        &mut outcome,
                        index,
                        description,
                        format!("[{path_name}] error status with empty message"),
                    ),
                    Ok(got) => fail(
                        failures,
                        &mut outcome,
                        index,
                        description,
                        format!(
                            "[{path_name}] expected error {:?}, got result {got}",
                            obj.get("error")
                        ),
                    ),
                }
                continue;
            }

            let expected = expected_result.expect("checked above");
            match got {
                Err(msg) => fail(
                    failures,
                    &mut outcome,
                    index,
                    description,
                    format!("[{path_name}] expected {expected}, got error: {msg}"),
                ),
                Ok(got_str) => match serde_json::from_str::<Value>(&got_str) {
                    Ok(got) if &got == expected => outcome.passed += 1,
                    Ok(got) => fail(
                        failures,
                        &mut outcome,
                        index,
                        description,
                        format!("[{path_name}] expected {expected}, got {got}"),
                    ),
                    Err(e) => fail(
                        failures,
                        &mut outcome,
                        index,
                        description,
                        format!("[{path_name}] result is not valid JSON ({e}): {got_str}"),
                    ),
                },
            }
        }
    }

    outcome
}
