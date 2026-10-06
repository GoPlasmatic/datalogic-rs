//! `DataHandle` napi class — parse-once data documents (ABI v2 mirror).
//!
//! Parsing the data JSON dominates a string-shaped evaluation
//! (70-90% of a parse-eval-serialize round trip, see
//! `tools/benchmark/BINDINGS-OVERHEAD.md`), and every string entry point
//! re-pays it per call. A `DataHandle` pays it once: the JSON is parsed
//! into a self-contained [`datalogic_rs::ParsedData`] tree that every
//! evaluation then consumes through the core's zero-cost `&DataValue`
//! passthrough.
//!
//! ## Thread affinity
//!
//! `ParsedData` is `Send` but **not** `Sync` (its backing bump arena is
//! `!Sync`). That matches JS single-threaded semantics exactly: a
//! `DataHandle` lives on the JS thread that constructed it and cannot be
//! shared across worker threads — napi class instances are not
//! transferable or cloneable through `postMessage`, so the type system
//! and the runtime agree here. Open one handle per worker.

use datalogic_rs::ParsedData;
use napi::Env;
use napi::bindgen_prelude::*;

use crate::error::{engine_error, guard};

/// An immutable, pre-parsed JSON document.
///
/// Construct once, then evaluate any number of rules against it —
/// `Rule.evaluateData`, `Session.evaluateData`, the typed
/// `Session.evaluateBool` / `evaluateInt` / `evaluateFloat` /
/// `evaluateTruthy`, and the batch entry points all take handles. A
/// handle is independent of any engine (one handle can feed rules
/// compiled by different engines) and is never consumed or mutated by
/// evaluation.
///
/// Handles are per-JS-thread: the underlying parsed tree is `Send` but
/// not `Sync`, which matches JS single-threaded semantics — a handle
/// cannot be shared across worker threads. Parse one per worker.
///
/// ```js
/// const handle = new DataHandle('{"user": {"age": 34}}');
/// rule.evaluateData(handle);          // no JSON parse per call
/// session.evaluateData(rule, handle); // hot path: arena reuse + no parse
/// ```
#[napi]
pub struct DataHandle {
    pub(crate) parsed: ParsedData,
}

#[napi]
impl DataHandle {
    /// Parse `json` into a reusable handle. Throws `ParseError` on
    /// malformed JSON.
    #[napi(catch_unwind, constructor)]
    pub fn new(env: Env, json: String) -> Result<Self> {
        guard(&env, || match ParsedData::from_json(&json) {
            Ok(parsed) => Ok(Self { parsed }),
            Err(e) => Err(engine_error(&env, &e, None)),
        })
    }

    /// Build a handle from a JS value (object, array, string, number,
    /// boolean or null) without serialising it to JSON first. The value is
    /// copied into the handle once; later changes to it are not seen.
    ///
    /// ```js
    /// const handle = DataHandle.fromValue({ user: { age: 34 } });
    /// ```
    #[napi(catch_unwind, factory)]
    pub fn from_value(env: Env, data: serde_json::Value) -> Result<Self> {
        guard(&env, || {
            Ok(Self {
                parsed: ParsedData::from_value(&data),
            })
        })
    }

    /// Bytes held by the handle's backing arena (input copy + parsed
    /// tree). Useful for sizing and diagnostics.
    #[napi(catch_unwind, getter)]
    pub fn allocated_bytes(&self) -> f64 {
        // A JS number rather than a u32, so a handle past 4 GiB does not
        // wrap; exact far beyond any handle.
        self.parsed.allocated_bytes() as f64
    }
}
