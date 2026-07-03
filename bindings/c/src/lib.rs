//! `datalogic-c` — C ABI (v2) for the `datalogic-rs` JSONLogic engine.
//!
//! See `include/datalogic.h` for the generated public surface and
//! `README.md` for the contract; the measurements and design rationale
//! behind v2 are recorded in `tools/benchmark/BINDINGS-OVERHEAD.md`.
//! Consumers in Go, JVM, .NET, PHP, and other C-FFI languages link
//! against the `cdylib`/`staticlib` produced by this crate.
//!
//! ## The v2 contract in one paragraph
//!
//! Byte inputs cross the boundary as `(ptr, len)` UTF-8 — no NUL
//! terminators anywhere. Every fallible entry point returns a
//! [`Status`] and takes a trailing `datalogic_error **err` out-param:
//! pass `NULL` to skip error capture, otherwise release whatever lands
//! there with [`datalogic_error_free`]. Hot results are **borrowed**:
//! session evaluations return a pointer into a session-owned buffer
//! that stays valid until the next call touching that session. One-shot
//! paths return an owned [`Buf`] released via [`datalogic_buf_free`].
//! There is no thread-local state anywhere in the binding.
//!
//! Wrappers must assert `datalogic_abi_version() ==
//! DATALOGIC_ABI_VERSION` at load time so a stale shared library fails
//! loudly at init instead of corrupting at call time.

mod builder;
mod data;
mod engine;
mod error;
mod rule;
mod session;
mod traced_session;

pub use builder::*;
pub use data::*;
pub use engine::*;
pub use error::*;
pub use rule::*;
pub use session::*;
pub use traced_session::*;

use std::ffi::c_char;

/// ABI version stamp — bumped on any breaking change to the exported
/// surface (v1 was the NUL-terminated / thread-local-error contract;
/// v2 is the current `(ptr,len)` + status-code contract).
pub const DATALOGIC_ABI_VERSION: u32 = 2;

/// Runtime counterpart of [`DATALOGIC_ABI_VERSION`]. Wrappers call this
/// once at library load and refuse to run on a mismatch.
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_abi_version() -> u32 {
    DATALOGIC_ABI_VERSION
}

/// Return the binding's crate version as a static, NUL-terminated UTF-8
/// string (the one deliberate NUL-terminated survivor — it's a literal
/// for `printf`-style consumption).
///
/// The returned pointer is valid for the program's lifetime — never
/// free it.
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_version() -> *const c_char {
    // `concat!` embeds the NUL terminator at compile time, so the
    // returned pointer is into static memory and never needs freeing.
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

// =============== byte-range types ===============

/// Owned byte buffer returned by the one-shot entry points
/// ([`datalogic_engine_apply`], [`datalogic_rule_evaluate`],
/// [`datalogic_traced_session_evaluate`], …). Release via
/// [`datalogic_buf_free`]. The bytes are UTF-8 JSON, not NUL-terminated.
#[repr(C)]
pub struct Buf {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

impl Buf {
    /// Hand a `Vec`'s allocation across the boundary. Reclaimed by
    /// [`datalogic_buf_free`] via `Vec::from_raw_parts`.
    pub(crate) fn from_vec(v: Vec<u8>) -> Self {
        let mut v = std::mem::ManuallyDrop::new(v);
        Self {
            ptr: v.as_mut_ptr(),
            len: v.len(),
            cap: v.capacity(),
        }
    }
}

/// Borrowed byte range (UTF-8, not NUL-terminated). Used for the batch
/// result arrays; validity follows the owning session's borrow rules.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Slice {
    pub ptr: *const u8,
    pub len: usize,
}

/// Release a [`Buf`] returned by a one-shot entry point. Safe to call
/// with a zeroed/NULL-ptr buf.
///
/// # Safety
///
/// `buf` must be exactly as returned by this library (same ptr/len/cap
/// triple), passed at most once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_buf_free(buf: Buf) {
    if buf.ptr.is_null() {
        return;
    }
    drop(unsafe { Vec::from_raw_parts(buf.ptr, buf.len, buf.cap) });
}

// =============== shared entry-point plumbing ===============

/// Borrow a caller `(ptr, len)` byte range as `&str`, or produce the
/// `InvalidArg` error naming the parameter. A NULL pointer with zero
/// length reads as the empty string (which then fails JSON parsing with
/// a proper `Parse` error rather than a pointer complaint).
///
/// # Safety
///
/// If `ptr` is non-NULL it must reference `len` readable bytes that
/// stay valid for the duration of the surrounding call.
pub(crate) unsafe fn str_from_raw<'a>(
    name: &str,
    ptr: *const u8,
    len: usize,
) -> Result<&'a str, Error> {
    if ptr.is_null() {
        if len == 0 {
            return Ok("");
        }
        return Err(Error::invalid_arg(format!(
            "{name} pointer is null (with non-zero length)"
        )));
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes)
        .map_err(|_| Error::invalid_arg(format!("{name} is not valid UTF-8")))
}

/// Run a handle-returning C ABI entry-point body, converting any panic
/// into `default` instead of unwinding across the `extern "C"` boundary
/// (which aborts the host process since Rust 1.81).
pub(crate) fn ffi_guard<T>(default: T, body: impl FnOnce() -> T) -> T {
    // `AssertUnwindSafe`: on panic we discard all captured state and
    // return a fixed default, so the usual `UnwindSafe` concerns don't
    // apply at this boundary.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(value) => value,
        Err(_) => default,
    }
}

/// Run a status-returning C ABI entry-point body, converting any panic
/// (engine bug, arena OOM, a panicking custom-operator callback) into
/// [`Status::Internal`] with the error handle stored for the caller.
pub(crate) fn guard_status(
    err_out: *mut *mut Error,
    body: impl FnOnce() -> Status,
) -> Status {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(status) => status,
        Err(_) => unsafe {
            error::fail(
                err_out,
                Error::internal(
                    "internal error: a panic was caught at the datalogic FFI boundary",
                ),
            )
        },
    }
}
