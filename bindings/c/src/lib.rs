//! `datalogic-c` — C ABI for the `datalogic-rs` JSONLogic engine.
//!
//! See `include/datalogic.h` for the public surface. Consumers in Go,
//! PHP, JVM, and other C-FFI languages link against the `cdylib`/`staticlib`
//! produced by this crate.
//!
//! The contract is JSON-in/JSON-out throughout: rules and data cross
//! the boundary as NUL-terminated UTF-8 strings, results are returned
//! as freshly-allocated owned strings the caller releases via
//! [`datalogic_string_free`]. Errors surface as `NULL` returns plus a
//! thread-local last-error block queryable via
//! [`datalogic_last_error_message`] et al.

mod builder;
mod engine;
mod error;
mod rule;
mod session;

pub use builder::*;
pub use engine::*;
pub use error::*;
pub use rule::*;
pub use session::*;

use std::ffi::{CStr, CString, c_char};

/// Return the binding's version as a static, NUL-terminated UTF-8 string.
///
/// The returned pointer is valid for the program's lifetime — never free it.
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_version() -> *const c_char {
    // `concat!` lets us embed the NUL terminator at compile time, so the
    // returned pointer is into static memory and never needs freeing.
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

/// Release a string previously returned by any `datalogic_*` function
/// that documents owned-string return semantics. Safe to call with `NULL`.
///
/// # Safety
///
/// `ptr` must either be `NULL` or a pointer previously returned by this
/// library's owned-string path (e.g. [`datalogic_engine_apply`],
/// [`datalogic_rule_evaluate`], [`datalogic_session_evaluate`]). Calling
/// with any other pointer (or twice on the same pointer) is undefined
/// behaviour.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // `CString::from_raw` reclaims the allocation we leaked in
    // `string_to_cstring`; dropping it frees the buffer.
    drop(unsafe { CString::from_raw(ptr) });
}

/// Borrow a `*const c_char` as a Rust `&str`. Returns `None` if the
/// pointer is `NULL` or the bytes are not valid UTF-8.
pub(crate) fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: caller contract — `ptr` is NUL-terminated and lives for the
    // duration of the call. The returned `&str` is constrained by `'a`
    // which the caller binds to the surrounding stack frame.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Convert an owned Rust `String` into an owned C string that the
/// caller must release via [`datalogic_string_free`].
///
/// If `s` contains interior NUL bytes (it shouldn't — JSON output never
/// does, but engine results round-tripped through user data theoretically
/// could) we truncate at the first NUL rather than failing the call.
pub(crate) fn string_to_cstring(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(e) => {
            let bytes = e.into_vec();
            let pos = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            let truncated = String::from_utf8_lossy(&bytes[..pos]).into_owned();
            CString::new(truncated)
                .unwrap_or_else(|_| CString::new("").expect("empty CString is always valid"))
                .into_raw()
        }
    }
}
