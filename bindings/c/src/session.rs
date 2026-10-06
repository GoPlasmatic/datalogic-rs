//! Hot-loop `Session` — one arena + one result buffer, reused across
//! evaluations. Every borrowed-result entry point lives here.
//!
//! ## The borrowed-result contract
//!
//! `datalogic_session_evaluate*` return `(ptr, len)` into a
//! session-owned buffer. The bytes stay valid **until the next call
//! that touches the same session** (any evaluate, `reset`, or `free`).
//! Wrappers copy into a managed string immediately; that copy replaces
//! the malloc + `free`-crossing round trip of the v1 contract.
//!
//! ## Re-entry
//!
//! A custom-operator callback runs inside the evaluation that called it.
//! If it calls back into the session running that evaluation, the
//! session's arena and buffer are still in use. Every entry point takes
//! the session through [`Session::enter`], which refuses a session that is
//! already in use (`DATALOGIC_STATUS_INVALID_ARG`) instead of handing out
//! a second mutable borrow of it.

use std::cell::{Cell, UnsafeCell};
use std::sync::Arc;

use datalogic_bind::{ItemError, typed};
use datalogic_rs::Engine as RsEngine;
use datalogic_rs::bumpalo::Bump;
use datalogic_rs::datavalue::DataValue;

use crate::data::Data;
use crate::error::{Error, Status, fail};
use crate::rule::Rule;
use crate::{Slice, guard_status, str_from_raw};

/// Single-threaded session (`struct datalogic_session`): one reusable
/// `bumpalo::Bump` for evaluation scratch and one reusable `Vec<u8>`
/// for serialized results. **Not thread-safe** — open one per thread.
///
/// Holds `Arc<Engine>` so the underlying engine outlives the session
/// even if the consumer frees the engine handle first.
pub struct Session {
    engine: Arc<RsEngine>,
    /// Set while an entry point is using `state`; see [`Session::enter`].
    busy: Cell<bool>,
    /// Reached only through a [`Busy`] guard, so at most one entry point
    /// holds it mutably, even when a custom operator calls back in.
    state: UnsafeCell<State>,
}

/// The parts of a [`Session`] an evaluation writes.
struct State {
    arena: Bump,
    result_buf: Vec<u8>,
}

/// The refusal a session gives a call made while it is already in use.
const REENTERED: &str = "session is already evaluating: a custom operator called back into the \
                         session running it (open a second session for nested evaluations)";

impl Session {
    pub(crate) fn new(engine: Arc<RsEngine>) -> Self {
        Self {
            engine,
            busy: Cell::new(false),
            state: UnsafeCell::new(State {
                arena: Bump::new(),
                result_buf: Vec::new(),
            }),
        }
    }

    /// Take the session for one entry point, or refuse it when an entry
    /// point further up the stack (one whose custom operator called back
    /// in) holds it. The guard gives the session back when dropped,
    /// unwinding included.
    fn enter(&self) -> Result<Busy<'_>, Error> {
        if self.busy.replace(true) {
            return Err(Error::invalid_arg(REENTERED));
        }
        Ok(Busy { session: self })
    }
}

/// One entry point's hold on a [`Session`], from [`Session::enter`].
struct Busy<'s> {
    session: &'s Session,
}

impl<'s> Busy<'s> {
    fn engine(&self) -> &'s Arc<RsEngine> {
        &self.session.engine
    }

    fn state(&mut self) -> &mut State {
        // SAFETY: `busy` is set for as long as this guard lives, so no
        // other guard (and so no other reference to `state`) exists, and
        // the `&mut self` borrow keeps this one unique.
        unsafe { &mut *self.session.state.get() }
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.session.busy.set(false);
    }
}

/// Shared head of every session entry point: deref the handles, verify
/// the rule belongs to the session's engine, and take the session.
unsafe fn check_pair<'s>(
    session: *mut Session,
    rule: *const Rule,
) -> Result<(Busy<'s>, &'s Rule), Error> {
    // A shared borrow: a re-entrant call must not create a second `&mut`.
    let session =
        unsafe { session.as_ref() }.ok_or_else(|| Error::invalid_arg("session pointer is null"))?;
    let rule =
        unsafe { rule.as_ref() }.ok_or_else(|| Error::invalid_arg("rule pointer is null"))?;
    datalogic_bind::same_engine(&session.engine, &rule.engine).map_err(Error::invalid_arg)?;
    Ok((session.enter()?, rule))
}

