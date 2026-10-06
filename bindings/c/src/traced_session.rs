//! Traced evaluation surface — wraps [`datalogic_rs::TracedSession`].
//!
//! [`datalogic_traced_session_evaluate`] is a one-shot path: it
//! compiles `rule_json` internally with the optimizer disabled so every
//! operator in the rule surfaces as an execution step. The result is an
//! owned JSON-object buffer with shape `{"result": <value|null>,
//! "expression_tree": <node>, "steps": [...], "error": <message>?,
//! "structured_error": <Error>?}` — matching the WASM binding's wire
//! format so downstream consumers see one shape across every language.
//!
//! This is a debug tier: results are returned as owned [`Buf`]s (not
//! session-borrowed bytes), which keeps the handle free of mutable
//! state and therefore Send + Sync — the one evaluate surface that can
//! be shared across threads without an external lock.

use std::sync::Arc;

use datalogic_rs::Engine as RsEngine;

use crate::engine::Engine;
use crate::error::{Error, Status, fail};
use crate::{Buf, ffi_guard, guard_status, str_from_raw};

/// Trace-enabled handle over a [`datalogic_rs::Engine`]
/// (`struct datalogic_traced_session`). Constructed via
/// [`datalogic_engine_traced_session`]. Send + Sync — share freely.
pub struct TracedSession {
    engine: Arc<RsEngine>,
}

/// Open a [`TracedSession`] bound to this engine. Returns `NULL` for a
/// `NULL` engine.
///
/// # Safety
///
/// `engine` must be `NULL` or a valid pointer returned by
/// [`crate::datalogic_engine_new`] /
/// [`crate::datalogic_engine_builder_build`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_traced_session(
    engine: *const Engine,
) -> *mut TracedSession {
    ffi_guard(std::ptr::null_mut(), || match unsafe { engine.as_ref() } {
        Some(engine) => Box::into_raw(Box::new(TracedSession {
            engine: Arc::clone(&engine.inner),
        })),
        None => std::ptr::null_mut(),
    })
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

/// One-shot traced evaluation: compile `(rule_json, rule_len)` with the
/// optimizer disabled, evaluate against `(data_json, data_len)`, and
/// store the result + trace as an owned JSON buffer in `*out` (release
/// via [`crate::datalogic_buf_free`]).
///
/// The envelope is `{result, expression_tree, steps, error?,
/// structured_error?, pointers?}`; `pointers` maps each node id (a decimal
/// string key) to the RFC 6901 JSON Pointer into the rule of the value that
/// node was compiled from, and is absent when the rule did not compile.
///
/// Engine errors (parse / eval) surface **inside** the returned JSON's
/// `error` / `structured_error` fields with a `DATALOGIC_STATUS_OK`
/// return — a non-OK status is reserved for invalid arguments.
///
/// # Safety
///
/// `session` must be a valid handle; the byte inputs must reference
/// their stated lengths; `out` must be writable; `err` follows the
/// crate-wide error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_traced_session_evaluate(
    session: *const TracedSession,
    rule_json: *const u8,
    rule_len: usize,
    data_json: *const u8,
    data_len: usize,
    out: *mut Buf,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(session) = (unsafe { session.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("traced session pointer is null")) };
        };
        if out.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out pointer is null")) };
        }
        let rule_src = match unsafe { str_from_raw("rule_json", rule_json, rule_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        let data = match unsafe { str_from_raw("data_json", data_json, data_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        let json = datalogic_bind::traced_json(&session.engine, rule_src, data);
        unsafe { *out = Buf::from_vec(json.into_bytes()) };
        Status::Ok
    })
}
