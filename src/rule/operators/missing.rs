use serde_json::Value;
use crate::Error;
use super::Rule;
use std::borrow::Cow;

const ERR_MISSING_SOME: &str = "missing_some requires 2 arguments";
const ERR_FIRST_ARG: &str = "First argument must be a number";
const ERR_SECOND_ARG: &str = "Second argument must be an array";

pub struct MissingOperator;
pub struct MissingSomeOperator;

impl MissingOperator {
    fn process_keys<'a>(value: &'a Value) -> Vec<Cow<'a, str>> {
        match value {
            Value::String(s) => vec![Cow::Borrowed(s.as_str())],
            Value::Array(arr) => {
                let mut keys = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        Value::String(s) => keys.push(Cow::Borrowed(s.as_str())),
                        Value::Number(n) => keys.push(Cow::Owned(n.to_string())),
                        _ => continue,
                    }
                }
                keys
            },
            Value::Number(n) => vec![Cow::Owned(n.to_string())],
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

    pub fn apply<'a>(&self, args: &[Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        if args.is_empty() {
            return Ok(Cow::Owned(Value::Array(Vec::new())));
        }
    
        let mut missing = Vec::with_capacity(args.len());
        
        for arg in args {
            let key = arg.apply(data)?;
            let key_list = Self::process_keys(&key);
    
            for key in key_list {
                if Self::check_path(data, &key) {
                    missing.push(Value::String(key.into_owned()));
                }
            }
        }
        
        Ok(Cow::Owned(Value::Array(missing)))
    }
}

impl MissingSomeOperator {
    pub fn apply<'a>(&self, args: &[Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match args {
            [min_rule, keys_rule] => {
                let min_required = min_rule.apply(data)?
                    .as_ref()
                    .as_u64()
                    .ok_or_else(|| Error::InvalidExpression(ERR_FIRST_ARG.into()))?;

                let keys = keys_rule.apply(data)?;
                let keys = keys
                    .as_ref()
                    .as_array()
                    .ok_or_else(|| Error::InvalidExpression(ERR_SECOND_ARG.into()))?;

                if keys.is_empty() {
                    return Ok(Cow::Owned(Value::Array(Vec::new())));
                }

                let mut missing = Vec::with_capacity(keys.len());
                let mut found_count = 0;

                for key in keys {
                    if let Value::String(key_str) = key {
                        if MissingOperator::check_path(data, key_str) {
                            missing.push(Value::String(key_str.to_owned()));
                        } else {
                            found_count += 1;
                            if found_count >= min_required {
                                return Ok(Cow::Owned(Value::Array(Vec::new())));
                            }
                        }
                    } else {
                        return Err(Error::InvalidExpression("Keys must be strings".into()));
                    }
                }

                Ok(Cow::Owned(Value::Array(missing)))
            }
            _ => Err(Error::InvalidArguments(ERR_MISSING_SOME.into()))
        }
    }
}