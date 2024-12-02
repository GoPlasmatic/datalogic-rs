use crate::operators::operator::Operator;
use crate::{Error, JsonLogic, JsonLogicResult};
use serde_json::Value;

pub struct InOperator;

impl Operator for InOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Err(Error::InvalidArguments("in requires 2 arguments".into()));
            }

            let needle = logic.apply(&values[0], data)?;
            let haystack = logic.apply(&values[1], data)?;

            let result = match (&needle, &haystack) {
                // String in String
                (Value::String(n), Value::String(h)) => h.contains(n),
                // Element in Array
                (n, Value::Array(arr)) => arr.contains(n),
                _ => false,
            };

            Ok(Value::Bool(result))
        } else {
            Err(Error::InvalidArguments("in requires array argument".into()))
        }
    }
}

pub struct CatOperator;

impl Operator for CatOperator {
    fn apply(&self, _logic: &JsonLogic, args: &Value, _data: &Value) -> JsonLogicResult {
        match args {
            Value::Array(arr) if arr.is_empty() => Ok(Value::String(String::new())),
            Value::Array(arr) => {
                let total_len = arr.iter()
                    .map(|v| match v {
                        Value::String(s) => s.len(),
                        _ => 2
                    })
                    .sum();

                let mut result = String::with_capacity(total_len);
                
                for value in arr {
                    match value {
                        Value::String(s) => result.push_str(s),
                        Value::Null => {},
                        _ => result.push_str(&value.to_string())
                    }
                }
                
                Ok(Value::String(result))
            },
            Value::String(s) => Ok(Value::String(s.clone())),
            _ => Ok(Value::String(args.to_string()))
        }
    }
}

pub struct SubstrOperator;

impl Operator for SubstrOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        match args {
            Value::Array(arr) => {
                if arr.len() < 2 || arr.len() > 3 {
                    return Ok(Value::Null);
                }

                // Get string and convert to chars for proper Unicode handling
                let string = match logic.apply(&arr[0], data)? {
                    Value::String(s) => s,
                    other => other.to_string(),
                };
                let chars: Vec<char> = string.chars().collect();

                // Handle negative start index
                let start = match logic.apply(&arr[1], data)? {
                    Value::Number(n) => n.as_i64().unwrap_or(0) as isize,
                    _ => return Ok(Value::Null),
                };
                
                let start_idx = if start < 0 {
                    chars.len().saturating_sub((-start) as usize)
                } else {
                    start.min(chars.len() as isize) as usize
                };

                // Handle length with negative values
                let length = if arr.len() == 3 {
                    match logic.apply(&arr[2], data)? {
                        Value::Number(n) => n.as_i64().unwrap_or(0),
                        _ => return Ok(Value::Null),
                    }
                } else {
                    chars.len() as i64
                };

                let take_count = if length < 0 {
                    chars.len().saturating_sub(start_idx).saturating_sub((-length) as usize)
                } else {
                    length as usize
                };

                Ok(Value::String(
                    chars.iter()
                        .skip(start_idx)
                        .take(take_count)
                        .collect()
                ))
            }
            _ => Ok(Value::Null),
        }
    }
}