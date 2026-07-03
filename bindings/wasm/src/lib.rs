use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CustomOperator, DataValue, Engine as RsEngine, Error, EvaluationConfig, Logic, ParsedData,
    Result as DlResult,
};
use js_sys::{Array, Function, Object, Reflect};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Build an [`RsEngine`] honoring the `templating` flag and an optional
/// [`EvaluationConfig`] override.
fn make_engine(templating: bool, config: Option<EvaluationConfig>) -> RsEngine {
    let mut builder = RsEngine::builder();
    if templating {
        builder = builder.with_templating(true);
    }
    if let Some(config) = config {
        builder = builder.with_config(config);
    }
    builder.build()
}

/// Serialize an `Error` (the merged structured form) for the JS boundary.
/// Falls back to the Display string if JSON serialisation somehow fails so
/// callers always receive *something* informative. Feeds the `detailJson`
/// property of the thrown JS `Error` (see [`build_js_error`]).
fn err_to_json(err: &Error) -> String {
    serde_json::to_string(err).unwrap_or_else(|_| err.to_string())
}

/// Wrap a parse-stage failure into the same `{ type: "ParseError", ... }`
/// JSON shape used for runtime errors. Used when the WASM boundary itself
/// fails to parse user input (logic JSON / data JSON / options) before the
/// engine ever runs.
fn input_err_to_json(stage: &str, message: impl std::fmt::Display) -> String {
    #[derive(Serialize)]
    struct Wire<'a> {
        #[serde(rename = "type")]
        kind: &'a str,
        message: String,
        stage: &'a str,
    }
    serde_json::to_string(&Wire {
        kind: "ParseError",
        message: message.to_string(),
        stage,
    })
    .unwrap_or_else(|_| message.to_string())
}

// =============== JS error bridge ===============

/// Build the JS `Error` object every export rejects with.
///
/// Shape (single source of truth, shared by every export):
/// - `name`: the stable error-kind tag ([`Error::tag`] values such as
///   `"ParseError"`, `"Thrown"`, `"ConfigurationError"`), so callers can
///   switch on `e.name`.
/// - `message`: the human-readable Display string, including the
///   `(in operator: ...)` suffix when the failing operator is known.
/// - own properties: every field of the structured error JSON (`type`,
///   variant extras like `variable` / `thrown` / `index` / `length`,
///   optional `operator` / `node_ids` / `stage`) attached via
///   `Reflect::set`.
/// - `detailJson`: the raw JSON string that releases up to 5.0.x used as
///   the rejection value, kept so existing consumers migrate with one
///   property access instead of restructuring their catch blocks.
fn build_js_error(name: &str, message: &str, detail_json: &str) -> JsValue {
    let error = js_sys::Error::new(message);
    error.set_name(name);
    // Copy the structured fields onto the Error as own properties. Parsing
    // our own serialisation cannot realistically fail; if it somehow does,
    // callers still get `name` / `message` / `detailJson`.
    if let Ok(parsed) = js_sys::JSON::parse(detail_json)
        && let Some(obj) = parsed.dyn_ref::<Object>()
    {
        let keys = Object::keys(obj);
        for i in 0..keys.length() {
            let key = keys.get(i);
            // The JSON `message` field is the kind-only form; the Error
            // constructor already set the fuller Display string. Skip it.
            if key.as_string().as_deref() == Some("message") {
                continue;
            }
            if let Ok(value) = Reflect::get(obj, &key) {
                let _ = Reflect::set(&error, &key, &value);
            }
        }
    }
    let _ = Reflect::set(
        &error,
        &JsValue::from_str("detailJson"),
        &JsValue::from_str(detail_json),
    );
    error.into()
}

/// Convert an engine [`Error`] into the thrown JS `Error`.
fn engine_err_to_js(err: &Error) -> JsValue {
    build_js_error(err.tag(), &err.to_string(), &err_to_json(err))
}

/// Convert a boundary input failure (bad logic / data / options before the
/// engine runs) into the thrown JS `Error`. Always named `ParseError`; the
/// `stage` property says which input was bad.
fn input_err_to_js(stage: &str, message: impl std::fmt::Display) -> JsValue {
    let message = message.to_string();
    build_js_error("ParseError", &message, &input_err_to_json(stage, &message))
}

/// JSON type name for `TypeMismatch` messages. Wording is copied from the
/// C ABI binding (`bindings/c/src/session.rs`) so every wrapper reports
/// the same thing for the same result.
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

/// Build the thrown JS `Error` for a typed-evaluation result of the wrong
/// type: `name` / `type` are `"TypeMismatch"`, mirroring the C ABI's
/// `DATALOGIC_STATUS_TYPE_MISMATCH` status + `"TypeMismatch"` tag.
fn type_mismatch_err_to_js(message: &str) -> JsValue {
    #[derive(Serialize)]
    struct Wire<'a> {
        #[serde(rename = "type")]
        kind: &'a str,
        message: &'a str,
    }
    let detail = serde_json::to_string(&Wire {
        kind: "TypeMismatch",
        message,
    })
    .unwrap_or_else(|_| message.to_string());
    build_js_error("TypeMismatch", message, &detail)
}

