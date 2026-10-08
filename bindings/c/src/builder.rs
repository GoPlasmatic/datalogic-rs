//! Engine builder + custom operator FFI (v2 callback contract).
//!
//! The Rust core's [`datalogic_rs::EngineBuilder`] is consume-on-method
//! (each `with_*` returns the moved builder), so the C wrapper hides
//! ownership transfer behind an opaque handle whose mutating entry
//! points `take` and `replace` the inner builder in-place.
//!
//! ## Callback contract (v2)
//!
//! v1 callbacks returned a freshly `malloc`'d NUL-terminated string the
//! binding parsed and then `free`'d — one cross-allocator handoff and
//! one extra boundary crossing per operator invocation. v2 callbacks
//! receive a `datalogic_op_result *` and *write* their outcome through
//! [`datalogic_op_result_set_json`] / [`datalogic_op_result_set_error`]
//! (both copy immediately), then return `0` for success or non-zero for
//! failure. No allocator crosses the boundary in either direction.

use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CustomOperator, DataValue, Engine as RsEngine, Error as DlError, EvaluationConfig,
    Result as DlResult,
};

use crate::engine::Engine;
use crate::error::{Error, Status, fail};
use crate::{ffi_guard, guard_status, str_from_raw};

/// Outcome carrier handed to custom-operator callbacks (opaque
/// `struct datalogic_op_result`). Only valid for the duration of the
/// callback invocation — never store the pointer.
pub struct OpResult {
    json: Option<Vec<u8>>,
    error: Option<Vec<u8>>,
}

/// Set the operator's result as UTF-8 JSON (copied immediately; the
/// caller keeps ownership of `json`). Calling twice replaces the
/// earlier value. A success return (`0`) with no JSON set evaluates to
/// JSON `null`.
///
/// # Safety
///
/// `out` must be the pointer passed into the currently-running
/// callback; `json` must reference `len` readable bytes (a `NULL`
/// pointer reads as empty).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_op_result_set_json(
    out: *mut OpResult,
    json: *const u8,
    len: usize,
) {
    let Some(out) = (unsafe { out.as_mut() }) else {
        return;
    };
    let bytes = if json.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(json, len) }
    };
    out.json = Some(bytes.to_vec());
}

/// Set the operator's error message (copied immediately). Read only
/// when the callback returns non-zero; a non-zero return with no
/// message set produces a generic error naming the operator.
///
/// # Safety
///
/// Same contract as [`datalogic_op_result_set_json`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_op_result_set_error(
    out: *mut OpResult,
    msg: *const u8,
    len: usize,
) {
    let Some(out) = (unsafe { out.as_mut() }) else {
        return;
    };
    let bytes = if msg.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(msg, len) }
    };
    out.error = Some(bytes.to_vec());
}

/// Callback signature for user-defined operators.
///
/// * `args_json` / `args_len` — borrowed UTF-8 JSON-array of the
///   pre-evaluated arguments (e.g. `[1, 2, "x"]`). Not NUL-terminated;
///   valid only during the invocation.
/// * `user_data` — opaque pointer registered alongside the callback.
/// * `out` — write the outcome through the `datalogic_op_result_set_*`
///   functions.
///
/// Return `0` for success, non-zero for failure.
pub type DatalogicOpFn = Option<
    unsafe extern "C" fn(
        args_json: *const u8,
        args_len: usize,
        user_data: *mut c_void,
        out: *mut OpResult,
    ) -> i32,
>;

/// Opaque builder handle (`struct datalogic_engine_builder`).
/// Internally holds an `Option<EngineBuilder>` so each `set_*` /
/// `add_operator` entry point can take the inner by value (the Rust
/// builder consumes `self` on every method) and put it back.
pub struct EngineBuilder {
    inner: Option<datalogic_rs::EngineBuilder>,
    /// Refuse an operator named like a built-in (see
    /// [`datalogic_engine_builder_set_strict_operator_names`]).
    strict_names: bool,
}

/// The refusal a setter gives a builder [`datalogic_engine_builder_build`]
/// has already drained.
const ALREADY_BUILT: &str = "engine builder was already built";

impl EngineBuilder {
    /// Take the inner builder for a setter, or refuse a builder that
    /// [`datalogic_engine_builder_build`] has already drained.
    fn take(&mut self) -> Result<datalogic_rs::EngineBuilder, Error> {
        self.inner
            .take()
            .ok_or_else(|| Error::invalid_arg(ALREADY_BUILT))
    }

