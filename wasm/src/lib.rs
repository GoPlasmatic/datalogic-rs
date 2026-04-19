use std::sync::Arc;

use datalogic_rs::{CompiledLogic, DataLogic, StructuredError};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Build a `DataLogic` engine honoring the `preserve_structure` flag.
fn make_engine(preserve_structure: bool) -> DataLogic {
    if preserve_structure {
        DataLogic::with_preserve_structure()
    } else {
        DataLogic::new()
    }
}

/// Serialize a `StructuredError` to a JSON string for the JS boundary.
/// If serialization somehow fails, fall back to the Display string so the
/// caller always receives *something* informative.
fn structured_err_to_json(err: &StructuredError) -> String {
    serde_json::to_string(err).unwrap_or_else(|_| err.to_string())
}

/// Wrap any serializable error payload into `{ "type": "ParseError", ... }`
/// JSON. Used for parse/compile failures at the WASM surface so the error
/// shape is consistent with runtime errors from the engine.
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
/// * `preserve_structure` - If true, preserves object structure for JSON templates with embedded JSONLogic
///
/// # Returns
/// JSON string result or error message
#[wasm_bindgen]
pub fn evaluate(logic: &str, data: &str, preserve_structure: bool) -> Result<String, String> {
    make_engine(preserve_structure)
        .evaluate_json(logic, data)
        .map(|v| v.to_string())
        .map_err(|e| e.to_string())
}

/// Evaluate a JSONLogic expression with execution trace for debugging.
///
/// Returns a JSON string containing the result, expression tree, and execution steps.
/// This enables step-by-step debugging and visualization of the evaluation process.
///
/// # Arguments
/// * `logic` - JSON string containing the JSONLogic expression
/// * `data` - JSON string containing the data to evaluate against
/// * `preserve_structure` - If true, preserves object structure for JSON templates with embedded JSONLogic
///
/// # Returns
/// JSON string containing TracedResult (result, expression_tree, steps) or error message
///
/// # Example Output
/// ```json
/// {
///   "result": true,
///   "expression_tree": {
///     "id": 0,
///     "expression": "{\"and\": [...]}",
///     "children": [...]
///   },
///   "steps": [
///     {"id": 0, "node_id": 2, "context": {...}, "result": 25, "error": null},
///     ...
///   ]
/// }
/// ```
#[wasm_bindgen]
pub fn evaluate_with_trace(
    logic: &str,
    data: &str,
    preserve_structure: bool,
) -> Result<String, String> {
    make_engine(preserve_structure)
        .evaluate_json_with_trace(logic, data)
        .map(|traced_result| serde_json::to_string(&traced_result).unwrap_or_default())
        .map_err(|e| e.to_string())
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
    /// * `preserve_structure` - If true, preserves object structure for JSON templates with embedded JSONLogic
    #[wasm_bindgen(constructor)]
    pub fn new(logic: &str, preserve_structure: bool) -> Result<CompiledRule, String> {
        let engine = make_engine(preserve_structure);
        let parsed: serde_json::Value = serde_json::from_str(logic).map_err(|e| e.to_string())?;
        let compiled = engine.compile(&parsed).map_err(|e| e.to_string())?;
        Ok(CompiledRule { engine, compiled })
    }

    /// Evaluate the compiled rule against data.
    ///
    /// # Arguments
    /// * `data` - JSON string containing the data to evaluate against
    ///
    /// # Returns
    /// JSON string result or error message
    pub fn evaluate(&self, data: &str) -> Result<String, String> {
        let data: serde_json::Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
        self.engine
            .evaluate_owned(&self.compiled, data)
            .map(|v| v.to_string())
            .map_err(|e| e.to_string())
    }

    /// Evaluate the compiled rule and return a structured error on failure.
    ///
    /// On success, returns the JSON-encoded result (same shape as
    /// [`evaluate`](Self::evaluate)). On failure, the error string is a
    /// JSON document with `type`, `message`, variant-specific extras
    /// (e.g. `thrown`, `index`, `length`), and an optional `operator`
    /// field naming the outermost operator.
    #[wasm_bindgen(js_name = evaluateStructured)]
    pub fn evaluate_structured(&self, data: &str) -> Result<String, String> {
        let data: serde_json::Value = serde_json::from_str(data)
            .map_err(|e| input_err_to_json("parse-data", e))?;
        let data_arc = Arc::new(data);
        self.engine
            .evaluate_structured(&self.compiled, data_arc)
            .map(|v| v.to_string())
            .map_err(|e| structured_err_to_json(&e))
    }
}

/// Evaluate a JSONLogic expression with structured errors on failure.
///
/// Behaves like [`evaluate`] on success. On error, returns a JSON-encoded
/// [`StructuredError`] with `type`, `message`, variant-specific extras, and
/// an optional `operator` field. Callers can `JSON.parse` the error string
/// and switch on `err.type`.
#[wasm_bindgen(js_name = evaluateStructured)]
pub fn evaluate_structured(
    logic: &str,
    data: &str,
    preserve_structure: bool,
) -> Result<String, String> {
    make_engine(preserve_structure)
        .evaluate_json_structured(logic, data)
        .map(|v| v.to_string())
        .map_err(|e| structured_err_to_json(&e))
}

/// Evaluate a JSONLogic expression with execution trace and structured errors.
///
/// Returns the same `TracedResult` JSON as [`evaluate_with_trace`], with an
/// additional `error_structured` field populated when the rule errored at
/// runtime. Setup failures (invalid logic/data JSON, compile errors) are
/// returned via the `Err` channel as a JSON `StructuredError`.
#[wasm_bindgen(js_name = evaluateWithTraceStructured)]
pub fn evaluate_with_trace_structured(
    logic: &str,
    data: &str,
    preserve_structure: bool,
) -> Result<String, String> {
    make_engine(preserve_structure)
        .evaluate_json_with_trace_structured(logic, data)
        .map(|traced| serde_json::to_string(&traced).unwrap_or_default())
        .map_err(|e| structured_err_to_json(&e))
}