/// Decode an optional engine-config input. Accepts `undefined` / `null`
/// (no override), a JSON string, or a plain JS object (stringified via
/// `JSON.stringify`), then parses it with
/// [`EvaluationConfig::from_json_str`]. Unknown keys or values reject with
/// a `ConfigurationError`.
fn parse_config_value(value: &JsValue) -> Result<Option<EvaluationConfig>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let json = match value.as_string() {
        Some(s) => s,
        // `JSON.stringify` returns `undefined` (not a string) for
        // non-serialisable inputs such as bare functions; `.as_string()`
        // filters that case into the error arm below.
        None => js_sys::JSON::stringify(value)
            .ok()
            .and_then(|s| s.as_string())
            .ok_or_else(|| {
                input_err_to_js("parse-config", "config must be a JSON string or a plain object")
            })?,
    };
    EvaluationConfig::from_json_str(&json)
        .map(Some)
        .map_err(|e| engine_err_to_js(&e))
}

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Evaluate a JSONLogic expression against data.
///
/// # Arguments
/// * `logic` - JSON string containing the JSONLogic expression
/// * `data` - JSON string containing the data to evaluate against
/// * `templating` - If true, enables templating mode (multi-key objects compile to output-shaping templates with embedded JSONLogic)
///
/// # Returns
/// JSON string result.
///
/// # Throws
/// A real `Error` object on failure: `name` is the error-kind tag (for
/// example `"ParseError"`), `message` is human-readable, and the
/// structured fields (`type`, `operator`, `node_ids`, variant extras,
/// `detailJson`) ride along as own properties.
#[wasm_bindgen]
pub fn evaluate(logic: &str, data: &str, templating: bool) -> Result<String, JsValue> {
    make_engine(templating, None)
        .eval_str(logic, data)
        .map_err(|e| engine_err_to_js(&e))
}

/// Evaluate a JSONLogic expression with execution trace for debugging.
///
/// Returns a JSON string containing the result, expression tree, and execution
/// steps. Powered by [`RsEngine::trace`] +
/// [`datalogic_rs::TracedSession::eval_str`].
///
/// # Arguments
/// * `logic` - JSON string containing the JSONLogic expression
/// * `data` - JSON string containing the data to evaluate against
/// * `templating` - If true, enables templating mode (multi-key objects compile to output-shaping templates with embedded JSONLogic)
///
/// # Returns
/// JSON string of the form `{ result, steps, expression_tree, error? }`. On
/// runtime failure the `error` field carries the merged structured `Error`
/// JSON (`type`, `message`, variant extras, optional `operator`/`path`).
#[wasm_bindgen(js_name = evaluateWithTrace)]
pub fn evaluate_with_trace(logic: &str, data: &str, templating: bool) -> Result<String, JsValue> {
    let engine = make_engine(templating, None);
    let run = engine.trace().eval_str(logic, data);
    Ok(traced_run_to_json(&run))
}

