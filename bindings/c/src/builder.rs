//! Engine builder + custom operator FFI.
//!
//! The Rust core's [`datalogic_rs::EngineBuilder`] is consume-on-method
//! (each `with_*` returns the moved builder), so the C wrapper hides
//! ownership transfer behind an opaque handle whose mutating entry points
//! `take` and `replace` the inner builder in-place.

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::atomic::{AtomicPtr, Ordering};

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CustomOperator, DataValue, Engine as RsEngine, Error as DlError, Result as DlResult,
};

use crate::cstr_to_str;
use crate::engine::Engine;
use crate::error::{clear_error_state, set_error_message};

unsafe extern "C" {
    /// `free(3)` from libc. The Rust default allocator on `cdylib` targets
    /// is the system allocator (which is libc malloc on Linux/macOS/Windows),
    /// so freeing user-supplied `malloc`'d strings via libc `free` is safe
    /// across the FFI boundary.
    fn free(ptr: *mut c_void);
}

/// Callback signature for user-defined operators.
///
/// * `args_json` — borrowed, NUL-terminated UTF-8 JSON-array string of
///   pre-evaluated arguments (e.g. `"[1, 2, \"x\"]"`). Do not free.
/// * `user_data` — opaque pointer the caller registered via
///   [`datalogic_engine_builder_add_operator`].
/// * `error_out` — out-pointer. On error, set `*error_out` to a freshly
///   `malloc`'d, NUL-terminated UTF-8 message (the binding will call
///   `free` on it). May be left untouched / `NULL` for a generic error.
///
/// Returns:
/// - **success**: a freshly `malloc`'d, NUL-terminated UTF-8 JSON string
///   (e.g. `"42"`, `"\"variant_a\""`, `"{\"a\":1}"`). The binding parses
///   and then calls `free` on it.
/// - **error**: `NULL`, optionally with `*error_out` filled.
pub type DatalogicOpCallback = Option<
    unsafe extern "C" fn(
        args_json: *const c_char,
        user_data: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> *mut c_char,
>;

/// Opaque builder handle. Internally holds an `Option<EngineBuilder>` so
/// each `set_*` / `add_operator` entry point can take the inner by value
/// (the Rust builder consumes `self` on every method) and put it back.
pub struct EngineBuilder {
    inner: Option<datalogic_rs::EngineBuilder>,
}

/// Construct a new engine builder. Caller must release the handle via
/// [`datalogic_engine_builder_free`] (no-op after a successful
/// [`datalogic_engine_builder_build`], which consumes the builder).
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_engine_builder_new() -> *mut EngineBuilder {
    clear_error_state();
    Box::into_raw(Box::new(EngineBuilder {
        inner: Some(RsEngine::builder()),
    }))
}

/// Free an engine builder handle. Safe with `NULL`. Idempotent after a
/// successful `build()` (which already drops the inner builder; the
/// outer handle still needs freeing).
///
/// # Safety
///
/// `builder` must either be `NULL` or a pointer previously returned by
/// [`datalogic_engine_builder_new`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_free(builder: *mut EngineBuilder) {
    if builder.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(builder) });
}

/// Toggle templating mode on the builder.
///
/// # Safety
///
/// `builder` must be a valid pointer returned by
/// [`datalogic_engine_builder_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_set_templating(
    builder: *mut EngineBuilder,
    enabled: i32,
) {
    clear_error_state();
    let Some(handle) = (unsafe { builder.as_mut() }) else {
        set_error_message("engine builder pointer is null", "ParseError");
        return;
    };
    if let Some(b) = handle.inner.take() {
        handle.inner = Some(b.with_templating(enabled != 0));
    }
}

