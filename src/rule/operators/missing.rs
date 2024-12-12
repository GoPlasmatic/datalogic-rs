use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

pub struct MissingOperator;
pub struct MissingSomeOperator;

impl Operator for MissingOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        let mut missing = Vec::new();
        
        for arg in args {
            let key = arg.apply(data)?;
            let key_list = match key {
                Value::String(s) => vec![s],
                Value::Array(arr) => arr
                    .into_iter()
                    .filter_map(|v| match v {
                        Value::String(s) => Some(s),
                        Value::Number(n) => Some(n.to_string()),
                        _ => None,
                    })
                    .collect(),
                Value::Number(n) => vec![n.to_string()],
                _ => vec![],
            };

            for key_str in key_list {
                let parts: Vec<&str> = key_str.split('.').collect();
                let mut current = data;
                let mut is_missing = false;
                
                for part in parts {
                    match current {
                        Value::Object(obj) => {
                            if let Some(val) = obj.get(part) {
                                current = val;
                            } else {
                                is_missing = true;
                                break;
                            }
                        },
                        Value::Array(arr) => {
                            if let Ok(index) = part.parse::<usize>() {
                                if let Some(val) = arr.get(index) {
                                    current = val;
                                } else {
                                    is_missing = true;
                                    break;
                                }
                            } else {
                                is_missing = true;
                                break;
                            }
                        },
                        _ => {
                            is_missing = true;
                            break;
                        }
                    }
                }
                
                if is_missing {
                    missing.push(Value::String(key_str));
                }
            }
        }
        
        Ok(Value::Array(missing))
    }
}

impl Operator for MissingSomeOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("missing_some requires 2 arguments".to_string()));
        }

        // First argument is the minimum number of required fields
        let min_required = args[0].apply(data)?
            .as_u64()
            .ok_or_else(|| Error::InvalidRule("First argument must be a number".to_string()))?;

        // Second argument is the array of keys to check
        let keys = args[1].apply(data)?;
        let keys = keys.as_array()
            .ok_or_else(|| Error::InvalidRule("Second argument must be an array".to_string()))?;

        let mut key_rules = Vec::new();
        for key in keys {
            key_rules.push(Rule::Value(key.clone()));
        }

        // Use MissingOperator to find missing keys
        let missing_op = MissingOperator;
        let missing = missing_op.apply(&key_rules, data)?;
        let missing_count = missing.as_array().unwrap().len() as u64;

        // If we have enough required fields, return empty array
        if keys.len() as u64 - missing_count >= min_required {
            Ok(Value::Array(Vec::new()))
        } else {
            Ok(missing)
        }
    }
}