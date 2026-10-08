//! `Engine` and `Rule` napi classes — the heart of the binding.

use std::collections::HashMap;
use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CheckMode, CustomOperator, DataValue, Engine as RsEngine, Error as DlError, EvaluationConfig,
    Logic, Result as DlResult,
};
use napi::bindgen_prelude::*;
use napi::sys;
use napi::{Env, Task};
use serde_json::Value;

use crate::data::DataHandle;
use crate::error::{
    INTERNAL_ERROR, engine_error, engine_error_value, guard, internal_error_value, panic_message,
};
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
    /// `division_by_zero`, `loose_equality_errors`, `missing_var`,
    /// `truthy_evaluator`, `numeric_coercion`, `max_recursion_depth`,
    /// `ops_budget`. Unknown keys or values throw at construction with
    /// `errorType: "ConfigurationError"`.
    pub config: Option<Value>,
    /// Single-character prefix that marks a template key as a literal
    /// output field instead of an operator invocation. Unset by default.
    ///
    /// In templating mode a single-key object is always an operator
    /// invocation, so a key naming a built-in (`type`, `map`, `if`,
    /// `length`, …) or a registered custom operator can never be emitted
    /// as an output field. With this set, exactly one leading prefix is
    /// stripped from every template key and an escaped key is never
    /// resolved as an operator: with `"$"`, `{"$type": ...}` emits the key
    /// `type` and `{"$$type": ...}` emits a literal `$type`.
    ///
    /// Only meaningful together with `templating`. Anything other than a
    /// one-character string throws at construction with
    /// `errorType: "InvalidArguments"`.
    pub template_key_escape: Option<String>,
    /// When `true`, a custom operator named like a built-in (`length`,
    /// `var`, an alias such as `?:`) throws at construction with
    /// `errorType: "ConfigurationError"` instead of being registered and
    /// never running. Defaults to `false`.
    pub strict_operator_names: Option<bool>,
    /// The operator families the engine has besides the JSONLogic core
    /// (`["ExtString", "DateTime"]`: the `family` of each `operators()`
    /// row). Unset, the engine has every family. A family left out is not
    /// there for the engine: its names compile as unknown operators, and a
    /// custom operator may take them. An unknown family name throws at
    /// construction with `errorType: "ConfigurationError"`.
    pub families: Option<Vec<String>>,
}

/// An evaluation's result paired with what it cost, returned by the
/// `*Metered` methods.
#[napi(object)]
pub struct MeteredResult {
    /// The evaluation's result, as a JSON string. A string rather than a
    /// JS value so metering costs no more than `evaluateStr`; parse it
    /// with `JSON.parse` when a value is wanted.
    pub result: String,
    /// Operations charged: one per dispatched node, one per item an
    /// iterator examined, plus whatever an operator charged for the data
    /// it moved. Literals and constant-folded subtrees cost nothing.
    ///
    /// Typed as a JS number, which is exact to 2^53 — far past any
    /// budget worth setting.
    pub ops: f64,
}

/// JSONLogic compile/evaluate engine.
///
/// Construct once at startup and share across calls — `Engine` is
/// internally `Arc<datalogic_rs::Engine>` and JS reference semantics mean
/// every reference points at the same underlying engine. Like every napi
/// class instance it cannot be posted or transferred to a worker thread:
/// each worker loads the module and builds its own.
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
/// constructed on: the JS function reference is bound to the originating
/// V8 isolate. If a custom operator is ever invoked from a different
/// thread (`Rule.evaluateStrAsync` runs on the libuv pool), evaluation
/// fails with a normal engine error naming the operator instead of
/// touching the foreign isolate.
#[napi]
pub struct Engine {
    pub(crate) inner: Arc<RsEngine>,
}