/// Release a session handle. Safe to call with `NULL`.
///
/// Called from a custom-operator callback on the session that is running
/// it, this does nothing: the session is still in use, and freeing it
/// there would leave the running evaluation reading freed memory.
///
/// # Safety
///
/// `session` must either be `NULL` or a pointer previously returned by
/// [`crate::datalogic_engine_session`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_free(session: *mut Session) {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return;
    };
    if s.busy.get() {
        return;
    }
    drop(unsafe { Box::from_raw(session) });
}

/// Reset the session's arena and invalidate any borrowed result.
/// Optional — every evaluate call already resets at the start. Exposed
/// for consumers who want to release memory between long pauses.
///
/// Called from a custom-operator callback on the session that is running
/// it, this does nothing: the running evaluation is still using the arena.
///
/// # Safety
///
/// `session` must be a valid pointer or `NULL` (no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_reset(session: *mut Session) {
    if let Some(s) = unsafe { session.as_ref() }
        && let Ok(mut busy) = s.enter()
    {
        let state = busy.state();
        state.arena.reset();
        state.result_buf.clear();
    }
}

/// Bytes currently held by the session's evaluation arena (sum across
/// all chunks; excludes the result buffer). Returns `0` for `NULL`, and
/// from a custom-operator callback on the session that is running it
/// (the arena is in use).
///
/// # Safety
///
/// `session` must be a valid pointer or `NULL`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_allocated_bytes(session: *const Session) -> usize {
    match unsafe { session.as_ref() }.map(Session::enter) {
        Some(Ok(mut busy)) => busy.state().arena.allocated_bytes(),
        _ => 0,
    }
}

/// Evaluate `rule` against `(data_json, data_len)` and expose the JSON
/// result as borrowed bytes: on success `*out_ptr`/`*out_len` point
/// into the session's buffer, valid until the next call touching this
/// session. On failure the out-params are left untouched.
///
/// # Safety
///
/// `session`/`rule` must be valid handles from the same engine, used
/// from one thread; `data_json` must reference `data_len` readable
/// bytes; `out_ptr`/`out_len` must be writable; `err` follows the
/// crate-wide error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate(
    session: *mut Session,
    rule: *const Rule,
    data_json: *const u8,
    data_len: usize,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let (mut busy, rule) = match unsafe { check_pair(session, rule) } {
            Ok(pair) => pair,
            Err(e) => return unsafe { fail(err, e) },
        };
        if out_ptr.is_null() || out_len.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out_ptr/out_len pointer is null")) };
        }
        let data = match unsafe { str_from_raw("data_json", data_json, data_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };

        // Reset BEFORE evaluating — the previous call's borrowed result
        // dies here, exactly as the contract states.
        let engine = busy.engine();
        let State { arena, result_buf } = busy.state();
        arena.reset();
        result_buf.clear();
        match engine.evaluate(&rule.logic, data, &*arena) {
            Ok(av) => {
                av.write_json_into(result_buf);
                unsafe {
                    *out_ptr = result_buf.as_ptr();
                    *out_len = result_buf.len();
                }
                Status::Ok
            }
            Err(e) => unsafe { fail(err, Error::from_engine(&e, Some(&rule.logic))) },
        }
    })
}

