use wasm_bindgen::prelude::*;
use crate::{JsonLogic, Rule};
use serde_json::Value;
use js_sys;

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
        
        // Convert the result to a JSON string
        let result_string = serde_json::to_string(&result)
            .map_err(|e| JsError::new(&e.to_string()))?;
        
        // Use JavaScript's JSON.parse to ensure proper object structure
        let parse_fn = js_sys::Function::new_with_args(
            "jsonString",
            "return JSON.parse(jsonString);"
        );
        
        let js_result = parse_fn.call1(&JsValue::NULL, &JsValue::from_str(&result_string))
            .map_err(|e| JsError::new(&format!("Failed to parse JSON: {:?}", e)))?;
            
        Ok(js_result)
    }
}