/// Render a [`datalogic_rs::TracedRun`] into the JS wire shape. Mirrors the
/// historical `TracedResult` JSON layout: `{ result, expression_tree, steps,
/// error?, structured_error? }`.
fn traced_run_to_json(run: &datalogic_rs::TracedRun<String>) -> String {
    #[derive(Serialize)]
    struct Wire<'a> {
        result: serde_json::Value,
        expression_tree: &'a datalogic_rs::ExpressionNode,
        steps: &'a [datalogic_rs::ExecutionStep],
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_error: Option<&'a Error>,
    }

    let result_json: serde_json::Value;
    let mut error_msg: Option<String> = None;
    let mut error_struct: Option<&Error> = None;
    match &run.result {
        Ok(s) => {
            // The String is already JSON; surface it as the parsed value when
            // possible, falling back to a JSON string otherwise.
            result_json = serde_json::from_str::<serde_json::Value>(s.as_str())
                .unwrap_or_else(|_| serde_json::Value::String(s.to_string()));
        }
        Err(e) => {
            result_json = serde_json::Value::Null;
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

/// A compiled JSONLogic rule that can be evaluated multiple times.
///
/// Use this when you need to evaluate the same logic against different data,
/// as it avoids re-parsing the logic on each evaluation.
///
/// `CompiledRule` builds its own engine internally and therefore does **not**
/// support custom operators. For custom operators, use [`Engine`] +
/// [`Engine::compile`] instead.
#[wasm_bindgen]
pub struct CompiledRule {
    engine: RsEngine,
    compiled: Arc<Logic>,
}

#[wasm_bindgen]
impl CompiledRule {
    /// Create a new CompiledRule from a JSONLogic expression.
    ///
    /// # Arguments
    /// * `logic` - JSON string containing the JSONLogic expression
    /// * `templating` - If true, enables templating mode (multi-key objects compile to output-shaping templates with embedded JSONLogic)
    /// * `config` - Optional evaluation config: a JSON string or a plain
    ///   object with keys such as `preset` (`"default"` | `"safe_arithmetic"`
    ///   | `"strict"`), `division_by_zero`, `truthy_evaluator`,
    ///   `numeric_coercion`, `max_recursion_depth`. Omit (or pass
    ///   `undefined` / `null`) for default semantics.
    ///
    /// # Throws
    /// An `Error` named `ParseError` for malformed logic, or
    /// `ConfigurationError` for an invalid config.
    #[wasm_bindgen(constructor)]
    pub fn new(
        logic: &str,
        templating: bool,
        config: Option<JsValue>,
    ) -> Result<CompiledRule, JsValue> {
        let config = match &config {
            Some(value) => parse_config_value(value)?,
            None => None,
        };
        let engine = make_engine(templating, config);
        let compiled = engine.compile_arc(logic).map_err(|e| engine_err_to_js(&e))?;
        Ok(CompiledRule { engine, compiled })
    }

    /// Evaluate the compiled rule against data.
    ///
    /// # Arguments
    /// * `data` - JSON string containing the data to evaluate against
    ///
    /// # Returns
    /// JSON string result.
    ///
    /// # Throws
    /// An `Error` object carrying the structured fields (see [`evaluate`]).
    pub fn evaluate(&self, data: &str) -> Result<String, JsValue> {
        let arena = Bump::new();
        let data_dv = DataValue::from_str(data, &arena)
            .map_err(|e| input_err_to_js("parse-data", format!("{:?}", e)))?;
        let result = self
            .engine
            .evaluate(&self.compiled, data_dv, &arena)
            .map_err(|e| engine_err_to_js(&e))?;
        Ok(result.to_string())
    }

    /// Evaluate the compiled rule against a pre-parsed [`DataHandle`] —
    /// no data copy or parse per call; only the result JSON string
    /// crosses back to JS.
    ///
    /// # Returns
    /// JSON string result.
    ///
    /// # Throws
    /// An `Error` object carrying the structured fields (see [`evaluate`]).
    #[wasm_bindgen(js_name = evaluateData)]
    pub fn evaluate_data(&self, data: &DataHandle) -> Result<String, JsValue> {
        let arena = Bump::new();
        let result = self
            .engine
            .evaluate(&self.compiled, &*data.parsed, &arena)
            .map_err(|e| engine_err_to_js(&e))?;
        Ok(result.to_string())
    }
}

// =============== Custom operator bridge ===============

/// JS-backed custom operator. The host JS function receives a JSON-array
/// string of evaluated args and returns a JSON string of the result.
struct JsOperator {
    name: String,
    callback: Function,
}

// SAFETY: `wasm32-unknown-unknown` is single-threaded. The `CustomOperator:
// Send + Sync` bound exists for the native build's multi-threaded engine;
// in WASM there is no other thread that could observe the `js_sys::Function`.
// If WASM threads (atomics + shared memory) are ever enabled, this needs
// to be revisited — but `js-sys` itself relies on the same assumption.
unsafe impl Send for JsOperator {}
unsafe impl Sync for JsOperator {}

impl CustomOperator for JsOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> DlResult<&'a DataValue<'a>> {
        // 1. Serialize args as a JSON array. `DataValue::to_json_string`
        //    handles all leaf types; we just need to wrap with `[...]` and
        //    insert commas.
        let mut json = String::from("[");
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&a.to_json_string());
        }
        json.push(']');

        // 2. Invoke the JS callback synchronously. WASM JS calls are sync
        //    by definition.
        let js_args = JsValue::from_str(&json);
        let ret = self.callback.call1(&JsValue::NULL, &js_args).map_err(|e| {
            Error::custom_message(format!(
                "custom operator '{}' threw: {}",
                self.name,
                e.as_string().unwrap_or_else(|| format!("{:?}", e))
            ))
        })?;

        // 3. Decode the return value into a JSON string. `null`/`undefined`
        //    map to JSON null; anything else must be a string.
        let ret_str: String = if ret.is_null() || ret.is_undefined() {
            "null".to_string()
        } else if let Some(s) = ret.as_string() {
            s
        } else {
            return Err(Error::custom_message(format!(
                "custom operator '{}' must return a JSON string (or null/undefined for JSON null), got {:?}",
                self.name, ret
            )));
        };

        // 4. Parse the returned JSON into the eval arena so the borrowed
        //    `DataValue` stays valid for the rest of the evaluation.
        let arena_str = arena.alloc_str(&ret_str);
        let parsed = DataValue::from_str(arena_str, arena).map_err(|e| {
            Error::custom_message(format!(
                "custom operator '{}' returned invalid JSON: {}",
                self.name, e
            ))
        })?;
        Ok(arena.alloc(parsed))
    }
}

