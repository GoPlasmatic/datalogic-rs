use serde_json::Value;
use crate::{rule::ArgType, Error};
use std::borrow::Cow;

pub struct ValOperator;

impl ValOperator {
    pub fn apply<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match &*value {
                    Value::Array(arr) => {
                        let mut current = data;
                        for key in arr {
                            match access_value(current, key) {
                                Some(value) => current = value,
                                None => return Ok(Cow::Owned(Value::Null))
                            }
                        }
                        Ok(Cow::Borrowed(current))
                    },
                    _ => Ok(match access_value(data, &value) {
                        Some(value) => Cow::Borrowed(value),
                        None => Cow::Owned(Value::Null)
                    })
                }
            },
            ArgType::Multiple(rules) => {
                match rules.len() {
                    0 => Ok(Cow::Borrowed(data)),
                    _ => {
                        let mut current = data;
                        for rule in rules {
                            let value = rule.apply(current)?;
                            match &*value {
                                Value::Array(arr) => {
                                    for key in arr {
                                        match access_value(current, key) {
                                            Some(value) => current = value,
                                            None => return Ok(Cow::Owned(Value::Null))
                                        }
                                    }
                                },
                                _ => {
                                    match access_value(current, &value) {
                                        Some(value) => current = value,
                                        None => return Ok(Cow::Owned(Value::Null))
                                    }
                                }
                            }
                        }
                        Ok(Cow::Borrowed(current))
                    }
                }
            }
        }
    }
}

pub struct ExistsOperator;

impl ExistsOperator {
    pub fn apply<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match &*value {
                    Value::Array(arr) => {
                        let mut current = data;
                        for key in arr {
                            match access_value(current, key) {
                                Some(value) => current = value,
                                None => return Ok(Cow::Owned(Value::Bool(false))),
                            }
                        }
                        Ok(Cow::Owned(Value::Bool(true)))
                    },
                    _ => Ok(Cow::Owned(Value::Bool(access_value(data, &value).is_some())))
                }
            },
            ArgType::Multiple(rules) => {
                match rules.len() {
                    0 => Ok(Cow::Owned(Value::Bool(false))),
                    _ => {
                        let mut current = data;
                        for rule in rules {
                            let value = rule.apply(current)?;
                            match &*value {
                                Value::Array(arr) => {
                                    for key in arr {
                                        match access_value(current, key) {
                                            Some(value) => current = value,
                                            None => return Ok(Cow::Owned(Value::Bool(false)))
                                        }
                                    }
                                },
                                _ => {
                                    match access_value(current, &value) {
                                        Some(value) => current = value,
                                        None => return Ok(Cow::Owned(Value::Bool(false)))
                                    }
                                }
                            }
                        }
                        Ok(Cow::Owned(Value::Bool(true)))
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