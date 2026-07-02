use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CustomOperator, DataValue, Engine as RsEngine, Error, EvaluationConfig, Logic,
    Result as DlResult,
};
use js_sys::{Function, Object, Reflect};
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