#[napi]
impl Engine {
    /// Create a new engine.
    #[napi(catch_unwind, constructor)]
    pub fn new(
        env: Env,
        options: Option<EngineOptions>,
        custom_operators: Option<HashMap<String, FunctionRef<String, String>>>,
    ) -> Result<Self> {
        guard(&env, || {
            let (templating, config, key_escape, strict_names, families) = match options {
                Some(o) => (
                    o.templating.unwrap_or(false),
                    o.config,
                    o.template_key_escape,
                    o.strict_operator_names.unwrap_or(false),
                    o.families,
                ),
                None => (false, None, None, false, None),
            };
            let mut opts = datalogic_bind::EngineOptions {
                templating,
                ..Default::default()
            };
            if let Some(names) = families {
                let families = datalogic_bind::families(names.iter().map(String::as_str))
                    .map_err(|msg| engine_error(&env, &DlError::configuration_error(msg), None))?;
                opts.families = Some(families);
            }
            // Reject a mis-typed escape at construction rather than silently
            // ignoring it: an option that looks accepted but does nothing is
            // worse than a loud failure.
            if let Some(prefix) = key_escape {
                let c = datalogic_bind::single_char(&prefix).ok_or_else(|| {
                    engine_error(
                        &env,
                        &DlError::invalid_arguments(
                            "templateKeyEscape must be exactly one character",
                        ),
                        None,
                    )
                })?;
                opts.template_key_escape = Some(c);
            }
            // JS `null` arrives as `Value::Null` rather than `None` through
            // the serde bridge; treat both as "not provided", matching the
            // other optional fields.
            if let Some(cfg) = config.filter(|c| !c.is_null()) {
                opts.config = Some(parse_config(&env, cfg)?);
            }
            let env_raw = env.raw();
            let thread_id = std::thread::current().id();
            let operators = custom_operators
                .into_iter()
                .flatten()
                .map(|(name, callback)| {
                    let op = NodeOperator {
                        name: name.clone(),
                        callback,
                        env_raw,
                        thread_id,
                    };
                    (name, op)
                });
            let builder = datalogic_bind::add_operators(opts.builder(), operators, strict_names)
                .map_err(|e| engine_error(&env, &e, None))?;
            Ok(Self {
                inner: Arc::new(builder.build()),
            })
        })
    }

    /// Compile a JSONLogic rule into a reusable `Rule`. Accepts either a
    /// JS object literal or a JSON-encoded string.
    #[napi(catch_unwind)]
    pub fn compile(&self, env: Env, rule: Value) -> Result<Rule> {
        guard(&env, || {
            let logic = compile_inner(&env, &self.inner, rule)?;
            Ok(self.rule(logic))
        })
    }

    /// Compile `rule` in templating mode, whatever this engine was built
    /// with: a multi-key object is an output template and an unknown key an
    /// output field.
    #[napi(catch_unwind)]
    pub fn compile_template(&self, env: Env, rule: Value) -> Result<Rule> {
        guard(&env, || self.compile_in(&env, rule, CheckMode::Template))
    }

    /// Compile `rule` outside templating mode, whatever this engine was
    /// built with. A multi-key object fails to compile; an unknown
    /// operator compiles and fails with `InvalidOperator` when evaluated
    /// (`compileChecked` refuses it up front).
    #[napi(catch_unwind)]
    pub fn compile_strict(&self, env: Env, rule: Value) -> Result<Rule> {
        guard(&env, || self.compile_in(&env, rule, CheckMode::Strict))
    }

    /// Compile `rule`, refusing it if `check` finds any error. Throws
    /// `errorType: "CompileError"` with a `diagnostics` array.
    #[napi(catch_unwind)]
    pub fn compile_checked(&self, env: Env, rule: Value) -> Result<Rule> {
        guard(&env, || {
            let logic = with_rule(rule, |r| match r {
                RuleSrc::Text(s) => self.inner.compile_checked(s),
                RuleSrc::Json(v) => self.inner.compile_checked(v),
            })
            .map_err(|e| crate::error::compile_error(&env, &e))?;
            Ok(self.rule(Arc::new(logic)))
        })
    }

