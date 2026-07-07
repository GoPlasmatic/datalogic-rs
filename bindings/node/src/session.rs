//! `Session` napi class — hot-loop arena reuse, single-threaded.
//!
//! Owns a [`bumpalo::Bump`] that is `!Sync`. In JS terms that means a
//! Session instance must not be shared between worker threads — each
//! worker that wants a hot-loop arena should open its own via
//! `engine.session()`. Node's single-threaded model on the main agent
//! makes this the natural case; the `!Sync` bound only matters when
//! someone tries to transfer the instance through `MessageChannel`.
//!
//! The arena is reset at the start of each `evaluate*` call to bound
//! peak memory across iterations — the previous call's owned result is
//! already materialised by then.

use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::{DataValue, Engine as RsEngine};
use napi::Env;
use napi::bindgen_prelude::*;
use serde_json::{Value, json};

use crate::data::DataHandle;
use crate::engine::Rule;
use crate::error::{engine_error, type_mismatch_error};

/// JSON type name for TypeMismatch messages (same wording as the C
/// ABI's `type_of` in `bindings/c/src/session.rs`).
fn type_of(v: &DataValue<'_>) -> &'static str {
    if v.is_null() {
        "null"
    } else if v.is_bool() {
        "boolean"
    } else if v.is_number() {
        "number"
    } else if v.is_string() {
        "string"
    } else if v.is_array() {
        "array"
    } else {
        "object"
    }
}

/// One item's outcome rendered in the `Promise.allSettled` shape the
/// batch entry points return: `{status: "fulfilled", value}` or
/// `{status: "rejected", reason: {tag, message, operator?}}`. The
/// reason object carries the same fields the C ABI's per-item error
/// JSON does.
fn batch_item(outcome: std::result::Result<String, &datalogic_rs::Error>) -> Value {
    match outcome {
        Ok(value) => json!({"status": "fulfilled", "value": value}),
        Err(e) => {
            let mut reason = json!({"tag": e.tag(), "message": e.to_string()});
            if let Some(op) = e.operator() {
                reason["operator"] = Value::String(op.to_string());
            }
            json!({"status": "rejected", "reason": reason})
        }
    }
}

#[napi]
pub struct Session {
    engine: Arc<RsEngine>,
    arena: Bump,
}

impl Session {
    pub(crate) fn new(engine: Arc<RsEngine>) -> Self {
        Self {
            engine,
            arena: Bump::new(),
        }
    }
}

#[napi]
impl Session {
    /// Evaluate `rule` against `data` and return the result as a JS value.
    #[napi]
    pub fn evaluate(&mut self, env: Env, rule: &Rule, data: Value) -> Result<Value> {
        // Reset BEFORE each call so the previous iteration's allocations
        // don't accumulate. The previous call's result was materialised
        // as an owned `serde_json::Value` (or `String`) before returning,
        // so resetting here is safe.
        self.arena.reset();

        let av = match &data {
            Value::String(s) => self
                .engine
                .evaluate(rule.logic(), s.as_str(), &self.arena)
                .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?,
            other => self
                .engine
                .evaluate(rule.logic(), other, &self.arena)
                .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?,
        };
        serde_json::to_value(av)
            .map_err(|e| engine_error(&env, &datalogic_rs::Error::wrap(e), Some(rule.logic())))
    }

    /// Evaluate `rule` against `data` and return the result as a JSON
    /// string. Skips the JS-value materialisation entirely — the fastest
    /// path through the binding.
    #[napi]
    pub fn evaluate_str(&mut self, env: Env, rule: &Rule, data: Value) -> Result<String> {
        self.arena.reset();

        let av = match &data {
            Value::String(s) => self
                .engine
                .evaluate(rule.logic(), s.as_str(), &self.arena)
                .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?,
            other => self
                .engine
                .evaluate(rule.logic(), other, &self.arena)
                .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?,
        };
        Ok(av.to_string())
    }

    /// Evaluate `rule` against a pre-parsed `DataHandle` and return the
    /// result as a JS value. The hot path for object results: session
    /// arena reuse plus zero JSON parse work per call.
    ///
    /// The rule's compiled logic is evaluated by **this session's
    /// engine** (its configuration and custom operators apply) — same
    /// contract as `evaluate`.
    #[napi(ts_return_type = "unknown")]
    pub fn evaluate_data(&mut self, env: Env, rule: &Rule, handle: &DataHandle) -> Result<Value> {
        self.arena.reset();
        let av = self
            .engine
            .evaluate(rule.logic(), &handle.parsed, &self.arena)
            .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?;
        serde_json::to_value(av)
            .map_err(|e| engine_error(&env, &datalogic_rs::Error::wrap(e), Some(rule.logic())))
    }

    /// Evaluate `rule` against a pre-parsed `DataHandle` and return the
    /// result as a JSON string — the fastest session path: no input
    /// parse, no JS-value materialisation.
    #[napi]
    pub fn evaluate_data_str(
        &mut self,
        env: Env,
        rule: &Rule,
        handle: &DataHandle,
    ) -> Result<String> {
        self.arena.reset();
        let av = self
            .engine
            .evaluate(rule.logic(), &handle.parsed, &self.arena)
            .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?;
        Ok(av.to_string())
    }

    // =============== typed scalar results ===============
    //
    // Handle-input only, mirroring the C ABI family: the
    // predicate-heavy flows that want typed results are exactly the
    // flows that parse data once. These skip JSON serialization of the
    // result entirely.

    /// Evaluate `rule` and return the result as a strict JSON boolean.
    /// Any other result type throws an `EvaluateError` with
    /// `errorType: "TypeMismatch"`; for JSONLogic truthiness coercion
    /// use `evaluateTruthy`.
    #[napi]
    pub fn evaluate_bool(&mut self, env: Env, rule: &Rule, handle: &DataHandle) -> Result<bool> {
        self.arena.reset();
        let av = self
            .engine
            .evaluate(rule.logic(), &handle.parsed, &self.arena)
            .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?;
        av.as_bool().ok_or_else(|| {
            type_mismatch_error(
                &env,
                &format!("result is not a boolean (got {})", type_of(av)),
            )
        })
    }

    /// Evaluate `rule` and return the result as a number. Accepts any
    /// JSON number (JS has a single number type, so there is no
    /// separate integer variant); any other result type throws an
    /// `EvaluateError` with `errorType: "TypeMismatch"`.
    #[napi]
    pub fn evaluate_number(&mut self, env: Env, rule: &Rule, handle: &DataHandle) -> Result<f64> {
        self.arena.reset();
        let av = self
            .engine
            .evaluate(rule.logic(), &handle.parsed, &self.arena)
            .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?;
        av.as_f64().ok_or_else(|| {
            type_mismatch_error(
                &env,
                &format!("result is not a number (got {})", type_of(av)),
            )
        })
    }

    /// Evaluate `rule` and collapse the result to a boolean via the
    /// engine's configured truthiness rules (the same coercion `if`,
    /// `and`, and `or` apply). Never type-mismatches — any result
    /// truthy-converts.
    #[napi]
    pub fn evaluate_truthy(&mut self, env: Env, rule: &Rule, handle: &DataHandle) -> Result<bool> {
        self.arena.reset();
        let av = self
            .engine
            .evaluate(rule.logic(), &handle.parsed, &self.arena)
            .map_err(|e| engine_error(&env, &e, Some(rule.logic())))?;
        Ok(self.engine.truthy(av))
    }

    // =============== batch evaluation ===============

    /// Evaluate one rule against many pre-parsed `DataHandle`s in a
    /// single native call. Returns one entry per handle, in order, in
    /// the `Promise.allSettled` shape:
    ///
    /// ```text
    /// { status: "fulfilled", value: string }              // result JSON
    /// { status: "rejected",  reason: { tag, message, operator? } }
    /// ```
    ///
    /// Item failures never throw and never affect their neighbours —
    /// one bad input cannot poison the batch. Argument errors (a
    /// non-`DataHandle` in the array, a null rule, …) do throw. The
    /// arena is reset between items; each item's result is materialised
    /// before the next item runs.
    #[napi(
        ts_return_type = "Array<{ status: 'fulfilled', value: string } | { status: 'rejected', reason: { tag: string, message: string, operator?: string } }>"
    )]
    pub fn evaluate_batch(
        &mut self,
        rule: &Rule,
        handles: Vec<ClassInstance<DataHandle>>,
    ) -> Result<Vec<Value>> {
        let mut out = Vec::with_capacity(handles.len());
        for handle in &handles {
            // Scratch from the previous item is dead — its result was
            // materialised as an owned String by `batch_item`.
            self.arena.reset();
            let item = match self
                .engine
                .evaluate(rule.logic(), &handle.parsed, &self.arena)
            {
                Ok(av) => batch_item(Ok(av.to_string())),
                Err(e) => batch_item(Err(&e)),
            };
            out.push(item);
        }
        Ok(out)
    }

    /// Evaluate many rules against one pre-parsed `DataHandle` in a
    /// single native call — the rule-set / feature-flag shape. Same
    /// per-item `allSettled` result shape, ordering, and error
    /// semantics as `evaluateBatch`.
    ///
    /// Every rule's compiled logic is evaluated by **this session's
    /// engine**, exactly like the single-rule methods.
    #[napi(
        ts_return_type = "Array<{ status: 'fulfilled', value: string } | { status: 'rejected', reason: { tag: string, message: string, operator?: string } }>"
    )]
    pub fn evaluate_many(
        &mut self,
        rules: Vec<ClassInstance<Rule>>,
        handle: &DataHandle,
    ) -> Result<Vec<Value>> {
        let mut out = Vec::with_capacity(rules.len());
        for rule in &rules {
            self.arena.reset();
            let item = match self
                .engine
                .evaluate(rule.logic(), &handle.parsed, &self.arena)
            {
                Ok(av) => batch_item(Ok(av.to_string())),
                Err(e) => batch_item(Err(&e)),
            };
            out.push(item);
        }
        Ok(out)
    }

    /// Reset the underlying arena. Calling this is optional — `evaluate*`
    /// resets at the start of each call.
    #[napi]
    pub fn reset(&mut self) {
        self.arena.reset();
    }

    /// Bytes currently allocated to the session's arena (sum of all
    /// chunks). Useful for sizing or diagnostics.
    #[napi]
    pub fn allocated_bytes(&self) -> u32 {
        self.arena.allocated_bytes() as u32
    }
}
