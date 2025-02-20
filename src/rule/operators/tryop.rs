use serde_json::{json, Value};
use crate::{rule::ArgType, error::Error};
use std::borrow::Cow;

pub struct TryOperator;

impl TryOperator {
    pub fn apply<'a>(&self, args: &'a ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match args {
            ArgType::Multiple(rules) => {
                let mut last_error = None;
                
                for rule in rules {
                    let current_data = if let Some(err) = &last_error {
                        json!({ "type": err })
                    } else {
                        data.to_owned()
                    };
    
                    match rule.apply(&current_data) {
                        Ok(value) => return Ok(Cow::Owned(value.into_owned())),
                        Err(e) => {
                            let clean_error = self.normalize_error(&e);
                            last_error = Some(clean_error);
                            continue;
                        }
                    }
                }
                
                // If we get here, all rules failed
                if let Some(last_err) = last_error {
                    Err(Error::Custom(last_err))
                } else {
                    Err(Error::Custom("No valid value found".to_string()))
                }
            },
            ArgType::Unary(rule) => rule.apply(data)
        }
    }

    fn normalize_error(&self, error: &Error) -> String {
        let error_str = error.to_string();
        
        if error_str.starts_with('{') {
            if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&error_str) {
                if let Some(type_val) = map.get("type") {
                    if let Some(type_str) = type_val.as_str() {
                        return type_str.to_string();
                    }
                }
            }
        }
        
        error_str.trim_matches('"').to_string()
    }
}