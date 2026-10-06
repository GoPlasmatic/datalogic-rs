//! ABI v2, minor 1 (5.8): per-compile modes, checking, introspection and
//! truthiness. Every result that is a document is JSON in an owned
//! [`Buf`], in the formats `datalogic-bind` defines for every binding.

use datalogic_rs::CheckMode;

use crate::engine::{Engine, compile_into, engine_ref};
use crate::error::{Error, Status, fail};
use crate::rule::Rule;
use crate::{Buf, guard_status, put_buf, str_from_raw};

/// How [`datalogic_engine_compile_mode`] and [`datalogic_engine_check`]
/// read a rule. Passed as a `uint32_t`.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DatalogicMode {
    /// The engine's own mode, as `datalogic_engine_compile` reads it.
    Engine = 0,
    /// Outside templating mode: a multi-key object or an unknown operator
    /// is an error.
    Strict = 1,
    /// In templating mode: a multi-key object is an output template and
    /// an unknown key an output field.
    Template = 2,
}

pub(crate) fn mode_from(raw: u32) -> Result<CheckMode, Error> {
    match raw {
        0 => Ok(CheckMode::Engine),
        1 => Ok(CheckMode::Strict),
        2 => Ok(CheckMode::Template),
        _ => Err(Error::invalid_arg(
            "mode must be 0 (engine), 1 (strict) or 2 (template)",
        )),
    }
}

/// [`crate::datalogic_engine_compile`] in an explicit [`DatalogicMode`]
/// (`mode` 0, 1 or 2), whatever mode the engine was built with.
///
/// # Safety
///
/// As [`crate::datalogic_engine_compile`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_compile_mode(
    engine: *const Engine,
    rule_json: *const u8,
    rule_len: usize,
    mode: u32,
    out_rule: *mut *mut Rule,
    err: *mut *mut Error,
) -> Status {
    unsafe {
        compile_into(engine, rule_json, rule_len, out_rule, err, |engine, src| {
            let mode = mode_from(mode)?;
            datalogic_bind::compile_in(engine, src, mode).map_err(|e| Error::from_engine(&e, None))
        })
    }
}

/// Compile a rule, refusing it if [`datalogic_engine_check`] finds any
/// error. A refusal returns `DATALOGIC_STATUS_PARSE` with tag
/// `"CompileError"`; read every diagnostic with
/// [`crate::datalogic_error_diagnostics_json`].
///
/// # Safety
///
/// As [`crate::datalogic_engine_compile`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_compile_checked(
    engine: *const Engine,
    rule_json: *const u8,
    rule_len: usize,
    out_rule: *mut *mut Rule,
    err: *mut *mut Error,
) -> Status {
    unsafe {
        compile_into(engine, rule_json, rule_len, out_rule, err, |engine, src| {
            engine
                .compile_checked(src)
                .map_err(|e| Error::from_compile(&e))
        })
    }
}

/// Every problem the engine can see in a rule before it runs, as a JSON
/// array of `{code, severity, message, pointer, operator}` in `*out`
/// (release via [`crate::datalogic_buf_free`]). Finding problems is not a
/// failure: the call returns `DATALOGIC_STATUS_OK` with them in the array.
///
/// # Safety
///
/// `engine` must be a valid handle; the rule bytes must reference
/// `rule_len` readable bytes; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_check(
    engine: *const Engine,
    rule_json: *const u8,
    rule_len: usize,
    mode: u32,
    out: *mut Buf,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let engine = match unsafe { engine_ref(engine) } {
            Ok(e) => e,
            Err(e) => return unsafe { fail(err, e) },
        };
        if out.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out pointer is null")) };
        }
        let mode = match mode_from(mode) {
            Ok(m) => m,
            Err(e) => return unsafe { fail(err, e) },
        };
        let src = match unsafe { str_from_raw("rule_json", rule_json, rule_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        let diagnostics = engine.inner.check(src, mode);
        unsafe { put_buf(out, datalogic_bind::diagnostics_json(&diagnostics)) }
    })
}

/// Every built-in operator the engine evaluates, as a JSON array (the
/// schema of the docs' `operators.json`) in `*out`.
///
/// # Safety
///
/// `engine` must be a valid handle; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_operators(
    engine: *const Engine,
    out: *mut Buf,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let engine = match unsafe { engine_ref(engine) } {
            Ok(e) => e,
            Err(e) => return unsafe { fail(err, e) },
        };
        if out.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out pointer is null")) };
        }
        unsafe { put_buf(out, datalogic_bind::operators_json(&engine.inner)) }
    })
}

/// Whether a JSON value is truthy under the engine's configured
/// truthiness; `*out` is 1 or 0.
///
/// # Safety
///
/// `engine` must be a valid handle; the value bytes must reference
/// `value_len` readable bytes; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_engine_truthy(
    engine: *const Engine,
    value_json: *const u8,
    value_len: usize,
    out: *mut i32,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let engine = match unsafe { engine_ref(engine) } {
            Ok(e) => e,
            Err(e) => return unsafe { fail(err, e) },
        };
        if out.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out pointer is null")) };
        }
        let src = match unsafe { str_from_raw("value_json", value_json, value_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        match datalogic_rs::ParsedData::from_json(src) {
            Ok(parsed) => {
                unsafe { *out = i32::from(engine.inner.truthy_of(&parsed)) };
                Status::Ok
            }
            Err(e) => unsafe { fail(err, Error::from_engine(&e, None)) },
        }
    })
}

/// What a compiled rule reads and calls, as JSON in `*out`:
/// `{reads, computed_reads, reads_complete, reads_data, operators,
/// custom_operators, deterministic}`, with each read path as its segments.
///
/// # Safety
///
/// `rule` must be a valid handle; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_rule_facts(
    rule: *const Rule,
    out: *mut Buf,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(rule) = (unsafe { rule.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("rule pointer is null")) };
        };
        if out.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out pointer is null")) };
        }
        unsafe { put_buf(out, datalogic_bind::facts_json(&rule.logic.facts())) }
    })
}