    /// Every problem this engine can see in `rule` before it runs, as an
    /// array of `{code, severity, message, pointer, operator}`. `mode` is
    /// `"engine"` (default), `"strict"` or `"template"`.
    #[napi(
        catch_unwind,
        ts_return_type = "Array<{ code: string; severity: 'error' | 'warning'; message: string; pointer: string; operator: string | null }>"
    )]
    pub fn check(&self, env: Env, rule: Value, mode: Option<String>) -> Result<Value> {
        guard(&env, || {
            let mode = datalogic_bind::check_mode(mode.as_deref())
                .map_err(|msg| engine_error(&env, &DlError::invalid_arguments(msg), None))?;
            let diagnostics = with_rule(rule, |r| match r {
                RuleSrc::Text(s) => self.inner.check(s, mode),
                RuleSrc::Json(v) => self.inner.check(v, mode),
            });
            Ok(datalogic_bind::diagnostics_value(&diagnostics))
        })
    }

    /// Every built-in operator this engine evaluates: name, aliases,
    /// family, gating feature, argument counts, whether it reads the data,
    /// its effect, its cost class and which argument runs per element.
    #[napi(
        catch_unwind,
        ts_return_type = "Array<{ name: string; aliases: string[]; family: string; feature: string | null; min_args: number; max_args: number | null; reads_context: boolean; effect: string; cost: string; scoped_arg: number | 'last' | null }>"
    )]
    pub fn operators(&self) -> Value {
        datalogic_bind::operators_value(&self.inner)
    }

    /// Whether `value` is truthy under this engine's configured
    /// truthiness. Under the default rules an empty object is falsy, like
    /// an empty array. A string is JSON text, as data is everywhere else
    /// here (and as WASM's `truthy` reads it): `truthy("[]")` is `false`,
    /// and `truthy('"a"')` asks about the string `a`.
    #[napi(catch_unwind)]
    pub fn truthy(&self, env: Env, value: Value) -> Result<bool> {
        guard(&env, || match value {
            Value::String(s) => datalogic_rs::ParsedData::from_json(&s)
                .map(|parsed| self.inner.truthy_of(&parsed))
                .map_err(|e| engine_error(&env, &e, None)),
            other => Ok(self.inner.truthy_of(&other)),
        })
    }

    /// One-shot evaluation. Compiles `rule` against `data` and returns
    /// the result as a JS value.
    ///
    /// For repeated evaluations of the same rule, prefer
    /// `compile()` + `Rule.evaluate()` — it skips re-parsing.
    #[napi(catch_unwind)]
    pub fn eval(&self, env: Env, rule: Value, data: Value) -> Result<Value> {
        guard(&env, || {
            let logic = compile_inner(&env, &self.inner, rule)?;
            evaluate_value(&env, &self.inner, &logic, data)
        })
    }

    /// One-shot evaluation returning the result as a JSON string. Skips
    /// the JS-value materialisation — useful when the caller will hand
    /// the result straight to another JSON consumer.
    #[napi(catch_unwind)]
    pub fn eval_str(&self, env: Env, rule: Value, data: Value) -> Result<String> {
        guard(&env, || {
            let logic = compile_inner(&env, &self.inner, rule)?;
            evaluate_str(&env, &self.inner, &logic, data)
        })
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
    /// step; use this for debugging, not hot paths. `mode` is `"engine"`
    /// (default), `"strict"` or `"template"`, as for `check`, so a rule
    /// compiled with `compileTemplate` is traced as one.
    #[napi(catch_unwind)]
    pub fn evaluate_with_trace(
        &self,
        env: Env,
        logic: String,
        data: String,
        mode: Option<String>,
    ) -> Result<String> {
        guard(&env, || {
            let mode = datalogic_bind::check_mode(mode.as_deref())
                .map_err(|msg| engine_error(&env, &DlError::invalid_arguments(msg), None))?;
            Ok(datalogic_bind::traced_json_in(
                &self.inner,
                logic.as_str(),
                data.as_str(),
                mode,
            ))
        })
    }

    /// One-shot metered evaluation: compile `rule`, evaluate it against
    /// `data` under an operation budget, and report what it cost.
    ///
    /// `budget` caps the operations the rule may charge; omit it to fall
    /// back to this engine's `config.ops_budget`, and omit both to meter
    /// without bounding.
    ///
    /// An operation is one dispatched node, one item examined by an
    /// iterator, or whatever an operator charges for the data it moves —
    /// the tensor family prices itself in elements. Literals and
    /// constant-folded subtrees cost nothing. The count is deterministic
    /// for a given rule, data and engine version; budget for the work you
    /// want to allow rather than for a number you measured.
    ///
    /// Throws `errorType: "BudgetExceeded"` (carrying `budget` and
    /// `spent`) when the rule charges past the ceiling. The evaluation is
    /// refused before the work, and a `try` in the rule cannot recover.
    #[napi(catch_unwind)]
    pub fn eval_metered(
        &self,
        env: Env,
        rule: Value,
        data: Value,
        budget: Option<f64>,
    ) -> Result<MeteredResult> {
        guard(&env, || {
            let logic = compile_inner(&env, &self.inner, rule)?;
            evaluate_metered(&env, &self.inner, &logic, data, budget)
        })
    }

    /// Open a hot-loop `Session` bound to this engine. The session
    /// reuses one bumpalo arena across calls and is reset between
    /// evaluations to bound peak memory.
    ///
    /// Sessions are not safe to share between worker threads — open one
    /// per worker.
    #[napi(catch_unwind)]
    pub fn session(&self) -> Session {
        Session::new(self.inner.clone())
    }

    /// Names of the custom operators registered on this engine (second
    /// constructor argument), in no particular order. Built-ins are listed
    /// by the module-level `builtinOperatorNames()`.
    #[napi(catch_unwind, js_name = "customOperatorNames")]
    pub fn custom_operator_names(&self) -> Vec<String> {
        self.inner
            .custom_operator_names()
            .map(str::to_owned)
            .collect()
    }
}

