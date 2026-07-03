//! Status codes + owned error handles (ABI v2).
//!
//! v1 kept a thread-local "last error" block that every entry point
//! cleared on entry; that cost a TLS access per call and forced Go to
//! pin the OS thread around every evaluation just to read the error
//! afterwards. v2 deletes it: fallible entry points return a [`Status`]
//! and, when the caller passes a non-NULL `datalogic_error **`, store a
//! freshly-allocated error handle the caller releases with
//! [`datalogic_error_free`]. Passing `NULL` skips capture entirely, so
//! error-reporting cost sits wholly on the error path.

use datalogic_rs::{Error as DlError, Logic};

/// Coarse, branchable outcome of a fallible call.
///
/// The fine-grained engine tag (e.g. `"Thrown"`, `"ArithmeticError"`,
/// `"ConfigurationError"`) stays available via [`datalogic_error_tag`],
/// so wrappers branch on the status and surface the tag.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Success.
    Ok = 0,
    /// A NULL handle, a NULL byte pointer with non-zero length, invalid
    /// UTF-8 input, or a mismatched handle (e.g. a rule compiled by a
    /// different engine than the session's).
    InvalidArg = 1,
    /// Rule / data / config JSON failed to parse.
    Parse = 2,
    /// Evaluation failed; the error tag carries the detail
    /// (`"Thrown"`, `"ArithmeticError"`, `"TypeError"`, …).
    Eval = 3,
    /// A typed-result call evaluated successfully but the result is not
    /// of the requested type.
    TypeMismatch = 4,
    /// A panic was caught at the FFI boundary (engine bug or a
    /// panicking custom-operator callback).
    Internal = 5,
}

/// Owned error detail behind `datalogic_error *`. Allocated only when
/// the caller asked for capture; released via [`datalogic_error_free`].
pub struct Error {
    status: Status,
    message: String,
    tag: String,
    operator: Option<String>,
    path_json: Option<String>,
}

impl Error {
    /// Wrap an engine error. Passing `compiled` lets us resolve the
    /// error's node-id breadcrumb into a JSON-serialised path (matching
    /// the Python binding's `.path`); pass `None` when no compiled
    /// `Logic` is in scope (e.g. a rule-parse failure).
    pub(crate) fn from_engine(err: &DlError, compiled: Option<&Logic>) -> Self {
        let tag = err.tag().to_string();
        let status = match tag.as_str() {
            "ParseError" => Status::Parse,
            "InternalError" => Status::Internal,
            _ => Status::Eval,
        };
        Self {
            status,
            message: err.to_string(),
            tag,
            operator: err.operator().map(str::to_owned),
            path_json: compiled.and_then(|c| serialize_path(err, c)),
        }
    }

    pub(crate) fn invalid_arg(message: impl Into<String>) -> Self {
        Self {
            status: Status::InvalidArg,
            message: message.into(),
            tag: "InvalidArgument".to_string(),
            operator: None,
            path_json: None,
        }
    }

    pub(crate) fn type_mismatch(message: impl Into<String>) -> Self {
        Self {
            status: Status::TypeMismatch,
            message: message.into(),
            tag: "TypeMismatch".to_string(),
            operator: None,
            path_json: None,
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: Status::Internal,
            message: message.into(),
            tag: "InternalError".to_string(),
            operator: None,
            path_json: None,
        }
    }

    pub(crate) fn status(&self) -> Status {
        self.status
    }

    /// Item-level error rendering for the batch entry points: a small
    /// JSON object written into the shared result buffer so per-item
    /// failures don't need their own handles.
    pub(crate) fn write_item_json_into(&self, out: &mut Vec<u8>) {
        let obj = match &self.operator {
            Some(op) => serde_json::json!({
                "tag": self.tag,
                "message": self.message,
                "operator": op,
            }),
            None => serde_json::json!({
                "tag": self.tag,
                "message": self.message,
            }),
        };
        // `to_string` on a json! literal cannot fail.
        out.extend_from_slice(obj.to_string().as_bytes());
    }
}

