//! `Engine` handle and the entry points that produce/consume it.

use std::ffi::c_char;
use std::sync::Arc;

use datalogic_rs::Engine as RsEngine;
use datalogic_rs::bumpalo::Bump;

use crate::error::{clear_error_state, set_error, set_error_message};
use crate::rule::Rule;
use crate::session::Session;
use crate::{cstr_to_str, string_to_cstring};

/// Opaque handle wrapping `Arc<datalogic_rs::Engine>`. Send + Sync — share
/// across threads freely. C consumers see this as `struct datalogic_engine`
/// (forward-declared in the generated header).
pub struct Engine {
    pub(crate) inner: Arc<RsEngine>,
}

/// Construct a new engine. Pass `templating != 0` to enable the engine's
/// templating mode (multi-key objects in compiled rules become
/// output-shaping templates).
///
/// Returns an owned handle the caller must release via
/// [`datalogic_engine_free`]. Never returns `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_engine_new(templating: i32) -> *mut Engine {
    clear_error_state();
    let engine = if templating != 0 {
        RsEngine::builder().with_templating(true).build()
    } else {
        RsEngine::new()
    };
    Box::into_raw(Box::new(Engine {
        inner: Arc::new(engine),
    }))
}

/// Release an engine handle. Safe to call with `NULL`.
///
/// # Safety
///
/// `engine` must either be `NULL` or a pointer previously returned by
/// [`datalogic_engine_new`] that has not been freed. Calling with any
/// other pointer (or twice on the same pointer) is undefined behaviour.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_free(engine: *mut Engine) {
    if engine.is_null() {
        return;
    }
    // SAFETY: caller contract — pointer originated from `Box::into_raw`
    // in `datalogic_engine_new` and is not aliased.
    drop(unsafe { Box::from_raw(engine) });
}

/// Compile a JSONLogic rule (`rule_json`, NUL-terminated UTF-8) into a
/// reusable [`Rule`] handle. Returns `NULL` on parse failure — query
/// [`crate::datalogic_last_error_message`] for details.
///
/// # Safety
///
/// `engine` must be a valid pointer returned by [`datalogic_engine_new`];
/// `rule_json` must be a valid NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_compile(
    engine: *mut Engine,
    rule_json: *const c_char,
) -> *mut Rule {
    clear_error_state();
    let engine = match unsafe { engine.as_ref() } {
        Some(e) => e,
        None => {
            set_error_message("engine pointer is null", "ParseError");
            return std::ptr::null_mut();
        }
    };
    let rule_json = match cstr_to_str(rule_json) {
        Some(s) => s,
        None => {
            set_error_message("rule_json is null or not valid UTF-8", "ParseError");
            return std::ptr::null_mut();
        }
    };

    match engine.inner.compile_arc(rule_json) {
        Ok(logic) => Box::into_raw(Box::new(Rule {
            engine: engine.inner.clone(),
            logic,
        })),
        Err(e) => {
            set_error(&e, None);
            std::ptr::null_mut()
        }
    }
}

/// One-shot: compile `rule_json` and evaluate against `data_json` in a
/// single call. Returns the result as a freshly-allocated JSON string
/// the caller must release via [`crate::datalogic_string_free`]. Returns
/// `NULL` on failure — query the last-error state for details.
///
/// For repeated evaluations of the same rule, prefer
/// [`datalogic_engine_compile`] + [`crate::datalogic_rule_evaluate`] to
/// avoid re-parsing on every call.
///
/// # Safety
///
/// `engine`, `rule_json`, and `data_json` must all be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_apply(
    engine: *mut Engine,
    rule_json: *const c_char,
    data_json: *const c_char,
) -> *mut c_char {
    clear_error_state();
    let engine = match unsafe { engine.as_ref() } {
        Some(e) => e,
        None => {
            set_error_message("engine pointer is null", "ParseError");
            return std::ptr::null_mut();
        }
    };
    let rule_json = match cstr_to_str(rule_json) {
        Some(s) => s,
        None => {
            set_error_message("rule_json is null or not valid UTF-8", "ParseError");
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

    // Compile first so we have `&Logic` available for error path resolution.
    let logic = match engine.inner.compile_arc(rule_json) {
        Ok(l) => l,
        Err(e) => {
            set_error(&e, None);
            return std::ptr::null_mut();
        }
    };
    let arena = Bump::new();
    match engine.inner.evaluate(&logic, data_json, &arena) {
        Ok(av) => string_to_cstring(av.to_string()),
        Err(e) => {
            set_error(&e, Some(&logic));
            std::ptr::null_mut()
        }
    }
}

/// Open a hot-loop [`Session`] bound to this engine. The session reuses
/// a single `bumpalo` arena across evaluations and resets it at the
/// start of each call to bound peak memory.
///
/// Sessions are **not thread-safe** — open one per thread.
///
/// # Safety
///
/// `engine` must be a valid pointer returned by [`datalogic_engine_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_session(engine: *mut Engine) -> *mut Session {
    clear_error_state();
    let engine = match unsafe { engine.as_ref() } {
        Some(e) => e,
        None => {
            set_error_message("engine pointer is null", "ParseError");
            return std::ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(Session::new(engine.inner.clone())))
}
