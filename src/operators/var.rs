// src/operators/var.rs
use crate::operators::operator::Operator;
use crate::{JsonLogic, JsonLogicResult};
use serde_json::Value;

pub struct VarOperator;

impl VarOperator {
    fn get_nested_value<'a>(data: &'a Value, path: &str) -> &'a Value {
        if path.is_empty() {
            return data;
        }

        let parts: Vec<&str> = path.split('.').collect();
        let mut current = data;
        
        for part in parts {
            current = match current {
                Value::Array(arr) => {
                    if let Ok(index) = part.parse::<usize>() {
                        arr.get(index).unwrap_or(&Value::Null)
                    } else {
                        &Value::Null
                    }
                },
                Value::Object(obj) => obj.get(part).unwrap_or(&Value::Null),
                _ => &Value::Null,
            };
        }
        
        current
    }
}

impl Operator for VarOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        match args {
            Value::Array(parts) => {
                if parts.is_empty() {
                    return Ok(data.clone());
                }

                // Evaluate the first argument which might be a logic expression
                let path_result = logic.apply(&parts[0], data)?;
                
                let path = match path_result {
                    Value::String(s) => s,
                    Value::Number(n) => return match data {
                        Value::Array(arr) => {
                            let idx = n.as_u64().unwrap_or(0) as usize;
                            Ok(arr.get(idx).cloned().unwrap_or(Value::Null))
                        },
                        _ => Ok(Value::Null),
                    },
                    _ => return Ok(Value::Null),
                };

                if path.is_empty() {
                    return Ok(data.clone());
                }

                let result = Self::get_nested_value(data, &path);

                if *result == Value::Null && parts.len() > 1 {
                    logic.apply(&parts[1], data)
                } else {
                    Ok(result.clone())
                }
            },
            Value::String(path) => {
                if path.is_empty() {
                    Ok(data.clone())
                } else {
                    Ok(Self::get_nested_value(data, path).clone())
                }
            },
            Value::Number(n) => match data {
                Value::Array(arr) => {
                    let idx = n.as_u64().unwrap_or(0) as usize;
                    Ok(arr.get(idx).cloned().unwrap_or(Value::Null))
                },
                _ => Ok(Value::Null),
            },
            Value::Null => Ok(data.clone()), // Handle null path case
            _ => Ok(Value::Null),
        }
    }
}