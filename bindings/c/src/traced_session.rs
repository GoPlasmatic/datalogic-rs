//! Traced evaluation surface — wraps [`datalogic_rs::TracedSession`].
//!
//! [`datalogic_traced_session_evaluate`] is a one-shot path: it compiles
//! `rule_json` internally with the optimizer disabled so every operator in
//! the rule surfaces as an execution step. The result is returned as a
//! JSON-object string with shape
//! `{"result": <value|null>, "expression_tree": <node>, "steps": [...],
//! "error": <message>?, "structured_error": <Error>?}` — matching the
//! WASM binding's wire format so downstream consumers see one shape across
//! every language.
//!
//! No pre-compiled-rule variant is exposed: the trace surface only makes
//! sense when the rule is compiled with the optimizer off (otherwise
//! folded sub-expressions are missing from the step log), so the one-shot
//! `eval_str` path is the only useful one.
//!
//! Threading: the underlying [`datalogic_rs::TracedSession`] is a
//! short-lived borrowed view over the engine that this handle owns via
//! `Arc<Engine>`. The handle itself is `Send + Sync` — share freely.

use std::ffi::c_char;
use std::sync::Arc;

use datalogic_rs::Engine as RsEngine;

use crate::cstr_to_str;
use crate::engine::Engine;
use crate::error::{clear_error_state, set_error_message};
use crate::string_to_cstring;

/// Trace-enabled handle over a [`datalogic_rs::Engine`]. Constructed via
/// [`datalogic_engine_traced_session`].
///
/// Unlike [`crate::session::Session`], this handle carries no per-call
/// arena — `TracedSession` always allocates a fresh `bumpalo::Bump` per
/// run to keep the borrowed-result lifetime tied to the trace. The
/// handle exists for API symmetry (every binding gets `engine ->
/// traced_session -> evaluate`).
///
/// Holds `Arc<Engine>` so the underlying engine outlives the handle
/// even if the consumer frees the engine handle first.
pub struct TracedSession {
    engine: Arc<RsEngine>,
}

/// Open a [`TracedSession`] bound to this engine. Every `evaluate` call
/// returns a JSON object carrying the result alongside execution-step and
/// expression-tree metadata.
///
/// # Safety
///
/// `engine` must be a valid pointer returned by
/// [`crate::datalogic_engine_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_traced_session(
    engine: *mut Engine,
) -> *mut TracedSession {
    clear_error_state();
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        set_error_message("engine pointer is null", "ParseError");
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(TracedSession {
        engine: Arc::clone(&engine.inner),
    }))
}

/// Release a traced-session handle. Safe to call with `NULL`.
///
/// # Safety
///
/// `session` must either be `NULL` or a pointer previously returned by
/// [`datalogic_engine_traced_session`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_traced_session_free(session: *mut TracedSession) {
    if session.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(session) });
}

/// One-shot traced evaluation: compile `rule_json` internally with the
/// optimizer disabled, evaluate against `data_json`, and return the
/// result + trace as a JSON-object string. Engine errors (parse / eval)
/// surface inside the returned JSON's `error` / `structured_error`
/// fields, not as a `NULL` return — `NULL` is reserved for invalid input
/// pointers (null / non-UTF8).
///
/// Caller releases the returned string via
/// [`crate::datalogic_string_free`].
///
/// # Safety
///
/// `session` must be a valid pointer; `rule_json` and `data_json` must be
/// valid NUL-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_traced_session_evaluate(
    session: *mut TracedSession,
    rule_json: *const c_char,
    data_json: *const c_char,
) -> *mut c_char {
    clear_error_state();
    let Some(session) = (unsafe { session.as_ref() }) else {
        set_error_message("traced session pointer is null", "ParseError");
        return std::ptr::null_mut();
    };
    let Some(rule_json) = cstr_to_str(rule_json) else {
        set_error_message("rule_json is null or not valid UTF-8", "ParseError");
        return std::ptr::null_mut();
    };
    let Some(data_json) = cstr_to_str(data_json) else {
        set_error_message("data_json is null or not valid UTF-8", "ParseError");
        return std::ptr::null_mut();
    };
    let run = session.engine.trace().eval_str(rule_json, data_json);
    string_to_cstring(traced_run_to_json(&run))
}

/// Render a [`datalogic_rs::TracedRun`] into the cross-binding wire JSON
/// shape: `{result, expression_tree, steps, error?, structured_error?}`.
/// Mirrors the WASM binding's `traced_run_to_json` so consumers see one
/// shape across every language.
fn traced_run_to_json(run: &datalogic_rs::TracedRun<String>) -> String {
    use serde::Serialize;

    #[derive(Serialize)]
    struct Wire<'a> {
        result: serde_json::Value,
        expression_tree: &'a datalogic_rs::ExpressionNode,
        steps: &'a [datalogic_rs::ExecutionStep],
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_error: Option<&'a datalogic_rs::Error>,
    }

    let result_json: serde_json::Value;
    let mut error_msg: Option<String> = None;
    let mut error_struct: Option<&datalogic_rs::Error> = None;
    match &run.result {
        Ok(s) => {
            // The String is already JSON; parse it back to a Value when
            // possible so consumers don't double-decode. Fall back to a
            // JSON string for the rare case where the engine result
            // wasn't well-formed JSON.
            result_json = serde_json::from_str::<serde_json::Value>(s.as_str())
                .unwrap_or_else(|_| serde_json::Value::String(s.to_string()));
        }
        Err(e) => {
            result_json = serde_json::Value::Null;
            error_msg = Some(e.to_string());
            error_struct = Some(e);
        }
    }
    serde_json::to_string(&Wire {
        result: result_json,
        expression_tree: &run.expression_tree,
        steps: &run.steps,
        error: error_msg,
        structured_error: error_struct,
    })
    .unwrap_or_default()
}
