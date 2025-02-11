use serde_json::{json, Value};
use crate::{JsonLogicResult, rule::ArgType, error::Error};

pub struct TryOperator;

impl TryOperator {
    pub fn apply(&self, args: &ArgType, data: &Value) -> JsonLogicResult {
        match args {
            ArgType::Multiple(rules) => {
                let mut last_error = None;
                
                for rule in rules {
                    let current_data = if let Some(err) = &last_error {
                        json!({ "type": err })
                    } else {
                        data.clone()
                    };
    
                    match rule.apply(&current_data) {
                        Ok(value) => return Ok(value),
                        Err(e) => {
                            let clean_error = self.normalize_error(&e);
                            last_error = Some(clean_error);
                            continue;
                        }
                    }
                }
                
                // If we get here, all rules failed
                if let Some(last_err) = last_error {
                    Err(Error::CustomError(last_err))
                } else {
                    Err(Error::CustomError("No valid value found".to_string()))
                }
            },
            ArgType::Unary(rule) => rule.apply(data)
        }
    }

    fn normalize_error(&self, error: &Error) -> String {
        let error_str = error.to_string();
        
        // If the error string looks like a JSON object
        if error_str.starts_with('{') {
            match serde_json::from_str::<Value>(&error_str) {
                Ok(Value::Object(map)) => {
                    // Try to get the "type" field
                    if let Some(type_val) = map.get("type") {
                        if let Some(type_str) = type_val.as_str() {
                            return type_str.to_string();
                        }
                    }
                },
                _ => {}
            }
        }
        
        // If not a JSON object or no "type" field, clean up the string
        error_str.trim_matches('"').to_string()
    }
}