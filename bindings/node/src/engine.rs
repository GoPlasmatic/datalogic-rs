//! `Engine` and `Rule` napi classes — the heart of the binding.

use std::collections::HashMap;
use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CustomOperator, DataValue, Engine as RsEngine, Error as DlError, EvaluationConfig, Logic,
    Result as DlResult,
};
use napi::Env;
use napi::bindgen_prelude::*;
use napi::sys;
use serde::Serialize;
use serde_json::Value;

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
    /// Evaluation configuration. Accepts either a plain JS object or a
    /// JSON-encoded string (the same dual-input convention `compile`
    /// uses for rules). Both funnel into the core crate's shared
    /// `EvaluationConfig::from_json_str` wire parser, so every binding
    /// accepts the same keys: `preset`, `arithmetic_nan_handling`,
    /// `division_by_zero`, `loose_equality_errors`, `truthy_evaluator`,
    /// `numeric_coercion`, `max_recursion_depth`. Unknown keys or
    /// values throw at construction with
    /// `errorType: "ConfigurationError"`.
    pub config: Option<Value>,
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
/// to the originating V8 isolate. If a custom operator is ever invoked
/// from a different thread, evaluation fails with a normal engine error
/// naming the operator instead of touching the foreign isolate. Engines
/// without custom operators are free to cross threads as before.
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
        let (templating, config) = match options {
            Some(o) => (o.templating.unwrap_or(false), o.config),
            None => (false, None),
        };
        let mut builder = if templating {
            RsEngine::builder().with_templating(true)
        } else {
            RsEngine::builder()
        };
        // JS `null` arrives as `Value::Null` rather than `None` through
        // the serde bridge; treat both as "not provided", matching the
        // other optional fields.
        if let Some(cfg) = config.filter(|c| !c.is_null()) {
            builder = builder.with_config(parse_config(&env, cfg)?);
        }
        if let Some(map) = custom_operators {
            let env_raw = env.raw();
            let thread_id = std::thread::current().id();
            for (name, callback) in map {
                builder = builder.add_operator(
                    name.clone(),
                    NodeOperator {
                        name,
                        callback,
                        env_raw,
                        thread_id,
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

    /// One-shot evaluation with a step-by-step execution trace.
    ///
    /// Both arguments are JSON-encoded strings. Returns a JSON string of
    /// the form `{ result, expression_tree, steps, error?,
    /// structured_error? }`, the same envelope the WASM package's
    /// `evaluateWithTrace` produces, so trace consumers (the React
    /// debugger among them) accept output from either binding.
    ///
    /// Runtime failures are reported inside the envelope rather than
    /// thrown: `result` is `null`, `error` carries the message, and
    /// `structured_error` the merged structured form. The rule is
    /// compiled with optimization disabled so every operator surfaces a
    /// step; use this for debugging, not hot paths.
    #[napi]
    pub fn evaluate_with_trace(&self, logic: String, data: String) -> Result<String> {
        let run = self.inner.trace().eval_str(logic.as_str(), data.as_str());
        Ok(traced_run_to_json(&run))
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
    /// stored FunctionRef during evaluation. The pointer belongs to the
    /// V8 isolate of `thread_id` and is only meaningful there;
    /// `evaluate` verifies that before touching it.
    env_raw: sys::napi_env,
    /// Thread the operator was registered on (the thread that owns
    /// `env_raw`'s isolate). `evaluate` refuses to run anywhere else.
    thread_id: std::thread::ThreadId,
}

// SAFETY: `FunctionRef` is `Send + Sync` (napi declares this so
// references can outlive the originating call scope). `sys::napi_env`
// is a raw pointer to per-isolate state that must only be dereferenced
// on the thread that owns the isolate. The invariant that makes these
// impls sound: `evaluate` compares `std::thread::current().id()`
// against the captured `thread_id` first and returns a normal engine
// error on mismatch, so `env_raw` is dereferenced only after the
// thread-affinity check has passed on the registering thread.
unsafe impl Send for NodeOperator {}
unsafe impl Sync for NodeOperator {}

impl CustomOperator for NodeOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> DlResult<&'a DataValue<'a>> {
        // 0. Thread-affinity guard. `env_raw` is only valid on the
        //    registering thread; crossing threads must fail as a normal
        //    engine error, never as a dereference of a foreign isolate.
        if std::thread::current().id() != self.thread_id {
            return Err(DlError::custom_message(format!(
                "custom operator '{}' was invoked from a different thread than the one that \
                 registered it; Node custom-operator engines are single-threaded",
                self.name
            )));
        }

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

/// Parse the `config` constructor option into an [`EvaluationConfig`].
/// A JS string is treated as JSON text; anything else is serialized
/// back to JSON first (mirroring `compile_inner`'s dual-input
/// convention). Both funnel into the core crate's shared
/// `EvaluationConfig::from_json_str` parser, so every binding rejects
/// the same typos with the same messages.
fn parse_config(env: &Env, config: Value) -> Result<EvaluationConfig> {
    let json = match config {
        Value::String(s) => s,
        other => serde_json::to_string(&other)
            .map_err(|e| engine_error(env, &DlError::wrap(e), None))?,
    };
    EvaluationConfig::from_json_str(&json).map_err(|e| engine_error(env, &e, None))
}

/// Render a [`datalogic_rs::TracedRun`] into the JS wire shape shared
/// with the WASM binding: `{ result, expression_tree, steps, error?,
/// structured_error? }`.
fn traced_run_to_json(run: &datalogic_rs::TracedRun<String>) -> String {
    #[derive(Serialize)]
    struct Wire<'a> {
        result: Value,
        expression_tree: &'a datalogic_rs::ExpressionNode,
        steps: &'a [datalogic_rs::ExecutionStep],
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_error: Option<&'a DlError>,
    }

    let result_json: Value;
    let mut error_msg: Option<String> = None;
    let mut error_struct: Option<&DlError> = None;
    match &run.result {
        Ok(s) => {
            // The String is already JSON; surface it as the parsed value
            // when possible, falling back to a JSON string otherwise.
            result_json = serde_json::from_str::<Value>(s.as_str())
                .unwrap_or_else(|_| Value::String(s.to_string()));
        }
        Err(e) => {
            result_json = Value::Null;
            error_msg = Some(e.to_string());
            error_struct = Some(e);
        }
    }
    serde_json::to_string(&Wire {
        result: result_json,
        expression_tree: &run.expression_tree,
        steps: &run.steps,
        error: error_msg,
        structured_error: error_struct,
    })
    .unwrap_or_default()
}

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
    // A JSON-string input parses straight into the arena via the engine's
    // `&str` entry point (mirroring `evaluate_str` and the Session
    // methods) instead of round-tripping through a second
    // `serde_json::Value` tree.
    let arena = Bump::new();
    let av = match &data {
        Value::String(s) => engine
            .evaluate(logic, s.as_str(), &arena)
            .map_err(|e| engine_error(env, &e, Some(logic)))?,
        other => engine
            .evaluate(logic, other, &arena)
            .map_err(|e| engine_error(env, &e, Some(logic)))?,
    };
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
