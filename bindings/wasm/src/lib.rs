use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CustomOperator, DataValue, Engine as RsEngine, Error, Logic, Result as DlResult,
};
use js_sys::{Function, Object, Reflect};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Build an [`RsEngine`] honoring the `templating` flag.
fn make_engine(templating: bool) -> RsEngine {
    if templating {
        RsEngine::builder().with_templating(true).build()
    } else {
        RsEngine::new()
    }
}

/// Serialize an `Error` (the merged structured form) for the JS boundary.
/// Falls back to the Display string if JSON serialisation somehow fails so
/// callers always receive *something* informative.
fn err_to_json(err: &Error) -> String {
    serde_json::to_string(err).unwrap_or_else(|_| err.to_string())
}

/// Wrap a parse-stage failure into the same `{ type: "ParseError", ... }`
/// JSON shape used for runtime errors. Used when the WASM boundary itself
/// fails to parse user input (logic JSON / data JSON) before the engine ever
/// runs.
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
/// JSON string result, or the merged structured `Error` JSON on failure.
#[wasm_bindgen]
pub fn evaluate(logic: &str, data: &str, templating: bool) -> Result<String, String> {
    make_engine(templating)
        .eval_str(logic, data)
        .map_err(|e| err_to_json(&e))
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
pub fn evaluate_with_trace(logic: &str, data: &str, templating: bool) -> Result<String, String> {
    let engine = make_engine(templating);
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
    #[wasm_bindgen(constructor)]
    pub fn new(logic: &str, templating: bool) -> Result<CompiledRule, String> {
        let engine = make_engine(templating);
        let compiled = engine.compile_arc(logic).map_err(|e| err_to_json(&e))?;
        Ok(CompiledRule { engine, compiled })
    }

    /// Evaluate the compiled rule against data.
    ///
    /// # Arguments
    /// * `data` - JSON string containing the data to evaluate against
    ///
    /// # Returns
    /// JSON string result or merged structured `Error` JSON on failure.
    pub fn evaluate(&self, data: &str) -> Result<String, String> {
        let arena = Bump::new();
        let data_dv = DataValue::from_str(data, &arena)
            .map_err(|e| input_err_to_json("parse-data", format!("{:?}", e)))?;
        let result = self
            .engine
            .evaluate(&self.compiled, data_dv, &arena)
            .map_err(|e| err_to_json(&e))?;
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
///   customOperators?: Record<string, (argsJson: string) => string>
/// }
/// ```
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
    pub fn new(options: JsValue) -> Result<Engine, String> {
        let (templating, custom_ops) = parse_engine_options(&options)?;
        let mut builder = RsEngine::builder();
        if templating {
            builder = builder.with_templating(true);
        }
        for (name, callback) in custom_ops {
            builder = builder.add_operator(name.clone(), JsOperator { name, callback });
        }
        Ok(Engine {
            inner: Arc::new(builder.build()),
        })
    }

    /// Compile a JSONLogic rule into a reusable [`Rule`].
    pub fn compile(&self, logic: &str) -> Result<Rule, String> {
        let compiled = self.inner.compile_arc(logic).map_err(|e| err_to_json(&e))?;
        Ok(Rule {
            engine: self.inner.clone(),
            compiled,
        })
    }

    /// One-shot: compile `logic` and evaluate against `data` in a single
    /// call. Returns the result as a JSON string.
    #[wasm_bindgen(js_name = evalStr)]
    pub fn eval_str(&self, logic: &str, data: &str) -> Result<String, String> {
        self.inner
            .eval_str(logic, data)
            .map_err(|e| err_to_json(&e))
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
    pub fn evaluate(&self, data: &str) -> Result<String, String> {
        let arena = Bump::new();
        let data_dv = DataValue::from_str(data, &arena)
            .map_err(|e| input_err_to_json("parse-data", format!("{:?}", e)))?;
        let result = self
            .engine
            .evaluate(&self.compiled, data_dv, &arena)
            .map_err(|e| err_to_json(&e))?;
        Ok(result.to_string())
    }
}

// =============== options-bag parsing ===============

/// Pull `{ templating, customOperators }` out of a JS options object.
/// Anything missing falls back to the zero value (no templating, no ops).
fn parse_engine_options(options: &JsValue) -> Result<(bool, Vec<(String, Function)>), String> {
    if options.is_null() || options.is_undefined() {
        return Ok((false, Vec::new()));
    }
    let obj: &Object = options
        .dyn_ref::<Object>()
        .ok_or_else(|| input_err_to_json("parse-options", "options must be an object"))?;

    let templating = match Reflect::get(obj, &JsValue::from_str("templating")) {
        Ok(v) if v.is_undefined() || v.is_null() => false,
        Ok(v) => v.as_bool().ok_or_else(|| {
            input_err_to_json("parse-options", "options.templating must be a boolean")
        })?,
        Err(_) => false,
    };

    let custom_ops = match Reflect::get(obj, &JsValue::from_str("customOperators")) {
        Ok(v) if v.is_undefined() || v.is_null() => Vec::new(),
        Ok(v) => parse_custom_operators(&v)?,
        Err(_) => Vec::new(),
    };

    Ok((templating, custom_ops))
}

fn parse_custom_operators(v: &JsValue) -> Result<Vec<(String, Function)>, String> {
    let obj: &Object = v.dyn_ref::<Object>().ok_or_else(|| {
        input_err_to_json(
            "parse-options",
            "options.customOperators must be an object of {name: function}",
        )
    })?;

    let keys = Object::keys(obj);
    let mut out = Vec::with_capacity(keys.length() as usize);
    for i in 0..keys.length() {
        let key = keys.get(i);
        let name = key.as_string().ok_or_else(|| {
            input_err_to_json("parse-options", "customOperators keys must be strings")
        })?;
        let value = Reflect::get(obj, &key).map_err(|e| {
            input_err_to_json(
                "parse-options",
                format!("failed to read customOperators['{}']: {:?}", name, e),
            )
        })?;
        let function = value.dyn_into::<Function>().map_err(|_| {
            input_err_to_json(
                "parse-options",
                format!("customOperators['{}'] must be a function", name),
            )
        })?;
        out.push((name, function));
    }
    Ok(out)
}
