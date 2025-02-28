use wasm_bindgen::prelude::*;
use crate::{JsonLogic, Rule};
use serde_json::Value;

#[wasm_bindgen]
pub struct JsJsonLogic(JsonLogic);

#[wasm_bindgen]
impl JsJsonLogic {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        JsJsonLogic(JsonLogic::new())
    }

    #[wasm_bindgen]
    pub fn apply(&self, logic: JsValue, data: JsValue) -> Result<JsValue, JsError> {
        let logic_value: Value = serde_wasm_bindgen::from_value(logic)?;
        let data_value: Value = serde_wasm_bindgen::from_value(data)?;
        
        // Convert Value to Rule
        let rule: Rule = Rule::from_value(&logic_value)
            .map_err(|e| JsError::new(&e.to_string()))?;
        
        let result = JsonLogic::apply(&rule, &data_value)
            .map_err(|e| JsError::new(&e.to_string()))?;
            
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }
}