// =============== Engine class (custom-op capable) ===============

/// JSONLogic compile/evaluate engine with optional custom operators.
///
/// Construct with `new Engine(options)` where `options` is:
/// ```ts
/// {
///   templating?: boolean,
///   customOperators?: Record<string, (argsJson: string) => string>,
///   config?: string | object
/// }
/// ```
///
/// `config` tunes evaluation semantics. Pass either a JSON string or a
/// plain object; accepted keys (all optional): `preset` (`"default"` |
/// `"safe_arithmetic"` | `"strict"`), `arithmetic_nan_handling`,
/// `division_by_zero`, `loose_equality_errors`, `truthy_evaluator`,
/// `numeric_coercion`, `max_recursion_depth`. Unknown keys or values
/// reject with a `ConfigurationError`.
///
/// `customOperators` registers a JS function under each name. The function
/// receives the evaluated args as a JSON-array string (e.g. `"[1, 2, \"x\"]"`)
/// and **must return a JSON string** (e.g. `"\"variant_a\""`, `"42"`,
/// `"null"`). Returning `null`/`undefined` is treated as JSON `null`. A
/// thrown JS exception or non-string return becomes a runtime evaluation
/// error.
///
/// Custom operator names collide-and-lose with built-ins: registering `"+"`
/// has no effect because the built-in dispatches first.
#[wasm_bindgen]
pub struct Engine {
    inner: Arc<RsEngine>,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<Engine, JsValue> {
        let (templating, custom_ops, config) = parse_engine_options(&options)?;
        let mut builder = RsEngine::builder();
        if templating {
            builder = builder.with_templating(true);
        }
        if let Some(config) = config {
            builder = builder.with_config(config);
        }
        for (name, callback) in custom_ops {
            builder = builder.add_operator(name.clone(), JsOperator { name, callback });
        }
        Ok(Engine {
            inner: Arc::new(builder.build()),
        })
    }

    /// Compile a JSONLogic rule into a reusable [`Rule`].
    pub fn compile(&self, logic: &str) -> Result<Rule, JsValue> {
        let compiled = self
            .inner
            .compile_arc(logic)
            .map_err(|e| engine_err_to_js(&e))?;
        Ok(Rule {
            engine: self.inner.clone(),
            compiled,
        })
    }

    /// One-shot: compile `logic` and evaluate against `data` in a single
    /// call. Returns the result as a JSON string.
    #[wasm_bindgen(js_name = evalStr)]
    pub fn eval_str(&self, logic: &str, data: &str) -> Result<String, JsValue> {
        self.inner
            .eval_str(logic, data)
            .map_err(|e| engine_err_to_js(&e))
    }

    /// Open a [`Session`]: a reusable evaluation handle that owns a bump
    /// arena and resets it at the start of every `evaluate` call. This is
    /// the hot-loop tier: steady-state evaluation reuses the arena chunks
    /// instead of allocating a fresh arena per call like `Rule.evaluate`.
    pub fn session(&self) -> Session {
        Session {
            engine: self.inner.clone(),
            arena: Bump::new(),
        }
    }
}

/// A rule compiled against a specific [`Engine`] — preserves access to that
/// engine's custom operators.
#[wasm_bindgen]
pub struct Rule {
    engine: Arc<RsEngine>,
    compiled: Arc<Logic>,
}

#[wasm_bindgen]
impl Rule {
    /// Evaluate the compiled rule against `data` (a JSON string).
    pub fn evaluate(&self, data: &str) -> Result<String, JsValue> {
        let arena = Bump::new();
        let data_dv = DataValue::from_str(data, &arena)
            .map_err(|e| input_err_to_js("parse-data", format!("{:?}", e)))?;
        let result = self
            .engine
            .evaluate(&self.compiled, data_dv, &arena)
            .map_err(|e| engine_err_to_js(&e))?;
        Ok(result.to_string())
    }

    /// Evaluate the compiled rule against a pre-parsed [`DataHandle`] —
    /// no data copy or parse per call; only the result JSON string
    /// crosses back to JS.
    #[wasm_bindgen(js_name = evaluateData)]
    pub fn evaluate_data(&self, data: &DataHandle) -> Result<String, JsValue> {
        let arena = Bump::new();
        let result = self
            .engine
            .evaluate(&self.compiled, &*data.parsed, &arena)
            .map_err(|e| engine_err_to_js(&e))?;
        Ok(result.to_string())
    }

    /// Internal: deposit a cheap reference-counted duplicate (two `Arc`
    /// clones) into the batch stash. Called through the JS prototype by
    /// [`Session::evaluate_many`] to snapshot rules out of a JS array
    /// without consuming the caller's objects (wasm-bindgen's only
    /// supported `JsValue` → Rust conversion for exported classes takes
    /// ownership of the JS object). Not part of the public API.
    #[doc(hidden)]
    #[wasm_bindgen(js_name = "__dlrsStashRule", skip_typescript)]
    pub fn stash_for_batch(&self) {
        RULE_STASH.with(|stash| {
            *stash.borrow_mut() = Some(Rule {
                engine: self.engine.clone(),
                compiled: self.compiled.clone(),
            });
        });
    }
}

