use serde_json::Value;
use crate::{JsonLogicResult, rule::ArgType};

pub struct ValOperator;

impl ValOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => {
                        let mut current = data;
                        for key in arr {
                            match access_value(current, &key) {
                                Some(value) => current = value,
                                None => return Ok(Value::Null)
                            }
                        }
                        Ok(current.clone())
                    },
                    _ => Ok(match access_value(data, &value) {
                        Some(value) => value.clone(),
                        None => Value::Null
                    })
                }
            },
            ArgType::Multiple(rules) => {
                match rules.len() {
                    0 => Ok(data.clone()),
                    _ => {
                        let mut current = data;
                        for rule in rules {
                            match rule.apply(current)? {
                                Value::Array(arr) => {
                                    for key in arr {
                                        match access_value(current, &key) {
                                            Some(value) => current = value,
                                            None => return Ok(Value::Null)
                                        }
                                    }
                                },
                                value => {
                                    match access_value(current, &value) {
                                        Some(value) => current = value,
                                        None => return Ok(Value::Null)
                                    }
                                }
                            }
                        }
                        Ok(current.clone())
                    }
                }
            }
        }
    }
}

pub struct ExistsOperator;

impl ExistsOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => {
                        let mut current = data;
                        for key in arr {
                            match access_value(current, &key) {
                                Some(value) => current = value,
                                None => return Ok(false.into()),
                            }
                        }
                        Ok(true.into())
                    },
                    _ => Ok(match access_value(data, &value) {
                        Some(_) => true.into(),
                        None => false.into(),
                    })
                }
            },
            ArgType::Multiple(rules) => {
                match rules.len() {
                    0 => Ok(false.into()),
                    _ => {
                        let mut current = data;
                        for rule in rules {
                            match rule.apply(current)? {
                                Value::Array(arr) => {
                                    for key in arr {
                                        match access_value(current, &key) {
                                            Some(value) => current = value,
                                            None => return Ok(false.into())
                                        }
                                    }
                                },
                                value => {
                                    match access_value(current, &value) {
                                        Some(value) => current = value,
                                        None => return Ok(false.into())
                                    }
                                }
                            }
                        }
                        Ok(true.into())
                    }
                }
            }
        }
    }
}

#[inline(always)]
fn access_value<'a>(data: &'a Value, key: &Value) -> Option<&'a Value> {
    match (data, key) {
        (Value::Null, _) => None,
        (Value::Object(map), Value::String(s)) => map.get(s),
        (Value::Array(arr), Value::Number(n)) => {
            if let Some(idx) = n.as_u64() {
                arr.get(idx as usize)
            } else {
                None
            }
        }
        (Value::Array(arr), Value::String(s)) => {
            if let Ok(idx) = s.parse::<f64>() {
                arr.get(idx as usize)
            } else {
                None
            }
        }
        _ => None
    }
}