    /// With strict names on, refuse settings under which an operator
    /// already registered would no longer run: the families or template
    /// key escape a setter is about to give `next`.
    fn recheck(&self, next: &datalogic_rs::EngineBuilder) -> Result<(), Error> {
        if !self.strict_names {
            return Ok(());
        }
        next.check_operator_names()
            .map_err(|e| Error::from_engine(&e, None))
    }
}

/// Construct a new engine builder. Release the handle via
/// [`datalogic_engine_builder_free`] (still required after a successful
/// [`datalogic_engine_builder_build`], which only drains the inner
/// builder).
#[unsafe(no_mangle)]
pub extern "C" fn datalogic_engine_builder_new() -> *mut EngineBuilder {
    ffi_guard(std::ptr::null_mut(), || {
        Box::into_raw(Box::new(EngineBuilder {
            inner: Some(RsEngine::builder()),
            strict_names: false,
        }))
    })
}

/// Free an engine builder handle. Safe with `NULL`.
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

/// Toggle templating mode on the builder. No-op on a `NULL` or
/// already-built builder.
///
/// # Safety
///
/// `builder` must be `NULL` or a valid builder handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_set_templating(
    builder: *mut EngineBuilder,
    enabled: i32,
) {
    let Some(handle) = (unsafe { builder.as_mut() }) else {
        return;
    };
    if let Some(b) = handle.inner.take() {
        handle.inner = Some(b.with_templating(enabled != 0));
    }
}

/// Set the template-key escape: the character (`codepoint`, a Unicode
/// scalar value) that marks a template key as a literal output field. Only
/// meaningful with templating on. On a builder
/// [`datalogic_engine_builder_build`] has already drained, fails with
/// `DATALOGIC_STATUS_INVALID_ARG`.
///
/// # Safety
///
/// `builder` must be `NULL` or a valid builder handle; `err` follows the
/// crate-wide error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_set_template_key_escape(
    builder: *mut EngineBuilder,
    codepoint: u32,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(handle) = (unsafe { builder.as_mut() }) else {
            return unsafe { fail(err, Error::invalid_arg("engine builder pointer is null")) };
        };
        let Some(c) = char::from_u32(codepoint) else {
            return unsafe {
                fail(
                    err,
                    Error::invalid_arg("codepoint is not a Unicode scalar value"),
                )
            };
        };
        let b = match handle.take() {
            Ok(b) => b,
            Err(e) => return unsafe { fail(err, e) },
        };
        let next = b.clone().with_template_key_escape(c);
        if let Err(e) = handle.recheck(&next) {
            handle.inner = Some(b);
            return unsafe { fail(err, e) };
        }
        handle.inner = Some(next);
        Status::Ok
    })
}

/// When `enabled != 0`, a later [`datalogic_engine_builder_add_operator`]
/// with a name a built-in answers to (`length`, `var`, an alias such as
/// `?:`) or beginning with the template key escape fails with tag
/// `ConfigurationError` instead of registering an operator that would
/// never run. A later [`datalogic_engine_builder_set_families`] or
/// [`datalogic_engine_builder_set_template_key_escape`] under which an
/// operator registered so would no longer run fails the same way and
/// leaves the builder unchanged. Set it before adding operators.
///
/// # Safety
///
/// `builder` must be `NULL` or a valid builder handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_set_strict_operator_names(
    builder: *mut EngineBuilder,
    enabled: i32,
) {
    if let Some(handle) = unsafe { builder.as_mut() } {
        handle.strict_names = enabled != 0;
    }
}

