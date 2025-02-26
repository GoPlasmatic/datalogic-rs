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
    fn check_path(data: &Value, path: &str) -> bool {
        if !data.is_object() {
            return true;
        }
        if !path.contains('.') {
            if let Value::Object(obj) = data {
                return !obj.contains_key(path);
            }
            return true;
        }

        let mut current = data;
        
        for part in path.split('.') {
            if let Value::Object(obj) = current {
                if let Some(val) = obj.get(part) {
                    current = val;
                } else {
                    return true;
                }
            } else {
                return true;
            }
        }
        false
    }

    pub fn apply<'a>(&self, args: &[Rule], context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        if args.is_empty() {
            return Ok(Cow::Owned(Value::Array(Vec::new())));
        }
    
        let mut missing = Vec::with_capacity(args.len());
        
        for arg in args {
            let key = arg.apply(context, root, path)?;
            match &*key {
                Value::String(s) => {
                    if Self::check_path(context, s) {
                        missing.push(key.as_ref().clone());
                    }
                },
                Value::Array(arr) => {
                    for v in arr {
                        if let Value::String(s) = v {
                            if Self::check_path(context, s) {
                                missing.push(v.clone());
                            }
                        }
                    }
                },
                _ => continue,
            };
        }
        
        Ok(Cow::Owned(Value::Array(missing)))
    }
}

impl MissingSomeOperator {
    pub fn apply<'a>(&self, args: &[Rule], context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match args {
            [min_rule, keys_rule] => {
                let min_required = min_rule.apply(context, root, path)?
                    .as_ref()
                    .as_u64()
                    .ok_or_else(|| Error::InvalidExpression(ERR_FIRST_ARG.into()))?;

                let keys = keys_rule.apply(context, root, path)?;
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
                        if MissingOperator::check_path(context, key_str) {
                            missing.push(key.clone());
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