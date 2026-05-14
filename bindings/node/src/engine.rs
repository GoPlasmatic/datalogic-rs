//! `Engine` and `Rule` napi classes — the heart of the binding.

use std::collections::HashMap;
use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CustomOperator, DataValue, Engine as RsEngine, Error as DlError, Logic, Result as DlResult,
};
use napi::Env;
use napi::bindgen_prelude::*;
use napi::sys;
use serde_json::Value;

use crate::conv::unify_input;
use crate::error::engine_error;
use crate::session::Session;

/// Constructor options. Wrapped in an `Object` rather than passed
/// positionally because JS lacks keyword args — and the Python binding
/// makes `templating` keyword-only for the same reason. Accepting a
/// single options object keeps the API extensible without breaking
/// positional callers when we add fields later.
#[napi(object)]
pub struct EngineOptions {
    /// When `true`, multi-key objects in compiled rules become
    /// output-shaping templates (the engine's "templating mode").
    /// Defaults to `false`.
    pub templating: Option<bool>,
}

/// JSONLogic compile/evaluate engine.
///
/// Construct once at startup and share across calls — `Engine` is
/// internally `Arc<datalogic_rs::Engine>` and JS reference semantics mean
/// every reference points at the same underlying engine.
///
/// # Custom operators
///
/// Pass a `{name: fn}` map as the second constructor argument to register
/// custom JSONLogic operators. Each callback receives the evaluated args as
/// a JSON-array string and must return a JSON string of the result:
///
/// ```js
/// const engine = new Engine({}, {
///   double: (argsJson) => {
///     const [n] = JSON.parse(argsJson);
///     return JSON.stringify(n * 2);
///   }
/// });
/// engine.evalStr('{"double": [21]}', '{}'); // "42"
/// ```
///
/// Callbacks run synchronously on the same thread the engine was
/// constructed on. **An engine carrying custom operators must not be
/// shared across worker threads** — the JS function reference is bound
/// to the originating V8 isolate. Engines without custom operators are
/// free to cross threads as before.
#[napi]
pub struct Engine {
    pub(crate) inner: Arc<RsEngine>,
}

#[napi]
impl Engine {
    /// Create a new engine.
    #[napi(constructor)]
    pub fn new(
        env: Env,
        options: Option<EngineOptions>,
        custom_operators: Option<HashMap<String, FunctionRef<String, String>>>,
    ) -> Result<Self> {
        let templating = options.and_then(|o| o.templating).unwrap_or(false);
        let mut builder = if templating {
            RsEngine::builder().with_templating(true)
        } else {
            RsEngine::builder()
        };
        if let Some(map) = custom_operators {
            let env_raw = env.raw();
            for (name, callback) in map {
                builder = builder.add_operator(
                    name.clone(),
                    NodeOperator {
                        name,
                        callback,
                        env_raw,
                    },
                );
            }
        }
        Ok(Self {
            inner: Arc::new(builder.build()),
        })
    }

    /// Compile a JSONLogic rule into a reusable `Rule`. Accepts either a
    /// JS object literal or a JSON-encoded string.
    #[napi]
    pub fn compile(&self, env: Env, rule: Value) -> Result<Rule> {
        let logic = compile_inner(&env, &self.inner, rule)?;
        Ok(Rule {
            engine: self.inner.clone(),
            logic,
        })
    }

    /// One-shot evaluation. Compiles `rule` against `data` and returns
    /// the result as a JS value.
    ///
    /// For repeated evaluations of the same rule, prefer
    /// `compile()` + `Rule.evaluate()` — it skips re-parsing.
    #[napi]
    pub fn eval(&self, env: Env, rule: Value, data: Value) -> Result<Value> {
        let logic = compile_inner(&env, &self.inner, rule)?;
        evaluate_value(&env, &self.inner, &logic, data)
    }

    /// One-shot evaluation returning the result as a JSON string. Skips
    /// the JS-value materialisation — useful when the caller will hand
    /// the result straight to another JSON consumer.
    #[napi]
    pub fn eval_str(&self, env: Env, rule: Value, data: Value) -> Result<String> {
        let logic = compile_inner(&env, &self.inner, rule)?;
        evaluate_str(&env, &self.inner, &logic, data)
    }

    /// Open a hot-loop `Session` bound to this engine. The session
    /// reuses one bumpalo arena across calls and is reset between
    /// evaluations to bound peak memory.
    ///
    /// Sessions are not safe to share between worker threads — open one
    /// per worker.
    #[napi]
    pub fn session(&self) -> Session {
        Session::new(self.inner.clone())
    }
}

