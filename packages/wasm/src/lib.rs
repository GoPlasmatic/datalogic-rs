use std::sync::Arc;

use datalogic_rs::{CompiledLogic, DataLogic, DataValue, Error};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Build a `DataLogic` engine honoring the `templating` flag.
fn make_engine(templating: bool) -> DataLogic {
    if templating {
        DataLogic::builder().with_templating(true).build()
    } else {
        DataLogic::new()
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
        .evaluate_str(logic, data)
        .map_err(|e| err_to_json(&e))
}

/// Evaluate a JSONLogic expression with execution trace for debugging.
///
/// Returns a JSON string containing the result, expression tree, and execution
/// steps. Powered by [`DataLogic::trace`] +
/// [`datalogic_rs::TracedSession::evaluate_str`].
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
#[wasm_bindgen]
pub fn evaluate_with_trace(logic: &str, data: &str, templating: bool) -> Result<String, String> {
    let engine = make_engine(templating);
    let run = engine.trace().evaluate_str(logic, data);
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
#[wasm_bindgen]
pub struct CompiledRule {
    engine: DataLogic,
    compiled: Arc<CompiledLogic>,
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
        let compiled = engine.compile(logic).map_err(|e| err_to_json(&e))?;
        Ok(CompiledRule {
            engine,
            compiled: Arc::new(compiled),
        })
    }

    /// Evaluate the compiled rule against data.
    ///
    /// # Arguments
    /// * `data` - JSON string containing the data to evaluate against
    ///
    /// # Returns
    /// JSON string result or merged structured `Error` JSON on failure.
    pub fn evaluate(&self, data: &str) -> Result<String, String> {
        let arena = bumpalo::Bump::new();
        let data_dv = DataValue::from_str(data, &arena)
            .map_err(|e| input_err_to_json("parse-data", format!("{:?}", e)))?;
        let result = self
            .engine
            .evaluate(&*self.compiled, data_dv, &arena)
            .map_err(|e| err_to_json(&e))?;
        Ok(datalogic_rs::arena::data_to_json_string(result))
    }

    /// Backwards-compatible alias for [`Self::evaluate`]. Pre-merge the
    /// "structured" variant returned a richer error shape; today every error
    /// already carries the merged structured form, so the two paths are
    /// identical.
    #[wasm_bindgen(js_name = evaluateStructured)]
    pub fn evaluate_structured(&self, data: &str) -> Result<String, String> {
        self.evaluate(data)
    }
}

/// Evaluate a JSONLogic expression with structured errors on failure.
///
/// Behaves like [`evaluate`] today — the merged `Error` shape always carries
/// `type`, `message`, variant extras, and (when populated) `operator` /
/// `path`. The function is kept as a separate JS export for back-compat with
/// callers binding `evaluateStructured`.
#[wasm_bindgen(js_name = evaluateStructured)]
pub fn evaluate_structured(logic: &str, data: &str, templating: bool) -> Result<String, String> {
    evaluate(logic, data, templating)
}

/// Evaluate a JSONLogic expression with execution trace and structured errors.
///
/// Today the trace path always returns the merged structured-error shape via
/// `structured_error` on failure, so this is an alias for
/// [`evaluate_with_trace`]. Kept for back-compat with the JS binding name.
#[wasm_bindgen(js_name = evaluateWithTraceStructured)]
pub fn evaluate_with_trace_structured(
    logic: &str,
    data: &str,
    templating: bool,
) -> Result<String, String> {
    evaluate_with_trace(logic, data, templating)
}