/// [`datalogic_session_evaluate`] under an operation budget, reporting
/// what the evaluation cost in `*out_ops`. `budget` 0 means the engine's
/// configured `ops_budget`, or unbounded when it has none. Crossing the
/// budget fails with tag `"BudgetExceeded"`. The result follows the same
/// borrow rule as `datalogic_session_evaluate`.
///
/// # Safety
///
/// As [`datalogic_session_evaluate`]; `out_ops` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate_metered(
    session: *mut Session,
    rule: *const Rule,
    data_json: *const u8,
    data_len: usize,
    budget: u64,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
    out_ops: *mut u64,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let (mut busy, rule) = match unsafe { check_pair(session, rule) } {
            Ok(pair) => pair,
            Err(e) => return unsafe { fail(err, e) },
        };
        if out_ptr.is_null() || out_len.is_null() || out_ops.is_null() {
            return unsafe {
                fail(
                    err,
                    Error::invalid_arg("out_ptr/out_len/out_ops pointer is null"),
                )
            };
        }
        let data = match unsafe { str_from_raw("data_json", data_json, data_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        let engine = busy.engine();
        let State { arena, result_buf } = busy.state();
        arena.reset();
        result_buf.clear();
        let budget = engine.resolve_ops_budget((budget != 0).then_some(budget));
        match engine.evaluate_metered(&rule.logic, data, &*arena, budget) {
            Ok(metered) => {
                metered.value.write_json_into(result_buf);
                unsafe {
                    *out_ptr = result_buf.as_ptr();
                    *out_len = result_buf.len();
                    *out_ops = metered.ops;
                }
                Status::Ok
            }
            Err(e) => unsafe { fail(err, Error::from_engine(&e, Some(&rule.logic))) },
        }
    })
}

/// Same as [`datalogic_session_evaluate`] with a parsed-data handle
/// instead of JSON text — the hot path: zero parse work per call.
///
/// # Safety
///
/// Same as [`datalogic_session_evaluate`]; `data` must be a valid
/// handle from [`crate::datalogic_data_parse`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate_data(
    session: *mut Session,
    rule: *const Rule,
    data: *const Data,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let (mut busy, rule) = match unsafe { check_pair(session, rule) } {
            Ok(pair) => pair,
            Err(e) => return unsafe { fail(err, e) },
        };
        if out_ptr.is_null() || out_len.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out_ptr/out_len pointer is null")) };
        }
        let Some(data) = (unsafe { data.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("data pointer is null")) };
        };

        let engine = busy.engine();
        let State { arena, result_buf } = busy.state();
        arena.reset();
        result_buf.clear();
        match engine.evaluate(&rule.logic, &data.parsed, &*arena) {
            Ok(av) => {
                av.write_json_into(result_buf);
                unsafe {
                    *out_ptr = result_buf.as_ptr();
                    *out_len = result_buf.len();
                }
                Status::Ok
            }
            Err(e) => unsafe { fail(err, Error::from_engine(&e, Some(&rule.logic))) },
        }
    })
}

// =============== typed scalar results ===============
//
// Handle-input only (design decision D8): the predicate-heavy flows
// that want typed results are exactly the flows that parse data once.
// These paths never touch the result buffer — no serialization at all.

/// Evaluate and read the result as a strict JSON boolean into `*out`
/// (0/1). Returns `DATALOGIC_STATUS_TYPE_MISMATCH` if the result is any
/// other type; for JSONLogic truthiness coercion use
/// [`datalogic_session_evaluate_truthy`].
///
/// # Safety
///
/// Same handle/thread contract as [`datalogic_session_evaluate_data`];
/// `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate_bool(
    session: *mut Session,
    rule: *const Rule,
    data: *const Data,
    out: *mut i32,
    err: *mut *mut Error,
) -> Status {
    unsafe {
        typed_eval(session, rule, data, out, err, |av, _| {
            typed::bool(av).map(i32::from).map_err(Error::type_mismatch)
        })
    }
}

/// Evaluate and read the result as an integer into `*out`. Returns
/// `DATALOGIC_STATUS_TYPE_MISMATCH` when the result is not an exact
/// integer number.
///
/// # Safety
///
/// Same contract as [`datalogic_session_evaluate_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate_i64(
    session: *mut Session,
    rule: *const Rule,
    data: *const Data,
    out: *mut i64,
    err: *mut *mut Error,
) -> Status {
    unsafe {
        typed_eval(session, rule, data, out, err, |av, _| {
            typed::int(av).map_err(Error::type_mismatch)
        })
    }
}