// =============== Session (arena reuse) ===============

/// Reusable evaluation handle that owns a bump arena: the hot-loop tier.
///
/// Created via `engine.session()`. Each `evaluate` call resets the arena
/// at the start (constant-time; the allocated chunks are retained), so a
/// tight loop reuses memory instead of allocating a fresh arena per call.
/// Results are returned as owned JSON strings, so they stay valid across
/// subsequent calls and resets.
///
/// Like everything in this module, a session is single-threaded: share it
/// across calls within one Worker, never across Workers.
#[wasm_bindgen]
pub struct Session {
    engine: Arc<RsEngine>,
    arena: Bump,
}

#[wasm_bindgen]
impl Session {
    /// Evaluate a compiled [`Rule`] against `data` (a JSON string),
    /// reusing this session's arena. The arena is reset at the start of
    /// each call, so the previous call's allocations never accumulate.
    ///
    /// # Returns
    /// JSON string result.
    ///
    /// # Throws
    /// An `Error` object carrying the structured fields (see [`evaluate`]).
    pub fn evaluate(&mut self, rule: &Rule, data: &str) -> Result<String, JsValue> {
        // Reset BEFORE each call so the previous iteration's allocations
        // don't pile up. The previous call's result was already
        // materialised as an owned JS string, so resetting here is safe.
        self.arena.reset();
        let data_dv = DataValue::from_str(data, &self.arena)
            .map_err(|e| input_err_to_js("parse-data", format!("{:?}", e)))?;
        let result = self
            .engine
            .evaluate(&rule.compiled, data_dv, &self.arena)
            .map_err(|e| engine_err_to_js(&e))?;
        Ok(result.to_string())
    }

    /// Evaluate a compiled [`Rule`] against a pre-parsed [`DataHandle`],
    /// reusing this session's arena — the hot path of the parse-once
    /// tier: zero data copy or parse per call; only the result JSON
    /// string crosses back to JS.
    ///
    /// # Returns
    /// JSON string result.
    ///
    /// # Throws
    /// An `Error` object carrying the structured fields (see [`evaluate`]).
    #[wasm_bindgen(js_name = evaluateData)]
    pub fn evaluate_data(&mut self, rule: &Rule, data: &DataHandle) -> Result<String, JsValue> {
        self.arena.reset();
        let result = self
            .engine
            .evaluate(&rule.compiled, &*data.parsed, &self.arena)
            .map_err(|e| engine_err_to_js(&e))?;
        Ok(result.to_string())
    }

    /// Evaluate and read the result as a strict JSON boolean. Any other
    /// result type throws an `Error` named `TypeMismatch`; for
    /// JSONLogic truthiness coercion use `evaluateTruthy`.
    ///
    /// Handle-input only, like every typed evaluation: the
    /// predicate-heavy flows that want typed results are exactly the
    /// flows that parse data once. No JSON serialization at all.
    #[wasm_bindgen(js_name = evaluateBool)]
    pub fn evaluate_bool(&mut self, rule: &Rule, data: &DataHandle) -> Result<bool, JsValue> {
        self.arena.reset();
        let av = self
            .engine
            .evaluate(&rule.compiled, &*data.parsed, &self.arena)
            .map_err(|e| engine_err_to_js(&e))?;
        av.as_bool().ok_or_else(|| {
            type_mismatch_err_to_js(&format!("result is not a boolean (got {})", type_of(av)))
        })
    }

    /// Evaluate and read the result as a number (JS has one number
    /// type; any JSON number is accepted). A non-number result throws
    /// an `Error` named `TypeMismatch`.
    #[wasm_bindgen(js_name = evaluateNumber)]
    pub fn evaluate_number(&mut self, rule: &Rule, data: &DataHandle) -> Result<f64, JsValue> {
        self.arena.reset();
        let av = self
            .engine
            .evaluate(&rule.compiled, &*data.parsed, &self.arena)
            .map_err(|e| engine_err_to_js(&e))?;
        av.as_f64().ok_or_else(|| {
            type_mismatch_err_to_js(&format!("result is not a number (got {})", type_of(av)))
        })
    }

    /// Evaluate and collapse the result to a boolean via the engine's
    /// configured truthiness rules (the same coercion `if` / `and` /
    /// `or` apply). Never type-mismatches — any result truthy-converts.
    #[wasm_bindgen(js_name = evaluateTruthy)]
    pub fn evaluate_truthy(&mut self, rule: &Rule, data: &DataHandle) -> Result<bool, JsValue> {
        self.arena.reset();
        let av = self
            .engine
            .evaluate(&rule.compiled, &*data.parsed, &self.arena)
            .map_err(|e| engine_err_to_js(&e))?;
        Ok(self.engine.truthy(av))
    }