impl Engine {
    fn rule(&self, logic: Arc<Logic>) -> Rule {
        Rule {
            engine: self.inner.clone(),
            logic,
        }
    }

    /// Compile `rule` as `mode` reads it, whatever this engine was built
    /// with.
    fn compile_in(&self, env: &Env, rule: Value, mode: CheckMode) -> Result<Rule> {
        let logic = with_rule(rule, |r| match r {
            RuleSrc::Text(s) => datalogic_bind::compile_in(&self.inner, s, mode),
            RuleSrc::Json(v) => datalogic_bind::compile_in(&self.inner, v, mode),
        })
        .map_err(|e| engine_error(env, &e, None))?;
        Ok(self.rule(Arc::new(logic)))
    }
}

/// A rule as the host passed it: JSON text, or a JS value.
enum RuleSrc<'a> {
    Text(&'a str),
    Json(&'a Value),
}

fn with_rule<T>(rule: Value, f: impl FnOnce(RuleSrc<'_>) -> T) -> T {
    match &rule {
        Value::String(s) => f(RuleSrc::Text(s)),
        other => f(RuleSrc::Json(other)),
    }
}

/// A compiled JSONLogic rule.
///
/// Hold one and call `evaluate()` against many data inputs without
/// re-parsing. `Rule` has no thread affinity on the Rust side, but napi
/// class instances cannot be posted or transferred to worker threads:
/// each worker compiles its own. To evaluate off the JS thread, use
/// `evaluateStrAsync`, which runs on the libuv pool.
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
    /// What the rule reads and calls, from a walk of the compiled rule:
    /// `{reads, computed_reads, reads_complete, reads_data, operators,
    /// custom_operators, deterministic}`. `reads` lists each root path as
    /// its segments.
    #[napi(
        catch_unwind,
        ts_return_type = "{ reads: string[][]; computed_reads: boolean; reads_complete: boolean; reads_data: boolean; operators: string[]; custom_operators: string[]; deterministic: boolean }"
    )]
    pub fn facts(&self) -> Value {
        datalogic_bind::facts_value(&self.logic.facts())
    }

    /// Evaluate against `data` and return the result as a JS value.
    #[napi(catch_unwind)]
    pub fn evaluate(&self, env: Env, data: Value) -> Result<Value> {
        guard(&env, || {
            evaluate_value(&env, &self.engine, &self.logic, data)
        })
    }

    /// Evaluate against `data` and return the result as a JSON string.
    /// Skips the JS-value materialisation entirely.
    #[napi(catch_unwind)]
    pub fn evaluate_str(&self, env: Env, data: Value) -> Result<String> {
        guard(&env, || evaluate_str(&env, &self.engine, &self.logic, data))
    }

    /// Evaluate against `data` under an operation budget, returning the
    /// result JSON and what the evaluation cost.
    ///
    /// Same metering as `Engine.evalMetered`, on an already-compiled
    /// rule. Omit `budget` to fall back to the engine's
    /// `config.ops_budget`.
    #[napi(catch_unwind)]
    pub fn evaluate_metered(
        &self,
        env: Env,
        data: Value,
        budget: Option<f64>,
    ) -> Result<MeteredResult> {
        guard(&env, || {
            evaluate_metered(&env, &self.engine, &self.logic, data, budget)
        })
    }

    /// Evaluate against a pre-parsed `DataHandle` and return the result
    /// as a JS value. Skips the per-call JSON parse of the data — parse
    /// once with `new DataHandle(json)`, evaluate many times.
    #[napi(catch_unwind, ts_return_type = "unknown")]
    pub fn evaluate_data(&self, env: Env, handle: &DataHandle) -> Result<Value> {
        guard(&env, || {
            let arena = Bump::new();
            let av = self
                .engine
                .evaluate(&self.logic, &handle.parsed, &arena)
                .map_err(|e| engine_error(&env, &e, Some(&self.logic)))?;
            serde_json::to_value(av)
                .map_err(|e| engine_error(&env, &DlError::wrap(e), Some(&self.logic)))
        })
    }

    /// Evaluate against a pre-parsed `DataHandle` and return the result
    /// as a JSON string — no input parse, no JS-value materialisation.
    #[napi(catch_unwind)]
    pub fn evaluate_data_str(&self, env: Env, handle: &DataHandle) -> Result<String> {
        guard(&env, || {
            let arena = Bump::new();
            let av = self
                .engine
                .evaluate(&self.logic, &handle.parsed, &arena)
                .map_err(|e| engine_error(&env, &e, Some(&self.logic)))?;
            Ok(av.to_string())
        })
    }

    /// Evaluate `dataJson` (a JSON string) on the libuv thread pool and
    /// resolve with the result as a JSON string.
    ///
    /// This is not faster per operation than `evaluateStr` — the win is
    /// event-loop hygiene: a large payload's parse + evaluate + serialize
    /// happens off the JS thread, so use it when payloads are big enough
    /// to cause noticeable event-loop stalls, or to overlap evaluation
    /// with other work. String input only: `DataHandle` is pinned to the
    /// JS thread and cannot cross to the pool.
    ///
    /// Rejections carry the same structured fields as the synchronous
    /// throws (`name`, `errorType`, `operator`, `nodeIds`, `path`).
    /// Rules from engines with custom operators reject if evaluation
    /// reaches a JS-backed operator (the callback is pinned to the JS
    /// thread).
    #[napi(catch_unwind, ts_return_type = "Promise<string>")]
    pub fn evaluate_str_async(&self, data_json: String) -> AsyncTask<EvaluateStrTask> {
        AsyncTask::new(EvaluateStrTask {
            engine: self.engine.clone(),
            logic: self.logic.clone(),
            data: data_json,
            failure: None,
        })
    }
}

