//! Shared conversion helpers — JS string ↔ `serde_json::Value`.
//!
//! napi-rs's `serde-json` feature auto-converts JS objects to
//! `serde_json::Value` at the parameter boundary; a JS string lands as
//! `Value::String(..)`. The binding's input convention (mirroring the
//! Python binding) lets callers pass either:
//!
//!   * a JS object literal → arrives as `Value::Object(..)` / array / scalar
//!   * a JSON-encoded string → arrives as `Value::String("...")` and needs
//!     a re-parse before it can be handed to the engine
//!
//! [`unify_input`] normalises both into a single `Value` that the engine
//! can consume.

use napi::Env;
use napi::bindgen_prelude::*;
use serde_json::Value;

use crate::error::parse_error;

/// Normalise a JS-side input into a `Value`. A `Value::String` is
/// interpreted as a JSON-encoded payload and re-parsed; anything else
/// is returned as-is.
///
/// Parse failure surfaces as a `ParseError` rather than a cryptic napi
/// internal error so callers can `catch (e => e.name === 'ParseError')`.
pub fn unify_input(env: &Env, input: Value) -> Result<Value> {
    match input {
        Value::String(s) => serde_json::from_str(&s)
            .map_err(|e| parse_error(env, &format!("failed to parse JSON input: {e}"))),
        other => Ok(other),
    }
}
