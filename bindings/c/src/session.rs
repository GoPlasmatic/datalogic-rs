//! Hot-loop `Session` — owns one arena, reused across evaluations.

use std::ffi::c_char;
use std::sync::Arc;

use datalogic_rs::Engine as RsEngine;
use datalogic_rs::bumpalo::Bump;

use crate::cstr_to_str;
use crate::error::{clear_error_state, set_error, set_error_message};
use crate::rule::Rule;
use crate::string_to_cstring;

/// Single-threaded session reusing one `bumpalo::Bump` across
/// evaluations. **Not `Sync`** — a session must only ever be used from
/// the thread that created it (the C ABI does not enforce this; consumer
/// languages should).
///
/// Holds `Arc<Engine>` so the underlying engine outlives the session
/// even if the consumer frees the engine handle first.
pub struct Session {
    engine: Arc<RsEngine>,
    arena: Bump,
}

impl Session {
    pub(crate) fn new(engine: Arc<RsEngine>) -> Self {
        Self {
            engine,
            arena: Bump::new(),
        }
    }
}

/// Release a session handle. Safe to call with `NULL`.
///
/// # Safety
///
/// `session` must either be `NULL` or a pointer previously returned by
/// [`crate::datalogic_engine_session`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_free(session: *mut Session) {
    if session.is_null() {
        return;
    }
    // SAFETY: caller contract.
    drop(unsafe { Box::from_raw(session) });
}

/// Evaluate `rule` against `data_json` using the session's reusable
/// arena. The arena is reset at the start of every call so peak memory
/// stays bounded; the previous call's result string has already been
/// materialised by the time we reset.
///
/// Returns a freshly-allocated JSON string the caller must release via
/// [`crate::datalogic_string_free`]. Returns `NULL` on failure.
///
/// # Safety
///
/// `session` and `rule` must be valid pointers; `data_json` must be a
/// valid NUL-terminated UTF-8 string. Must be called from the same
/// thread that created the session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate(
    session: *mut Session,
    rule: *mut Rule,
    data_json: *const c_char,
) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        clear_error_state();
        let session = match unsafe { session.as_mut() } {
            Some(s) => s,
            None => {
                set_error_message("session pointer is null", "ParseError");
                return std::ptr::null_mut();
            }
        };
        let rule = match unsafe { rule.as_ref() } {
            Some(r) => r,
            None => {
                set_error_message("rule pointer is null", "ParseError");
                return std::ptr::null_mut();
            }
        };
        let data_json = match cstr_to_str(data_json) {
            Some(s) => s,
            None => {
                set_error_message("data_json is null or not valid UTF-8", "ParseError");
                return std::ptr::null_mut();
            }
        };

        // Reset BEFORE evaluating — the previous call's owned result string
        // has already been materialised, so dropping the arena's allocations
        // is safe. Matches the Python binding's Session semantics.
        session.arena.reset();
        match session
            .engine
            .evaluate(&rule.logic, data_json, &session.arena)
        {
            Ok(av) => string_to_cstring(av.to_string()),
            Err(e) => {
                set_error(&e, Some(&rule.logic));
                std::ptr::null_mut()
            }
        }
    })
}

/// Manually reset the session's arena. Optional — every
/// [`datalogic_session_evaluate`] already resets at the start of the call.
/// Exposed mainly for consumers who want to release memory between
/// long pauses between evaluations.
///
/// # Safety
///
/// `session` must be a valid pointer or `NULL` (no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_reset(session: *mut Session) {
    if let Some(s) = unsafe { session.as_mut() } {
        s.arena.reset();
    }
}

/// Number of bytes currently held by the session's arena (sum across
/// all chunks). Useful for sizing or diagnostics. Returns `0` if
/// `session` is `NULL`.
///
/// # Safety
///
/// `session` must be a valid pointer or `NULL`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_allocated_bytes(session: *mut Session) -> usize {
    match unsafe { session.as_ref() } {
        Some(s) => s.arena.allocated_bytes(),
        None => 0,
    }
}
