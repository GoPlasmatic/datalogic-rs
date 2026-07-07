//! Compiled-rule handle + session-less one-shot evaluation.
//!
//! The session-less paths run over a pooled thread-local arena, so a
//! consumer that never discovers `datalogic_session` still gets
//! session-grade allocation behaviour (one warm arena per thread)
//! without the "session is not thread-safe" trap.

use std::cell::RefCell;
use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::{Engine as RsEngine, Logic};

use crate::data::Data;
use crate::error::{Error, Status, fail};
use crate::{Buf, guard_status, str_from_raw};

/// Compiled JSONLogic rule (`struct datalogic_rule`). Send + Sync —
/// share one across threads and evaluate in parallel.
///
/// Holds an `Arc<Engine>` so the engine outlives every rule compiled
/// from it — consumers can free the engine handle before the rule and
/// the rule still works.
pub struct Rule {
    pub(crate) engine: Arc<RsEngine>,
    pub(crate) logic: Arc<Logic>,
}

thread_local! {
    /// One warm arena per thread for the session-less paths. `take` on
    /// entry / put back on exit keeps reentrancy safe: a custom
    /// operator that calls back into a `datalogic_*` entry point on the
    /// same thread simply builds a fresh arena for the nested call.
    static POOLED_ARENA: RefCell<Option<Bump>> = const { RefCell::new(None) };
}

/// Cap on the retained per-thread arena so one huge payload doesn't pin
/// its high-water mark for the thread's lifetime.
const MAX_POOLED_ARENA_BYTES: usize = 4 * 1024 * 1024;

/// Run `body` with a warm thread-local arena; reset and repool it after
/// (dropping it instead when it grew past the cap — `Bump::reset` keeps
/// the largest chunk, so `allocated_bytes` reads retained capacity).
pub(crate) fn with_pooled_arena<T>(body: impl FnOnce(&Bump) -> T) -> T {
    let arena = POOLED_ARENA
        .with(|cell| cell.borrow_mut().take())
        .unwrap_or_else(Bump::new);
    let result = body(&arena);
    let mut arena = arena;
    arena.reset();
    if arena.allocated_bytes() <= MAX_POOLED_ARENA_BYTES {
        POOLED_ARENA.with(|cell| *cell.borrow_mut() = Some(arena));
    }
    result
}

/// Release a rule handle. Safe to call with `NULL`.
///
/// # Safety
///
/// `rule` must either be `NULL` or a pointer previously stored by
/// [`crate::datalogic_engine_compile`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_rule_free(rule: *mut Rule) {
    if rule.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(rule) });
}

/// Evaluate a compiled rule against `(data_json, data_len)` and store
/// the owned JSON result in `*out` (release via
/// [`crate::datalogic_buf_free`]). Runs over the pooled thread-local
/// arena. For tight loops, prefer a session — it also skips the result
/// allocation.
///
/// # Safety
///
/// `rule` must be a valid handle; `data_json` must reference `data_len`
/// readable bytes; `out` must be writable; `err` follows the crate-wide
/// error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_rule_evaluate(
    rule: *const Rule,
    data_json: *const u8,
    data_len: usize,
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
        let data = match unsafe { str_from_raw("data_json", data_json, data_len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        with_pooled_arena(
            |arena| match rule.engine.evaluate(&rule.logic, data, arena) {
                Ok(av) => {
                    let mut v = Vec::new();
                    av.write_json_into(&mut v);
                    unsafe { *out = Buf::from_vec(v) };
                    Status::Ok
                }
                Err(e) => unsafe { fail(err, Error::from_engine(&e, Some(&rule.logic))) },
            },
        )
    })
}

/// Same as [`datalogic_rule_evaluate`] with a parsed-data handle
/// instead of JSON text.
///
/// # Safety
///
/// Same as [`datalogic_rule_evaluate`]; `data` must be a valid handle
/// from [`crate::datalogic_data_parse`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_rule_evaluate_data(
    rule: *const Rule,
    data: *const Data,
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
        let Some(data) = (unsafe { data.as_ref() }) else {
            return unsafe { fail(err, Error::invalid_arg("data pointer is null")) };
        };
        with_pooled_arena(
            |arena| match rule.engine.evaluate(&rule.logic, &data.parsed, arena) {
                Ok(av) => {
                    let mut v = Vec::new();
                    av.write_json_into(&mut v);
                    unsafe { *out = Buf::from_vec(v) };
                    Status::Ok
                }
                Err(e) => unsafe { fail(err, Error::from_engine(&e, Some(&rule.logic))) },
            },
        )
    })
}
