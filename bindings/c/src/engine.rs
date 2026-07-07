//! `Engine` handle and the entry points that produce/consume it.

use std::sync::Arc;

use datalogic_rs::Engine as RsEngine;

use crate::error::{Error, Status, fail};
use crate::rule::{Rule, with_pooled_arena};
use crate::session::Session;
use crate::{Buf, ffi_guard, guard_status, str_from_raw};

/// Opaque handle wrapping `Arc<datalogic_rs::Engine>`
/// (`struct datalogic_engine`). Send + Sync — share across threads
/// freely.
pub struct Engine {
    pub(crate) inner: Arc<RsEngine>,
}

/// Construct a new engine. Pass `templating != 0` to enable templating
/// mode (multi-key objects in compiled rules become output-shaping
/// templates).
///
/// Returns an owned handle released via [`datalogic_engine_free`];
/// `NULL` only if construction panics internally.
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_engine_new(templating: i32) -> *mut Engine {
    ffi_guard(std::ptr::null_mut(), || {
        let engine = if templating != 0 {
            RsEngine::builder().with_templating(true).build()
        } else {
            RsEngine::new()
        };
        Box::into_raw(Box::new(Engine {
            inner: Arc::new(engine),
        }))
    })
}

/// Release an engine handle. Safe to call with `NULL`. Rules, sessions,
/// and traced sessions hold their own reference — freeing the engine
/// first is fine.
///
/// # Safety
///
/// `engine` must either be `NULL` or a pointer previously returned by
/// [`datalogic_engine_new`] / [`crate::datalogic_engine_builder_build`]
/// that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_free(engine: *mut Engine) {
    if engine.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(engine) });
}

/// Compile `(rule_json, rule_len)` into a reusable rule handle stored
/// in `*out_rule` (release via [`crate::datalogic_rule_free`]).
///
/// # Safety
///
/// `engine` must be a valid handle; `rule_json` must reference
/// `rule_len` readable bytes; `out_rule` must be writable; `err`
/// follows the crate-wide error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_compile(
    engine: *const Engine,
    rule_json: *const u8,
    rule_len: usize,
    out_rule: *mut *mut Rule,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(engine) = (unsafe { engine.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("engine pointer is null")) };
        };
        if out_rule.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out_rule pointer is null")) };
        }
        let rule_src = match unsafe { str_from_raw("rule_json", rule_json, rule_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        match engine.inner.compile_arc(rule_src) {
            Ok(logic) => {
                unsafe {
                    *out_rule = Box::into_raw(Box::new(Rule {
                        engine: engine.inner.clone(),
                        logic,
                    }));
                }
                Status::Ok
            }
            Err(e) => unsafe { fail(err, Error::from_engine(&e, None)) },
        }
    })
}

/// One-shot: compile `(rule_json)` and evaluate against `(data_json)`
/// in a single call, storing the owned JSON result in `*out` (release
/// via [`crate::datalogic_buf_free`]).
///
/// For repeated evaluations of one rule, prefer
/// [`datalogic_engine_compile`] + a session; for repeated evaluations
/// against one payload, add [`crate::datalogic_data_parse`].
///
/// # Safety
///
/// `engine` must be a valid handle; the byte inputs must reference
/// their stated lengths; `out` must be writable; `err` follows the
/// crate-wide error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_apply(
    engine: *const Engine,
    rule_json: *const u8,
    rule_len: usize,
    data_json: *const u8,
    data_len: usize,
    out: *mut Buf,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(engine) = (unsafe { engine.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("engine pointer is null")) };
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

        // Compile first so `&Logic` is available for error path resolution.
        let logic = match engine.inner.compile_arc(rule_src) {
            Ok(l) => l,
            Err(e) => return unsafe { fail(err, Error::from_engine(&e, None)) },
        };
        with_pooled_arena(|arena| match engine.inner.evaluate(&logic, data, arena) {
            Ok(av) => {
                let mut v = Vec::new();
                av.write_json_into(&mut v);
                unsafe { *out = Buf::from_vec(v) };
                Status::Ok
            }
            Err(e) => unsafe { fail(err, Error::from_engine(&e, Some(&logic))) },
        })
    })
}

/// Open a hot-loop [`Session`] bound to this engine. Sessions are
/// **not thread-safe** — open one per thread. Returns `NULL` for a
/// `NULL` engine.
///
/// # Safety
///
/// `engine` must be `NULL` or a valid engine handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_session(engine: *const Engine) -> *mut Session {
    ffi_guard(std::ptr::null_mut(), || match unsafe { engine.as_ref() } {
        Some(engine) => Box::into_raw(Box::new(Session::new(engine.inner.clone()))),
        None => std::ptr::null_mut(),
    })
}
