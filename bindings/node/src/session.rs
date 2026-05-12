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

use datalogic_rs::Engine as RsEngine;
use datalogic_rs::bumpalo::Bump;
use napi::Env;
use napi::bindgen_prelude::*;
use serde_json::Value;

use crate::engine::Rule;
use crate::error::engine_error;

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