fn serialize_path(err: &DlError, compiled: &Logic) -> Option<String> {
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
    serde_json::to_string(&arr).ok()
}

/// Store `err` into the caller's out-param (if provided) and return its
/// status. The single exit path every fallible entry point funnels
/// failures through.
///
/// # Safety
///
/// `err_out` must be `NULL` or a valid, writable `datalogic_error *`
/// slot. On non-NULL, any previous value is overwritten without being
/// freed — callers own initialising the slot (conventionally to `NULL`)
/// and releasing whatever lands in it.
pub(crate) unsafe fn fail(err_out: *mut *mut Error, err: Error) -> Status {
    let status = err.status;
    if !err_out.is_null() {
        unsafe { *err_out = Box::into_raw(Box::new(err)) };
    }
    status
}

// === Exported C ABI ===

/// Release an error handle produced by any `datalogic_*` call. Safe to
/// call with `NULL`.
///
/// # Safety
///
/// `err` must either be `NULL` or a pointer stored into a
/// `datalogic_error **` out-param by this library that has not been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_error_free(err: *mut Error) {
    if err.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(err) });
}

/// The error's [`Status`] (same value the failing call returned).
/// Returns [`Status::Internal`] for a `NULL` handle.
///
/// # Safety
///
/// `err` must be `NULL` or a valid error handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_error_status(err: *const Error) -> Status {
    match unsafe { err.as_ref() } {
        Some(e) => e.status,
        None => Status::Internal,
    }
}

/// Write `s` through the (pointer, `*len_out`) accessor convention:
/// borrowed UTF-8 bytes, not NUL-terminated, `NULL`/0 when absent.
unsafe fn str_out(s: Option<&str>, len_out: *mut usize) -> *const u8 {
    let (ptr, len) = match s {
        Some(s) => (s.as_ptr(), s.len()),
        None => (std::ptr::null(), 0),
    };
    if !len_out.is_null() {
        unsafe { *len_out = len };
    }
    ptr
}

/// The error's human-readable message. Borrowed from the handle (valid
/// until [`datalogic_error_free`]); not NUL-terminated — use `*len_out`.
///
/// # Safety
///
/// `err` must be `NULL` (returns `NULL`) or a valid error handle;
/// `len_out` must be `NULL` or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_error_message(err: *const Error, len_out: *mut usize) -> *const u8 {
    unsafe { str_out(err.as_ref().map(|e| e.message.as_str()), len_out) }
}

/// The error's stable type tag — the engine's `Error::tag()` values
/// (`"ParseError"`, `"Thrown"`, `"ArithmeticError"`, `"TypeError"`,
/// `"ConfigurationError"`, …) plus this binding's own
/// `"InvalidArgument"` / `"TypeMismatch"` / `"InternalError"`. Same
/// lifetime and encoding contract as [`datalogic_error_message`].
///
/// # Safety
///
/// `err` must be `NULL` (returns `NULL`) or a valid error handle;
/// `len_out` must be `NULL` or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_error_tag(err: *const Error, len_out: *mut usize) -> *const u8 {
    unsafe { str_out(err.as_ref().map(|e| e.tag.as_str()), len_out) }
}

/// The outermost failing operator's name (e.g. `"+"`, `"var"`), or
/// `NULL` if the error didn't originate inside a named operator.
///
/// # Safety
///
/// `err` must be `NULL` (returns `NULL`) or a valid error handle;
/// `len_out` must be `NULL` or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_error_operator(
    err: *const Error,
    len_out: *mut usize,
) -> *const u8 {
    unsafe { str_out(err.as_ref().and_then(|e| e.operator.as_deref()), len_out) }
}

/// The resolved root-to-leaf error path as a JSON array string
/// (matching the Python binding's `.path`), or `NULL` when the failing
/// call had no compiled rule in scope.
///
/// # Safety
///
/// `err` must be `NULL` (returns `NULL`) or a valid error handle;
/// `len_out` must be `NULL` or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_error_path_json(
    err: *const Error,
    len_out: *mut usize,
) -> *const u8 {
    unsafe { str_out(err.as_ref().and_then(|e| e.path_json.as_deref()), len_out) }
}