/// Keep the engine to the JSONLogic core and the operator families named in
/// `(families_json, families_len)`, a JSON array of family names
/// (`["ExtString", "DateTime"]`, as the operator catalogue's `family`
/// field spells them). By default the engine has every family this build
/// compiled in. A family left out is not there for the engine: its names
/// compile as unknown operators (or template output fields), and a custom
/// operator may take them. With strict operator names on, families that
/// bring back a built-in named like an operator already registered fail
/// with tag `"ConfigurationError"`, as an unknown family name does; either
/// leaves the builder unchanged. On a builder
/// [`datalogic_engine_builder_build`] has already drained, fails with
/// `DATALOGIC_STATUS_INVALID_ARG`.
///
/// # Safety
///
/// `builder` must be a valid builder handle; `families_json` must
/// reference `families_len` readable bytes; `err` follows the crate-wide
/// error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_set_families(
    builder: *mut EngineBuilder,
    families_json: *const u8,
    families_len: usize,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(handle) = (unsafe { builder.as_mut() }) else {
            return unsafe { fail(err, Error::invalid_arg("engine builder pointer is null")) };
        };
        let json = match unsafe { str_from_raw("families_json", families_json, families_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        let families = match datalogic_bind::families_from_json(json) {
            Ok(f) => f,
            Err(msg) => {
                return unsafe {
                    fail(
                        err,
                        Error::from_engine(&datalogic_rs::Error::configuration_error(msg), None),
                    )
                };
            }
        };
        let b = match handle.take() {
            Ok(b) => b,
            Err(e) => return unsafe { fail(err, e) },
        };
        let next = b.clone().with_families(families);
        if let Err(e) = handle.recheck(&next) {
            handle.inner = Some(b);
            return unsafe { fail(err, e) };
        }
        handle.inner = Some(next);
        Status::Ok
    })
}

/// Set the engine's evaluation configuration from a JSON object.
///
/// Parsed by the core crate's shared config parser
/// ([`EvaluationConfig::from_json_str`]) — the same wire format every
/// binding uses (`preset`, `arithmetic_nan_handling`,
/// `division_by_zero`, `loose_equality_errors`, `missing_var`,
/// `truthy_evaluator`, `numeric_coercion`, `max_recursion_depth`,
/// `ops_budget`). Unknown keys and enum strings are rejected (tag
/// `"ConfigurationError"`) so typos fail loudly. Each call replaces the
/// builder's entire evaluation config; templating and registered operators
/// are unaffected. A failed call leaves the builder usable. On a builder
/// [`datalogic_engine_builder_build`] has already drained, fails with
/// `DATALOGIC_STATUS_INVALID_ARG`.
///
/// `ops_budget` is how a caller through this ABI bounds the work a rule
/// may do: an integer ceiling on the operations one evaluation may
/// charge, or `null` for unbounded (the default). Crossing it fails the
/// evaluation with tag `"BudgetExceeded"` before the work is done, and a
/// `try` in the rule cannot recover from it. The budget is engine-wide;
/// `datalogic_session_evaluate_metered` reports what one evaluation
/// charged, so a host can enforce its own per-call limit on top.
///
/// # Safety
///
/// `builder` must be a valid builder handle; `config_json` must
/// reference `config_len` readable bytes; `err` follows the crate-wide
/// error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_set_config_json(
    builder: *mut EngineBuilder,
    config_json: *const u8,
    config_len: usize,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(handle) = (unsafe { builder.as_mut() }) else {
            return unsafe { fail(err, Error::invalid_arg("engine builder pointer is null")) };
        };
        let config_str = match unsafe { str_from_raw("config_json", config_json, config_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        let config = match EvaluationConfig::from_json_str(config_str) {
            Ok(c) => c,
            Err(e) => return unsafe { fail(err, Error::from_engine(&e, None)) },
        };
        let b = match handle.take() {
            Ok(b) => b,
            Err(e) => return unsafe { fail(err, e) },
        };
        handle.inner = Some(b.with_config(config));
        Status::Ok
    })
}

