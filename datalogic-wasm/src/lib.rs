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
///
/// # Returns
/// JSON string result or error message
#[wasm_bindgen]
pub fn evaluate(logic: &str, data: &str) -> Result<String, String> {
    let engine = DataLogic::new();
    engine
        .evaluate_json(logic, data)
        .map(|v| v.to_string())
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
    #[wasm_bindgen(constructor)]
    pub fn new(logic: &str) -> Result<CompiledRule, String> {
        let engine = DataLogic::new();
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