/// Background evaluation job behind `Rule.evaluateStrAsync`. `compute`
/// runs on the libuv pool with a task-local arena; `Engine` and `Logic`
/// are both `Send + Sync` behind `Arc`s, and the data crosses as an
/// owned `String`.
///
/// `compute` runs under `catch_unwind`: napi-rs calls it from a bare
/// `extern "C"` libuv callback with no unwind guard of its own, so a
/// panic escaping it would abort the whole Node process.
pub struct EvaluateStrTask {
    engine: Arc<RsEngine>,
    logic: Arc<Logic>,
    data: String,
    /// Failure smuggled from the pool thread to `reject`, which runs on
    /// the JS thread and can build the decorated JS Error there.
    failure: Option<TaskFailure>,
}

/// Why `EvaluateStrTask::compute` failed.
enum TaskFailure {
    /// The engine returned an error.
    Engine(DlError),
    /// The engine panicked; carries the panic message.
    Panic(String),
}

impl Task for EvaluateStrTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        // `AssertUnwindSafe`: on a panic the arena and its borrows are
        // dropped with the closure, and nothing else is mutated.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let arena = Bump::new();
            self.engine
                .evaluate(&self.logic, self.data.as_str(), &arena)
                .map(|av| av.to_string())
        }));
        // Fallback reasons, used if `reject` cannot build the decorated
        // object: the message prefixed with the stable error tag.
        match outcome {
            Ok(Ok(json)) => Ok(json),
            Ok(Err(e)) => {
                let reason = format!("{}: {}", e.tag(), e);
                self.failure = Some(TaskFailure::Engine(e));
                Err(Error::new(Status::GenericFailure, reason))
            }
            Err(payload) => {
                let message = panic_message(payload.as_ref());
                let reason = format!("{INTERNAL_ERROR}: {message}");
                self.failure = Some(TaskFailure::Panic(message));
                Err(Error::new(Status::GenericFailure, reason))
            }
        }
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }

    fn reject(&mut self, env: Env, err: Error) -> Result<Self::JsValue> {
        let decorated = match self.failure.take() {
            Some(TaskFailure::Engine(dl)) => engine_error_value(&env, &dl, Some(&self.logic)),
            Some(TaskFailure::Panic(message)) => internal_error_value(&env, &message),
            None => None,
        };
        Err(decorated.unwrap_or(err))
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
        let json = datalogic_bind::args_json(args);

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
        datalogic_bind::parse_result(&self.name, &ret_str, arena)
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
        other => {
            serde_json::to_string(&other).map_err(|e| engine_error(env, &DlError::wrap(e), None))?
        }
    };
    EvaluationConfig::from_json_str(&json).map_err(|e| engine_error(env, &e, None))
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