    /// Evaluate one rule against many pre-parsed [`DataHandle`]s in a
    /// single boundary call — the bulk-scoring shape.
    ///
    /// Returns one `Promise.allSettled`-style plain object per input,
    /// in order:
    /// - `{ status: "fulfilled", value }` where `value` is the item's
    ///   result as a JSON string, or
    /// - `{ status: "rejected", reason }` where `reason` is
    ///   `{ tag, message, operator? }` (`tag` is the stable error-kind
    ///   tag, `operator` the outermost failing operator when known).
    ///
    /// Item failures are independent — a failing item (including a
    /// non-`DataHandle` element in `handles`) never fails the call or
    /// its neighbours. The call itself only throws for argument
    /// problems (e.g. `handles` not being an array).
    #[wasm_bindgen(
        js_name = evaluateBatch,
        unchecked_return_type = "({ status: \"fulfilled\"; value: string } | { status: \"rejected\"; reason: { tag: string; message: string; operator?: string } })[]"
    )]
    pub fn evaluate_batch(
        &mut self,
        rule: &Rule,
        #[wasm_bindgen(unchecked_param_type = "DataHandle[]")] handles: &JsValue,
    ) -> Result<Array, JsValue> {
        let handles = as_batch_array(handles, "evaluate-batch", "handles")?;
        // One JS string for the prototype lookup, hoisted out of the
        // per-item loop (marshalling it per element costs an encode +
        // allocation each time).
        let stash_method = JsValue::from_str(DATA_STASH_METHOD);
        let mut outcomes: Vec<BatchOutcome> = Vec::with_capacity(handles.length() as usize);
        for i in 0..handles.length() {
            let outcome = match stash_element(
                &DATA_STASH,
                &handles.get(i),
                &stash_method,
                "handles",
                "DataHandle",
                i,
            ) {
                Ok(handle) => {
                    self.arena.reset();
                    match self
                        .engine
                        .evaluate(&rule.compiled, &*handle.parsed, &self.arena)
                    {
                        Ok(av) => BatchOutcome::Fulfilled {
                            value: av.to_string(),
                        },
                        Err(e) => BatchOutcome::Rejected {
                            reason: ItemError::from_engine(&e),
                        },
                    }
                }
                Err(reason) => BatchOutcome::Rejected { reason },
            };
            outcomes.push(outcome);
        }
        outcomes_to_js(&outcomes)
    }

    /// Evaluate many rules against one pre-parsed [`DataHandle`] in a
    /// single boundary call — the rule-set / feature-flag shape.
    ///
    /// `rules` must be an array of [`Rule`]s from `engine.compile(...)`
    /// (not standalone `CompiledRule`s). Same per-item
    /// `Promise.allSettled`-style outcome objects, independence
    /// guarantees, and call-level error contract as `evaluateBatch`.
    #[wasm_bindgen(
        js_name = evaluateMany,
        unchecked_return_type = "({ status: \"fulfilled\"; value: string } | { status: \"rejected\"; reason: { tag: string; message: string; operator?: string } })[]"
    )]
    pub fn evaluate_many(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "Rule[]")] rules: &JsValue,
        data: &DataHandle,
    ) -> Result<Array, JsValue> {
        let rules = as_batch_array(rules, "evaluate-many", "rules")?;
        // See evaluateBatch: hoisted prototype-lookup key.
        let stash_method = JsValue::from_str(RULE_STASH_METHOD);
        let mut outcomes: Vec<BatchOutcome> = Vec::with_capacity(rules.length() as usize);
        for i in 0..rules.length() {
            let outcome = match stash_element(
                &RULE_STASH,
                &rules.get(i),
                &stash_method,
                "rules",
                "Rule",
                i,
            ) {
                Ok(rule) => {
                    self.arena.reset();
                    match self
                        .engine
                        .evaluate(&rule.compiled, &*data.parsed, &self.arena)
                    {
                        Ok(av) => BatchOutcome::Fulfilled {
                            value: av.to_string(),
                        },
                        Err(e) => BatchOutcome::Rejected {
                            reason: ItemError::from_engine(&e),
                        },
                    }
                }
                Err(reason) => BatchOutcome::Rejected { reason },
            };
            outcomes.push(outcome);
        }
        outcomes_to_js(&outcomes)
    }

    /// Reset the underlying arena, returning every chunk to its start
    /// position without freeing OS memory. Calling this is optional:
    /// `evaluate` resets at the start of each call.
    pub fn reset(&mut self) {
        self.arena.reset();
    }

    /// Bytes currently held by the session's arena chunks. Useful for
    /// sizing or diagnostics.
    #[wasm_bindgen(js_name = allocatedBytes)]
    pub fn allocated_bytes(&self) -> usize {
        self.arena.allocated_bytes()
    }
}

// =============== DataHandle (parse-once data) ===============