/// Evaluate and read the result as a double into `*out`. Accepts any
/// JSON number; returns `DATALOGIC_STATUS_TYPE_MISMATCH` otherwise.
///
/// # Safety
///
/// Same contract as [`datalogic_session_evaluate_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate_f64(
    session: *mut Session,
    rule: *const Rule,
    data: *const Data,
    out: *mut f64,
    err: *mut *mut Error,
) -> Status {
    unsafe {
        typed_eval(session, rule, data, out, err, |av, _| {
            typed::float(av).map_err(Error::type_mismatch)
        })
    }
}

/// Evaluate and collapse the result to 0/1 via the engine's configured
/// truthiness rules (the same coercion `if`/`and`/`or` apply). Never
/// type-mismatches — any result truthy-converts.
///
/// # Safety
///
/// Same contract as [`datalogic_session_evaluate_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate_truthy(
    session: *mut Session,
    rule: *const Rule,
    data: *const Data,
    out: *mut i32,
    err: *mut *mut Error,
) -> Status {
    unsafe {
        typed_eval(session, rule, data, out, err, |av, engine| {
            Ok(engine.truthy(av) as i32)
        })
    }
}

/// Shared body of the four typed entry points: evaluate over the
/// session arena, project through `extract`, write `*out` on success.
unsafe fn typed_eval<T>(
    session: *mut Session,
    rule: *const Rule,
    data: *const Data,
    out: *mut T,
    err: *mut *mut Error,
    extract: impl FnOnce(&DataValue<'_>, &RsEngine) -> Result<T, Error>,
) -> Status {
    guard_status(err, || {
        let (mut busy, rule) = match unsafe { check_pair(session, rule) } {
            Ok(pair) => pair,
            Err(e) => return unsafe { fail(err, e) },
        };
        if out.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out pointer is null")) };
        }
        let Some(data) = (unsafe { data.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("data pointer is null")) };
        };

        let engine = busy.engine();
        let arena = &mut busy.state().arena;
        arena.reset();
        match engine.evaluate(&rule.logic, &data.parsed, &*arena) {
            Ok(av) => match extract(av, engine) {
                Ok(value) => {
                    unsafe { *out = value };
                    Status::Ok
                }
                Err(e) => unsafe { fail(err, e) },
            },
            Err(e) => unsafe { fail(err, Error::from_engine(&e, Some(&rule.logic))) },
        }
    })
}

// =============== batch ===============

/// One rule × `n` data handles. `out_results`/`out_statuses` are
/// caller-allocated arrays of length `n`.
///
/// Per item `i`: `out_statuses[i]` carries the item's status and
/// `out_results[i]` points at either the result JSON or (on item
/// failure) a small error object `{"tag": ..., "message": ...}`. All
/// result slices borrow from the session buffer — same validity
/// contract as [`datalogic_session_evaluate`]. The call-level return
/// covers argument problems only; item failures never fail the call.
///
/// # Safety
///
/// `datas` must reference `n` readable handle pointers;
/// `out_results`/`out_statuses` must be writable for `n` entries
/// (`n == 0` short-circuits and allows NULL arrays). Same
/// session/thread contract as [`datalogic_session_evaluate`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate_batch(
    session: *mut Session,
    rule: *const Rule,
    datas: *const *const Data,
    n: usize,
    out_results: *mut Slice,
    out_statuses: *mut Status,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let (mut busy, rule) = match unsafe { check_pair(session, rule) } {
            Ok(pair) => pair,
            Err(e) => return unsafe { fail(err, e) },
        };
        if n == 0 {
            return Status::Ok;
        }
        if datas.is_null() || out_results.is_null() || out_statuses.is_null() {
            return unsafe {
                fail(
                    err,
                    Error::invalid_arg("datas/out_results/out_statuses pointer is null"),
                )
            };
        }
        let results = unsafe { std::slice::from_raw_parts_mut(out_results, n) };
        let statuses = unsafe { std::slice::from_raw_parts_mut(out_statuses, n) };

        let engine = busy.engine();
        let State { arena, result_buf } = busy.state();
        result_buf.clear();

        // First pass records (offset, len) spans — the buffer may
        // reallocate while growing, so pointers are materialised only
        // after the last write.
        let mut spans: Vec<(usize, usize)> = Vec::with_capacity(n);
        for (i, status_slot) in statuses.iter_mut().enumerate() {
            let start = result_buf.len();
            let status = match unsafe { (*datas.add(i)).as_ref() } {
                None => {
                    write_item(
                        &ItemError::invalid_argument("data handle is null"),
                        result_buf,
                    );
                    Status::InvalidArg
                }
                Some(data) => {
                    // Scratch from the previous item is dead — its
                    // result bytes already live in `result_buf`.
                    arena.reset();
                    match engine.evaluate(&rule.logic, &data.parsed, &*arena) {
                        Ok(av) => {
                            av.write_json_into(result_buf);
                            Status::Ok
                        }
                        Err(e) => {
                            write_item(&ItemError::from_engine(&e), result_buf);
                            Error::engine_status(&e)
                        }
                    }
                }
            };
            *status_slot = status;
            spans.push((start, result_buf.len() - start));
        }

        let base = result_buf.as_ptr();
        for (slot, (offset, len)) in results.iter_mut().zip(spans) {
            *slot = Slice {
                ptr: unsafe { base.add(offset) },
                len,
            };
        }
        Status::Ok
    })
}

