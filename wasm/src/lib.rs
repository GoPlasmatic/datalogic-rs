use std::sync::Arc;

use datalogic_rs::{CompiledLogic, DataLogic};
use wasm_bindgen::prelude::*;

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
    let engine = if preserve_structure {
        DataLogic::with_preserve_structure()
    } else {
        DataLogic::new()
    };
    engine
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
    let engine = if preserve_structure {
        DataLogic::with_preserve_structure()
    } else {
        DataLogic::new()
    };
    engine
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
        let engine = if preserve_structure {
            DataLogic::with_preserve_structure()
        } else {
            DataLogic::new()
        };
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
}