/// An immutable, pre-parsed JSON document resident in WASM linear
/// memory — the parse-once tier every other binding ships (ABI v2).
///
/// Every string-taking evaluation copies the data JSON across the
/// JS↔WASM boundary and re-parses it inside the module on each call; on
/// kilobyte payloads that copy + parse dominates the round trip. A
/// `DataHandle` pays it once: construct the handle from a JSON string,
/// then evaluate any number of rules against the resident tree — per
/// call only the rule dispatch and the (usually small) result cross the
/// boundary.
///
/// Handles are immutable, never consumed by evaluation, and independent
/// of any [`Engine`]: one handle can feed rules and sessions of
/// different engines, as long as everything lives in the same module
/// instance (WASM modules are isolated per Worker, like everything
/// else here). Call `free()` after the last evaluation to release the
/// linear memory eagerly; if you don't, the generated
/// `FinalizationRegistry` glue reclaims it when the JS object is
/// collected (best-effort), the same as every other class in this
/// package.
#[wasm_bindgen]
pub struct DataHandle {
    /// `Rc`, not `Arc`: `wasm32-unknown-unknown` is single-threaded and
    /// `ParsedData` is `!Sync`. The `Rc` is what makes the hidden
    /// `__dlrsDataDup` duplication O(1) for the batch entry points.
    parsed: Rc<ParsedData>,
}

impl std::fmt::Debug for DataHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DataHandle").field(&self.parsed).finish()
    }
}

#[wasm_bindgen]
impl DataHandle {
    /// Parse `json` into a resident document.
    ///
    /// # Throws
    /// An `Error` named `ParseError` on malformed JSON, carrying the
    /// same structured fields as every other error in this binding.
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> Result<DataHandle, JsValue> {
        ParsedData::from_json(json)
            .map(|parsed| DataHandle {
                parsed: Rc::new(parsed),
            })
            .map_err(|e| engine_err_to_js(&e))
    }

    /// Bytes held by the handle's backing arena (input copy + parsed
    /// tree). Useful for sizing and diagnostics.
    #[wasm_bindgen(getter, js_name = allocatedBytes)]
    pub fn allocated_bytes(&self) -> usize {
        self.parsed.allocated_bytes()
    }

    /// Internal: deposit a cheap reference-counted duplicate (one `Rc`
    /// clone) into the batch stash. Called through the JS prototype by
    /// [`Session::evaluate_batch`] to snapshot handles out of a JS
    /// array without consuming the caller's objects. Not part of the
    /// public API.
    #[doc(hidden)]
    #[wasm_bindgen(js_name = "__dlrsStashData", skip_typescript)]
    pub fn stash_for_batch(&self) {
        DATA_STASH.with(|stash| {
            *stash.borrow_mut() = Some(DataHandle {
                parsed: Rc::clone(&self.parsed),
            });
        });
    }
}

// =============== batch plumbing ===============

/// Hidden prototype-method names the batch extractors call — see
/// [`Rule::stash_for_batch`] / [`DataHandle::stash_for_batch`].
const RULE_STASH_METHOD: &str = "__dlrsStashRule";
const DATA_STASH_METHOD: &str = "__dlrsStashData";

thread_local! {
    /// Single-slot transfer stashes for the batch extractors.
    /// `wasm32-unknown-unknown` is single-threaded and each slot is
    /// cleared, filled, and drained within one `stash_element` call, so
    /// a slot never carries state across items (or across re-entrant
    /// engine callbacks — extraction completes before an item runs).
    static RULE_STASH: RefCell<Option<Rule>> = const { RefCell::new(None) };
    static DATA_STASH: RefCell<Option<DataHandle>> = const { RefCell::new(None) };
}

/// Per-item failure inside a batch result: the `reason` of a rejected
/// entry. Field set mirrors the C ABI's item-error JSON
/// (`{tag, message, operator?}`) exactly, so batch consumers see the
/// same shape in every binding.
#[derive(Serialize)]
struct ItemError {
    tag: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    operator: Option<String>,
}

impl ItemError {
    fn from_engine(err: &Error) -> Self {
        Self {
            tag: err.tag().to_string(),
            message: err.to_string(),
            operator: err.operator().map(str::to_owned),
        }
    }

    fn invalid_argument(message: String) -> Self {
        Self {
            tag: "InvalidArgument".to_string(),
            message,
            operator: None,
        }
    }
}

/// One entry of a batch result, in `Promise.allSettled` shape.
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum BatchOutcome {
    Fulfilled { value: String },
    Rejected { reason: ItemError },
}

/// Decode the array argument of a batch entry point (call-level check:
/// a non-array rejects the whole call, unlike per-item problems).
fn as_batch_array<'v>(value: &'v JsValue, stage: &str, arg: &str) -> Result<&'v Array, JsValue> {
    value
        .dyn_ref::<Array>()
        .ok_or_else(|| input_err_to_js(stage, format!("{arg} must be an Array")))
}

