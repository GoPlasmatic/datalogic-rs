use wasm_bindgen::prelude::*;
use datalogic_rs::DataLogic;

#[wasm_bindgen]
pub struct JsJsonLogic {
    inner: DataLogic,
}

#[wasm_bindgen]
impl JsJsonLogic {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: DataLogic::new(),
        }
    }

    #[wasm_bindgen]
    pub fn apply(&self, rules: &str, data: &str) -> JsValue {
        // First, validate the input strings
        if rules.is_empty() || data.is_empty() {
            return JsValue::from_str("Input strings cannot be empty");
        }

        // Evaluate the logic
        match self.inner.evaluate_str(rules, data, None) {
            Ok(v) => {
                // Convert the result to a JSON string
                match serde_json::to_string(&v) {
                    Ok(json_str) => JsValue::from_str(&json_str),
                    Err(e) => JsValue::from_str(&format!("JSON serialization error: {}", e))
                }
            }
            Err(e) => JsValue::from_str(&e.to_string())
        }
    }

    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.inner.reset_arena();
    }
} 