/// `n` rules × one data handle — the rule-set / feature-flag shape.
/// Same per-item semantics, buffer borrowing, and array contracts as
/// [`datalogic_session_evaluate_batch`].
///
/// # Safety
///
/// `rules` must reference `n` readable handle pointers; everything else
/// as [`datalogic_session_evaluate_batch`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_session_evaluate_many(
    session: *mut Session,
    rules: *const *const Rule,
    n: usize,
    data: *const Data,
    out_results: *mut Slice,
    out_statuses: *mut Status,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        let Some(session) = (unsafe { session.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("session pointer is null")) };
        };
        let mut busy = match session.enter() {
            Ok(busy) => busy,
            Err(e) => return unsafe { fail(err, e) },
        };
        if n == 0 {
            return Status::Ok;
        }
        if rules.is_null() || out_results.is_null() || out_statuses.is_null() {
            return unsafe {
                fail(
                    err,
                    Error::invalid_arg("rules/out_results/out_statuses pointer is null"),
                )
            };
        }
        let Some(data) = (unsafe { data.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("data pointer is null")) };
        };
        let results = unsafe { std::slice::from_raw_parts_mut(out_results, n) };
        let statuses = unsafe { std::slice::from_raw_parts_mut(out_statuses, n) };

        let engine = busy.engine();
        let State { arena, result_buf } = busy.state();
        result_buf.clear();

        let mut spans: Vec<(usize, usize)> = Vec::with_capacity(n);
        for (i, status_slot) in statuses.iter_mut().enumerate() {
            let start = result_buf.len();
            let status = match unsafe { (*rules.add(i)).as_ref() } {
                None => {
                    write_item(
                        &ItemError::invalid_argument("rule handle is null"),
                        result_buf,
                    );
                    Status::InvalidArg
                }
                Some(rule) if !Arc::ptr_eq(engine, &rule.engine) => {
                    write_item(
                        &ItemError::invalid_argument(datalogic_bind::DIFFERENT_ENGINE),
                        result_buf,
                    );
                    Status::InvalidArg
                }
                Some(rule) => {
                    arena.reset();
                    match engine.evaluate(&rule.logic, &data.parsed, &*arena) {
                        Ok(av) => {
                            av.write_json_into(result_buf);
                            Status::Ok
                        }
                        Err(e) => {
                            write_item(&ItemError::from_engine(&e), result_buf);
                            Error::engine_status(&e)
                        }
                    }
                }
            };
            *status_slot = status;
            spans.push((start, result_buf.len() - start));
        }

        let base = result_buf.as_ptr();
        for (slot, (offset, len)) in results.iter_mut().zip(spans) {
            *slot = Slice {
                ptr: unsafe { base.add(offset) },
                len,
            };
        }
        Status::Ok
    })
}

/// A failed batch item, written into the shared result buffer so per-item
/// failures need no handles of their own: `{"message", "operator"?,
/// "tag"}`.
fn write_item(item: &ItemError, out: &mut Vec<u8>) {
    out.extend_from_slice(item.to_value().to_string().as_bytes());
}
