use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

const ERR_MISSING_SOME: &str = "missing_some requires 2 arguments";
const ERR_FIRST_ARG: &str = "First argument must be a number";
const ERR_SECOND_ARG: &str = "Second argument must be an array";

pub struct MissingOperator;
pub struct MissingSomeOperator;

impl MissingOperator {
    fn process_keys(value: Value) -> Vec<String> {
        match value {
            Value::String(s) => vec![s],
            Value::Array(arr) => {
                let mut keys = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        Value::String(s) => keys.push(s),
                        Value::Number(n) => keys.push(n.to_string()),
                        _ => continue,
                    }
                }
                keys
            },
            Value::Number(n) => vec![n.to_string()],
            _ => Vec::new(),
        }
    }

    fn check_path(data: &Value, path: &str) -> bool {
        let mut current = data;
        
        for part in path.split('.') {
            match current {
                Value::Object(obj) => {
                    if let Some(val) = obj.get(part) {
                        current = val;
                    } else {
                        return true;
                    }
                },
                Value::Array(arr) => {
                    match part.parse::<usize>() {
                        Ok(index) if index < arr.len() => current = &arr[index],
                        _ => return true,
                    }
                },
                _ => return true,
            }
        }
        false
    }
}

impl Operator for MissingOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        // Fast path for empty args
        if args.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }

        // Pre-allocate with estimated capacity
        let mut missing = Vec::with_capacity(args.len());
        
        for arg in args {
            let key = arg.apply(data)?;
            let key_list = Self::process_keys(key);

            for key_str in key_list {
                if Self::check_path(data, &key_str) {
                    missing.push(Value::String(key_str));
                }
            }
        }
        
        Ok(Value::Array(missing))
    }
}

impl Operator for MissingSomeOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        // Fast path: validate args
        match args {
            [min_rule, keys_rule] => {
                // Get minimum required
                let min_required = min_rule.apply(data)?
                    .as_u64()
                    .ok_or_else(|| Error::InvalidRule(ERR_FIRST_ARG.into()))?;

                // Get keys array
                let keys = keys_rule.apply(data)?;
                let keys = keys.as_array()
                    .ok_or_else(|| Error::InvalidRule(ERR_SECOND_ARG.into()))?;

                // Fast path: empty keys array
                if keys.is_empty() {
                    return Ok(Value::Array(Vec::new()));
                }

                // Pre-allocate missing array with estimated capacity
                let mut missing = Vec::with_capacity(keys.len());
                let mut found_count = 0;

                // Single pass over keys
                for key in keys {
                    match key {
                        Value::String(key_str) => {
                            if MissingOperator::check_path(data, key_str) {
                                missing.push(Value::String(key_str.clone()));
                            } else {
                                found_count += 1;
                                // Fast path: we found enough keys
                                if found_count >= min_required {
                                    return Ok(Value::Array(Vec::new()));
                                }
                            }
                        }
                        _ => return Err(Error::InvalidRule("Keys must be strings".into()))
                    }
                }

                Ok(Value::Array(missing))
            }
            _ => Err(Error::InvalidArguments(ERR_MISSING_SOME.into()))
        }
    }
}