/// Snapshot one element of a batch input array as an owned `T`.
///
/// wasm-bindgen's only supported `JsValue` → Rust conversion for
/// exported classes (`TryFromJsValue`) consumes the JS object, which
/// would destroy the caller's rules/handles on first use. Instead each
/// class exposes a hidden prototype method that deposits a cheap
/// reference-counted duplicate into a single-slot stash; calling it
/// through the boundary and draining the slot borrows the element
/// without a temporary JS wrapper (no allocation, no
/// `FinalizationRegistry` churn). Costs two small JS calls per element
/// — the boundary benchmarks fold that into the batch numbers.
///
/// Failures (missing method, non-instance element, freed object) are
/// per-item errors, never call failures.
fn stash_element<T>(
    stash: &'static std::thread::LocalKey<RefCell<Option<T>>>,
    elem: &JsValue,
    stash_method: &JsValue,
    arg: &str,
    class: &str,
    index: u32,
) -> Result<T, ItemError> {
    let not_a = || ItemError::invalid_argument(format!("{arg}[{index}] is not a {class}"));
    // Drop anything stale (e.g. a caller invoking the hidden method by
    // hand) so a leftover value can never masquerade as this element.
    stash.with(|slot| slot.borrow_mut().take());
    let stash_fn: Function = Reflect::get(elem, stash_method)
        .ok()
        .and_then(|v| v.dyn_into::<Function>().ok())
        .ok_or_else(not_a)?;
    // The call throws for a freed (`.free()`d) object; treat that as an
    // invalid element too.
    stash_fn.call0(elem).map_err(|_| not_a())?;
    stash.with(|slot| slot.borrow_mut().take()).ok_or_else(not_a)
}

/// Materialise batch outcomes as one JS array via a single JSON round
/// trip: serialize the envelope in Rust, `JSON.parse` it once on the JS
/// side. One boundary crossing regardless of item count, and the item
/// `value`s stay JSON *strings* (the cheap direction), matching the
/// scalar entry points' contract.
fn outcomes_to_js(outcomes: &[BatchOutcome]) -> Result<Array, JsValue> {
    let json =
        serde_json::to_string(outcomes).map_err(|e| input_err_to_js("serialize-batch", e))?;
    js_sys::JSON::parse(&json)?
        .dyn_into::<Array>()
        .map_err(|_| input_err_to_js("serialize-batch", "batch envelope did not parse to an array"))
}

// =============== options-bag parsing ===============

/// Pull `{ templating, customOperators, config }` out of a JS options
/// object. Anything missing falls back to the zero value (no templating,
/// no ops, default config).
#[allow(clippy::type_complexity)]
fn parse_engine_options(
    options: &JsValue,
) -> Result<(bool, Vec<(String, Function)>, Option<EvaluationConfig>), JsValue> {
    if options.is_null() || options.is_undefined() {
        return Ok((false, Vec::new(), None));
    }
    let obj: &Object = options
        .dyn_ref::<Object>()
        .ok_or_else(|| input_err_to_js("parse-options", "options must be an object"))?;

    let templating = match Reflect::get(obj, &JsValue::from_str("templating")) {
        Ok(v) if v.is_undefined() || v.is_null() => false,
        Ok(v) => v.as_bool().ok_or_else(|| {
            input_err_to_js("parse-options", "options.templating must be a boolean")
        })?,
        Err(_) => false,
    };

    let custom_ops = match Reflect::get(obj, &JsValue::from_str("customOperators")) {
        Ok(v) if v.is_undefined() || v.is_null() => Vec::new(),
        Ok(v) => parse_custom_operators(&v)?,
        Err(_) => Vec::new(),
    };

    let config = match Reflect::get(obj, &JsValue::from_str("config")) {
        Ok(v) => parse_config_value(&v)?,
        Err(_) => None,
    };

    Ok((templating, custom_ops, config))
}

fn parse_custom_operators(v: &JsValue) -> Result<Vec<(String, Function)>, JsValue> {
    let obj: &Object = v.dyn_ref::<Object>().ok_or_else(|| {
        input_err_to_js(
            "parse-options",
            "options.customOperators must be an object of {name: function}",
        )
    })?;

    let keys = Object::keys(obj);
    let mut out = Vec::with_capacity(keys.length() as usize);
    for i in 0..keys.length() {
        let key = keys.get(i);
        let name = key.as_string().ok_or_else(|| {
            input_err_to_js("parse-options", "customOperators keys must be strings")
        })?;
        let value = Reflect::get(obj, &key).map_err(|e| {
            input_err_to_js(
                "parse-options",
                format!("failed to read customOperators['{}']: {:?}", name, e),
            )
        })?;
        let function = value.dyn_into::<Function>().map_err(|_| {
            input_err_to_js(
                "parse-options",
                format!("customOperators['{}'] must be a function", name),
            )
        })?;
        out.push((name, function));
    }
    Ok(out)
}