/// Resolve a JS-supplied operation budget: an explicit number wins, then
/// the engine's configured `ops_budget`, then unbounded.
///
/// JS has one number type, so the budget arrives as an `f64`. Anything
/// that is not a whole number >= 1 is rejected rather than silently
/// truncated — a budget of `0.5` means the caller has confused this with
/// a duration or a fraction.
fn resolve_budget(env: &Env, engine: &Arc<RsEngine>, budget: Option<f64>) -> Result<u64> {
    datalogic_bind::resolve_budget(engine, budget)
        .map_err(|msg| engine_error(env, &datalogic_rs::Error::invalid_arguments(msg), None))
}

pub(crate) fn evaluate_metered(
    env: &Env,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: Value,
    budget: Option<f64>,
) -> Result<MeteredResult> {
    let budget = resolve_budget(env, engine, budget)?;
    let arena = Bump::new();
    // Same string fast path as `evaluate_str`: a JSON-string input parses
    // straight into the arena instead of through a `serde_json::Value`.
    let metered = match &data {
        Value::String(s) => engine.evaluate_metered(logic, s.as_str(), &arena, budget),
        other => engine.evaluate_metered(logic, other, &arena, budget),
    }
    .map_err(|e| engine_error(env, &e, Some(logic)))?;
    Ok(MeteredResult {
        result: metered.value.to_string(),
        ops: metered.ops as f64,
    })
}

pub(crate) fn evaluate_str(
    env: &Env,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: Value,
) -> Result<String> {
    // The metered body with no explicit budget resolves to exactly what
    // `Engine::evaluate` would apply, so this is that body minus the count.
    evaluate_metered(env, engine, logic, data, None).map(|m| m.result)
}