/// A compiled JSONLogic rule.
///
/// Hold one and call `evaluate()` against many data inputs without
/// re-parsing. `Rule` is thread-safe — share the same instance across
/// workers to evaluate in parallel.
#[napi]
pub struct Rule {
    pub(crate) engine: Arc<RsEngine>,
    pub(crate) logic: Arc<Logic>,
}

impl Rule {
    pub(crate) fn logic(&self) -> &Arc<Logic> {
        &self.logic
    }
}

#[napi]
impl Rule {
    /// Evaluate against `data` and return the result as a JS value.
    #[napi]
    pub fn evaluate(&self, env: Env, data: Value) -> Result<Value> {
        evaluate_value(&env, &self.engine, &self.logic, data)
    }

    /// Evaluate against `data` and return the result as a JSON string.
    /// Skips the JS-value materialisation entirely.
    #[napi]
    pub fn evaluate_str(&self, env: Env, data: Value) -> Result<String> {
        evaluate_str(&env, &self.engine, &self.logic, data)
    }
}

// =============== Custom operator bridge ===============

/// Custom operator backed by a JS callback. The callback receives a
/// JSON-array string of args and returns a JSON string of the result.
struct NodeOperator {
    name: String,
    callback: FunctionRef<String, String>,
    /// Raw napi env captured at registration. The CustomOperator trait
    /// runs without an `Env`, so we keep one to `borrow_back` the
    /// stored FunctionRef during evaluation. The pointer is valid for
    /// the V8 isolate lifetime — which is also the engine's lifetime
    /// when used from a single thread, the contract for this binding.
    env_raw: sys::napi_env,
}

// SAFETY: `FunctionRef` is `Send + Sync` (napi declares this so
// references can outlive the originating call scope). `sys::napi_env`
// is a raw pointer to the V8 isolate; we capture it once at
// construction and only dereference it on the same thread the engine
// was created on (the documented contract for custom-op-bearing
// engines).
unsafe impl Send for NodeOperator {}
unsafe impl Sync for NodeOperator {}

impl CustomOperator for NodeOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> DlResult<&'a DataValue<'a>> {
        // 1. Build the args JSON array.
        let mut json = String::from("[");
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&a.to_json_string());
        }
        json.push(']');

        // 2. Borrow the JS function back through the stored env and call it.
        let env = Env::from_raw(self.env_raw);
        let func = self.callback.borrow_back(&env).map_err(|e| {
            DlError::custom_message(format!(
                "custom operator '{}': failed to acquire JS function: {}",
                self.name, e
            ))
        })?;
        let ret_str: String = func.call(json).map_err(|e| {
            DlError::custom_message(format!("custom operator '{}' threw: {}", self.name, e))
        })?;

        // 3. Parse the returned JSON into the arena.
        let arena_str = arena.alloc_str(&ret_str);
        let parsed = DataValue::from_str(arena_str, arena).map_err(|e| {
            DlError::custom_message(format!(
                "custom operator '{}' returned invalid JSON: {}",
                self.name, e
            ))
        })?;
        Ok(arena.alloc(parsed))
    }
}

// ---------------- shared helpers ----------------

pub(crate) fn compile_inner(env: &Env, engine: &Arc<RsEngine>, rule: Value) -> Result<Arc<Logic>> {
    match rule {
        Value::String(s) => engine
            .compile_arc(s.as_str())
            .map_err(|e| engine_error(env, &e, None)),
        other => engine
            .compile_arc(&other)
            .map_err(|e| engine_error(env, &e, None)),
    }
}

pub(crate) fn evaluate_value(
    env: &Env,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: Value,
) -> Result<Value> {
    let value = unify_input(env, data)?;
    let arena = Bump::new();
    let av = engine
        .evaluate(logic, &value, &arena)
        .map_err(|e| engine_error(env, &e, Some(logic)))?;
    serde_json::to_value(av)
        .map_err(|e| engine_error(env, &datalogic_rs::Error::wrap(e), Some(logic)))
}

pub(crate) fn evaluate_str(
    env: &Env,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: Value,
) -> Result<String> {
    // Fast path for the common case (`data` already JSON string): hand
    // it straight to the engine's str entry point, which parses directly
    // into a DataValue without an intermediate `serde_json::Value`.
    if let Value::String(ref s) = data {
        let arena = Bump::new();
        let av = engine
            .evaluate(logic, s.as_str(), &arena)
            .map_err(|e| engine_error(env, &e, Some(logic)))?;
        return Ok(av.to_string());
    }
    let arena = Bump::new();
    let av = engine
        .evaluate(logic, &data, &arena)
        .map_err(|e| engine_error(env, &e, Some(logic)))?;
    Ok(av.to_string())
}