/// Register a custom operator. The callback runs on every match of the
/// operator name during evaluation — see [`DatalogicOpFn`] for the
/// contract. **Built-ins win**: registering a name that collides with a
/// built-in JSONLogic operator silently never dispatches. On a builder
/// [`datalogic_engine_builder_build`] has already drained, fails with
/// `DATALOGIC_STATUS_INVALID_ARG`.
///
/// A callback may evaluate through the same engine, but not through the
/// `datalogic_session` running the evaluation that called it: that session
/// is in use until the callback returns, so its `evaluate*` calls fail with
/// `DATALOGIC_STATUS_INVALID_ARG`, and `reset` and `free` on it do nothing.
/// Open a second session, or use the session-less `datalogic_rule_*` calls.
///
/// # Safety
///
/// `builder` must be a valid builder handle; `name` must reference
/// `name_len` readable bytes; `callback` must be a valid function
/// pointer (a `NULL` callback is rejected as `InvalidArg`); `user_data`
/// is opaque, passed back into every invocation, and must be
/// thread-safe if the engine is shared across threads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_add_operator(
    builder: *mut EngineBuilder,
    name: *const u8,
    name_len: usize,
    callback: DatalogicOpFn,
    user_data: *mut c_void,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(handle) = (unsafe { builder.as_mut() }) else {
            return unsafe { fail(err, Error::invalid_arg("engine builder pointer is null")) };
        };
        let Some(callback) = callback else {
            return unsafe { fail(err, Error::invalid_arg("operator callback is null")) };
        };
        let name_str = match unsafe { str_from_raw("name", name, name_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        let name_owned = name_str.to_string();
        let b = match handle.take() {
            Ok(b) => b,
            Err(e) => return unsafe { fail(err, e) },
        };
        let op = CCustomOperator {
            name: name_owned.clone(),
            callback,
            user_data: AtomicPtr::new(user_data),
        };
        if handle.strict_names {
            if let Err(e) = b.check_operator_name(&name_owned) {
                handle.inner = Some(b);
                return unsafe { fail(err, Error::from_engine(&e, None)) };
            }
            // Checked above, so this registers; through `try_add_operator`
            // so later setters check the name again.
            handle.inner = b.try_add_operator(name_owned, op).ok();
        } else {
            handle.inner = Some(b.add_operator(name_owned, op));
        }
        Status::Ok
    })
}

/// Finalise the builder into an [`Engine`]. The builder handle is left
/// drained — [`datalogic_engine_builder_free`] still releases the outer
/// allocation. Returns `NULL` if the builder is `NULL` or already
/// built.
///
/// # Safety
///
/// `builder` must be `NULL` or a valid builder handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_builder_build(
    builder: *mut EngineBuilder,
) -> *mut Engine {
    ffi_guard(std::ptr::null_mut(), || {
        let Some(handle) = (unsafe { builder.as_mut() }) else {
            return std::ptr::null_mut();
        };
        let Some(b) = handle.inner.take() else {
            return std::ptr::null_mut();
        };
        Box::into_raw(Box::new(Engine {
            inner: std::sync::Arc::new(b.build()),
        }))
    })
}

// =============== custom-operator adapter ===============

struct CCustomOperator {
    name: String,
    callback: unsafe extern "C" fn(
        args_json: *const u8,
        args_len: usize,
        user_data: *mut c_void,
        out: *mut OpResult,
    ) -> i32,
    user_data: AtomicPtr<c_void>,
}

// SAFETY: the operator is shared with the engine's `Arc<Engine>` and
// may be invoked from any thread. `user_data` is an opaque pointer the
// user promises to make thread-safe themselves — the binding never
// dereferences it. The callback function pointer is `extern "C" fn`,
// which is Send + Sync by virtue of being a code address.
unsafe impl Send for CCustomOperator {}
unsafe impl Sync for CCustomOperator {}

impl CustomOperator for CCustomOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> DlResult<&'a DataValue<'a>> {
        // 1. Serialize args as one JSON array, straight into bytes.
        let mut args_json: Vec<u8> = Vec::with_capacity(64);
        datalogic_bind::write_args_json(args, &mut args_json);

        // 2. Invoke the C callback.
        let user_data = self.user_data.load(Ordering::Relaxed);
        let mut out = OpResult {
            json: None,
            error: None,
        };
        let rc =
            unsafe { (self.callback)(args_json.as_ptr(), args_json.len(), user_data, &mut out) };

        if rc != 0 {
            let msg = match &out.error {
                Some(bytes) if !bytes.is_empty() => format!(
                    "custom operator '{}': {}",
                    self.name,
                    String::from_utf8_lossy(bytes)
                ),
                _ => format!("custom operator '{}' failed (rc={rc})", self.name),
            };
            return Err(DlError::custom_message(msg));
        }

        // 3. Parse the result into the evaluation arena. No JSON set on
        //    a success return means JSON null.
        let result_bytes = out.json.as_deref().unwrap_or(b"null");
        let result_str = std::str::from_utf8(result_bytes).map_err(|_| {
            DlError::custom_message(format!(
                "custom operator '{}' returned invalid UTF-8",
                self.name
            ))
        })?;
        datalogic_bind::parse_result(&self.name, result_str, arena)
    }
}
