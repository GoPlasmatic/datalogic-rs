//! Node.js native bindings for `datalogic-rs`.
//!
//! See `bindings/node/README.md` for the user-facing API. This file is
//! the napi-rs wiring that exposes [`engine::Engine`], [`engine::Rule`],
//! [`session::Session`], the top-level [`apply`] convenience, and the
//! structured error fields documented in [`error`].

#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

mod data;
mod engine;
mod error;
mod session;

use napi::Env;
use napi::bindgen_prelude::*;
use serde_json::Value;

pub use crate::data::DataHandle;
pub use crate::engine::{Engine, Rule};
pub use crate::session::Session;

/// Top-level convenience: compile `rule` and evaluate against `data` in
/// one call. Equivalent to `new Engine().compile(rule).evaluate(data)`.
///
/// Use this for ad-hoc one-shots. For repeated evaluations of the same
/// rule, hold an `Engine` and a `Rule` instance — that path skips the
/// per-call compile.
#[napi]
pub fn apply(env: Env, rule: Value, data: Value) -> Result<Value> {
    let engine = std::sync::Arc::new(datalogic_rs::Engine::new());
    let logic = engine::compile_inner(&env, &engine, rule)?;
    engine::evaluate_value(&env, &engine, &logic, data)
}
