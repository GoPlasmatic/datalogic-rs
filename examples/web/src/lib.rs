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
    pub fn new_with_preserve_structure(preserve_structure: bool) -> Self {
        Self {
            inner: if preserve_structure {
                DataLogic::with_preserve_structure()
            } else {
                DataLogic::new()
            },
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
                // The value is already properly formatted, just pass it through
                JsValue::from_str(&v.to_string())
            }
            Err(e) => JsValue::from_str(&format!("Evaluation error: {}", e))
        }
    }

    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.inner.reset_arena();
    }
}