//! Thread-local last-error state.
//!
//! Every fallible C ABI entry point calls [`clear_error_state`] on entry
//! and, on failure, populates this block via [`set_error`] or
//! [`set_error_message`]. C consumers query it via
//! [`datalogic_last_error_message`] / `_type` / `_operator` / `_path_json`.
//!
//! The contract for the returned pointers is the same as POSIX `errno`:
//! valid until the next call on the current thread that mutates this state.

use std::cell::RefCell;
use std::ffi::{CString, c_char};

use datalogic_rs::{Error, Logic};

struct LastError {
    message: CString,
    error_type: CString,
    operator: Option<CString>,
    path_json: Option<CString>,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<LastError>> = const { RefCell::new(None) };
}

/// Stash a `datalogic_rs::Error` into thread-local state.
///
/// Passing `compiled` lets us resolve the error's node-id breadcrumb into
/// a JSON-serialised path (matching the Python binding's `.path`). When
/// the binding doesn't yet have a compiled `Logic` available (e.g. a
/// rule-parse failure), pass `None` and `path_json` stays empty.
pub(crate) fn set_error(err: &Error, compiled: Option<&Logic>) {
    let message = to_cstring_or_empty(err.to_string());
    let error_type = to_cstring_or_empty(err.tag().to_string());
    let operator = err.operator().and_then(|s| CString::new(s).ok());
    let path_json = compiled.and_then(|c| serialise_path(err, c));

    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(LastError {
            message,
            error_type,
            operator,
            path_json,
        });
    });
}

/// Stash a synthetic error originating in the binding itself (e.g. a
/// NULL pointer or invalid UTF-8 input). `error_type` should be one of
/// the engine's stable tags ("ParseError", "InternalError", …) so C
/// consumers can match on it the same way they would for engine errors.
pub(crate) fn set_error_message(message: impl Into<String>, error_type: &'static str) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(LastError {
            message: to_cstring_or_empty(message.into()),
            error_type: to_cstring_or_empty(error_type.to_string()),
            operator: None,
            path_json: None,
        });
    });
}

pub(crate) fn clear_error_state() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

fn to_cstring_or_empty(s: String) -> CString {
    // Strip any interior NUL bytes so CString::new never fails — these
    // shouldn't occur in engine error messages, but being defensive
    // here keeps the FFI surface infallible.
    let cleaned: String = s.chars().filter(|c| *c != '\0').collect();
    CString::new(cleaned).unwrap_or_else(|_| CString::new("").expect("empty"))
}

fn serialise_path(err: &Error, compiled: &Logic) -> Option<CString> {
    let steps = err.resolve_path(compiled);
    let arr: Vec<serde_json::Value> = steps
        .iter()
        .map(|s| {
            serde_json::json!({
                "node_id": s.node_id,
                "operator": s.operator,
                "arg_index": s.arg_index,
                "json_pointer": s.json_pointer,
            })
        })
        .collect();
    serde_json::to_string(&arr)
        .ok()
        .and_then(|j| CString::new(j).ok())
}

// === Exported C ABI ===

/// Reset thread-local last-error state. Safe to call when no error is set.
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_last_error_clear() {
    clear_error_state();
}

/// Return the last error's human-readable message, or `NULL` if no error.
///
/// The returned pointer is owned by the library; do **not** free. It is
/// valid only until the next call on this thread that mutates the
/// last-error block (any fallible `datalogic_*` call, plus
/// [`datalogic_last_error_clear`]).
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_last_error_message() -> *const c_char {
    LAST_ERROR.with(|cell| match &*cell.borrow() {
        Some(e) => e.message.as_ptr(),
        None => std::ptr::null(),
    })
}

/// Return the last error's stable type tag (e.g. `"ParseError"`,
/// `"Thrown"`, `"NaN"`, `"InternalError"`), or `NULL` if no error.
///
/// Same lifetime caveat as [`datalogic_last_error_message`].
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_last_error_type() -> *const c_char {
    LAST_ERROR.with(|cell| match &*cell.borrow() {
        Some(e) => e.error_type.as_ptr(),
        None => std::ptr::null(),
    })
}

/// Return the outermost failing operator's name (e.g. `"+"`, `"var"`),
/// or `NULL` if the last error didn't originate inside a named operator
/// (or if no error is set).
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_last_error_operator() -> *const c_char {
    LAST_ERROR.with(|cell| match &*cell.borrow() {
        Some(e) => e
            .operator
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null()),
        None => std::ptr::null(),
    })
}

/// Return the resolved root-to-leaf error path as a JSON array string
/// (matching the Python binding's `.path` attribute), or `NULL` if not
/// available. Available when the failing call had a compiled `Rule` in
/// scope at the time of failure.
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_last_error_path_json() -> *const c_char {
    LAST_ERROR.with(|cell| match &*cell.borrow() {
        Some(e) => e
            .path_json
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null()),
        None => std::ptr::null(),
    })
}
