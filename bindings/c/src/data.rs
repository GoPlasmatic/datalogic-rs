//! Parse-once data handles — the structural fix for the boundary's
//! dominant cost.
//!
//! Parsing the data JSON is 70-90% of a parse-eval-serialize round trip
//! (see `tools/benchmark/BINDINGS-OVERHEAD.md`), and the v1 contract
//! forced every consumer to re-pay it on every call.
//! [`datalogic_data_parse`] pays it once; the handle then feeds any
//! rule/session/engine through the core's zero-cost `&DataValue`
//! passthrough. Rule-set workloads (many rules, one payload — the flagd
//! shape) and bulk scoring (one rule, many payloads, parsed up front)
//! are the target use cases.

use datalogic_rs::ParsedData;

use crate::error::{Error, Status, fail};
use crate::{guard_status, str_from_raw};

/// Immutable parsed JSON document (`struct datalogic_data`).
///
/// Independent of any engine — one handle can feed rules compiled by
/// different engines. Shareable across threads.
pub struct Data {
    pub(crate) parsed: ParsedData,
}

// SAFETY: `ParsedData` is `Send` but not `Sync` only because its
// backing `bumpalo::Bump` is `!Sync` (allocation takes `&self`). This
// handle never allocates after construction: the tree is built once in
// `datalogic_data_parse` and every subsequent access is a `&`-read of a
// `DataValue` tree with no interior mutability (verified against
// datavalue 0.2.2 — re-audit on datavalue upgrades). Concurrent reads
// of immutable memory are sound, so exposing the handle as thread-safe
// is correct as long as this crate never mutates through it (it
// doesn't — there is no mutating entry point for `Data`).
unsafe impl Send for Data {}
unsafe impl Sync for Data {}

/// Parse `(json, len)` into a resident data handle.
///
/// On success stores the new handle in `*out` and returns
/// `DATALOGIC_STATUS_OK`. The handle is immutable, thread-safe, and
/// engine-independent; release it with [`datalogic_data_free`] after
/// the last evaluation that uses it (handles are not consumed by
/// evaluation).
///
/// # Safety
///
/// `json` must reference `len` readable bytes for the duration of the
/// call (the handle copies what it needs — the caller's buffer may be
/// freed afterwards). `out` must be a valid, writable slot. `err`
/// follows the crate-wide error out-param contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_data_parse(
    json: *const u8,
    len: usize,
    out: *mut *mut Data,
    err: *mut *mut Error,
) -> Status {
    guard_status(err, || {
        if out.is_null() {
            return unsafe { fail(err, Error::invalid_arg("out pointer is null")) };
        }
        let s = match unsafe { str_from_raw("json", json, len) } {
            Ok(s) => s,
            Err(e) => return unsafe { fail(err, e) },
        };
        match ParsedData::from_json(s) {
            Ok(parsed) => {
                unsafe { *out = Box::into_raw(Box::new(Data { parsed })) };
                Status::Ok
            }
            Err(e) => unsafe { fail(err, Error::from_engine(&e, None)) },
        }
    })
}

/// Release a data handle. Safe to call with `NULL`.
///
/// # Safety
///
/// `data` must either be `NULL` or a pointer previously returned by
/// [`datalogic_data_parse`] that has not been freed, with no evaluation
/// concurrently reading it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_data_free(data: *mut Data) {
    if data.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(data) });
}

/// Bytes held by the handle's backing arena (input copy + tree).
/// Returns `0` for `NULL`.
///
/// # Safety
///
/// `data` must be `NULL` or a valid data handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_data_allocated_bytes(data: *const Data) -> usize {
    match unsafe { data.as_ref() } {
        Some(d) => d.parsed.allocated_bytes(),
        None => 0,
    }
}