/// Register a custom operator. The callback is invoked on every match of
/// the operator name during evaluation. See [`DatalogicOpCallback`] for
/// the contract.
///
/// **Built-ins win** — registering a name that collides with a built-in
/// JSONLogic operator (`+`, `if`, `var`, …) silently does nothing at
/// evaluation time; the built-in dispatches first.
///
/// # Safety
///
/// `builder` must be valid; `name` must be NUL-terminated UTF-8;
/// `callback` must be a valid C function pointer (or `NULL` to make this
/// a no-op). `user_data` is opaque to the binding and passed back into
/// every invocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_add_operator(
    builder: *mut EngineBuilder,
    name: *const c_char,
    callback: DatalogicOpCallback,
    user_data: *mut c_void,
) -> i32 {
    clear_error_state();
    let Some(handle) = (unsafe { builder.as_mut() }) else {
        set_error_message("engine builder pointer is null", "ParseError");
        return -1;
    };
    let Some(callback) = callback else {
        // No-op: NULL callback. Could also error, but consistent with
        // the "permissive" feel of the other set_* entry points.
        return 0;
    };
    let Some(name_str) = cstr_to_str(name) else {
        set_error_message("operator name is null or not valid UTF-8", "ParseError");
        return -1;
    };
    let name_owned = name_str.to_string();
    if let Some(b) = handle.inner.take() {
        handle.inner = Some(b.add_operator(
            name_owned.clone(),
            CCustomOperator {
                name: name_owned,
                callback,
                user_data: AtomicPtr::new(user_data),
            },
        ));
    }
    0
}

/// Finalise the builder into an [`Engine`]. The builder handle is left
/// in a drained state — [`datalogic_engine_builder_free`] still needs to
/// be called to release the outer allocation.
///
/// Returns `NULL` if the builder has already been built / is invalid.
///
/// # Safety
///
/// `builder` must be a valid pointer returned by
/// [`datalogic_engine_builder_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_build(
    builder: *mut EngineBuilder,
) -> *mut Engine {
    clear_error_state();
    let Some(handle) = (unsafe { builder.as_mut() }) else {
        set_error_message("engine builder pointer is null", "ParseError");
        return std::ptr::null_mut();
    };
    let Some(b) = handle.inner.take() else {
        set_error_message("engine builder already built", "ParseError");
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(Engine {
        inner: std::sync::Arc::new(b.build()),
    }))
}

// =============== custom-operator adapter ===============

struct CCustomOperator {
    name: String,
    callback: unsafe extern "C" fn(
        args_json: *const c_char,
        user_data: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> *mut c_char,
    user_data: AtomicPtr<c_void>,
}

// SAFETY: the operator is shared with the engine's `Arc<Engine>` and may
// be invoked from any thread. `user_data` is an opaque pointer the user
// promises to make thread-safe themselves — the binding never dereferences
// it. The callback function pointer is `extern "C" fn`, which is Send +
// Sync by virtue of being a code address.
unsafe impl Send for CCustomOperator {}
unsafe impl Sync for CCustomOperator {}

impl CustomOperator for CCustomOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> DlResult<&'a DataValue<'a>> {
        // 1. Serialize args as a JSON array.
        let mut json = String::from("[");
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&a.to_json_string());
        }
        json.push(']');
        let c_json = CString::new(json).map_err(|e| {
            DlError::custom_message(format!(
                "custom operator '{}': args contained NUL byte: {}",
                self.name, e
            ))
        })?;

        // 2. Invoke the C callback.
        let user_data = self.user_data.load(Ordering::Relaxed);
        let mut error_ptr: *mut c_char = std::ptr::null_mut();
        let result_ptr =
            unsafe { (self.callback)(c_json.as_ptr(), user_data, &mut error_ptr as *mut _) };

        if result_ptr.is_null() {
            // Error path. Read out the error message (if any) and `free` it.
            let msg = if error_ptr.is_null() {
                format!("custom operator '{}' returned null", self.name)
            } else {
                let s = unsafe { CStr::from_ptr(error_ptr) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { free(error_ptr as *mut c_void) };
                format!("custom operator '{}': {}", self.name, s)
            };
            return Err(DlError::custom_message(msg));
        }

        // 3. Copy the result out and free the user's buffer.
        let result_str = unsafe { CStr::from_ptr(result_ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { free(result_ptr as *mut c_void) };

        // 4. Parse into the evaluation arena.
        let arena_str = arena.alloc_str(&result_str);
        let parsed = DataValue::from_str(arena_str, arena).map_err(|e| {
            DlError::custom_message(format!(
                "custom operator '{}' returned invalid JSON: {}",
                self.name, e
            ))
        })?;
        Ok(arena.alloc(parsed))
    }
}
