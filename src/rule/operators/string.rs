use serde_json::Value;
use crate::JsonLogicResult;
use super::{Rule, ValueCoercion};

pub struct InOperator;
pub struct CatOperator;
pub struct SubstrOperator;

impl InOperator {
    pub fn apply(&self, search: &Rule, target: &Rule, data: &Value) -> JsonLogicResult {
        let search = search.apply(data)?;
        let target = target.apply(data)?;        
        Ok(Value::Bool(match (&search, &target) {
            (Value::String(s), Value::String(t)) => t.contains(s),
            (_, Value::Array(arr)) => arr.contains(&search),
            _ => false,
        }))
    }
}

impl CatOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        // Fast paths
        match args.len() {
            0 => return Ok(Value::String(String::new())),
            1 => return Ok(Value::String(args[0].apply(data)?.coerce_to_string())),
            _ => {}
        }

        // Pre-allocate with estimated capacity
        let capacity = args.len() * 16;
        let mut result = String::with_capacity(capacity);

        for arg in args {
            let value = arg.apply(data)?;
            Value::coerce_append(&mut result, &value);
        }

        Ok(Value::String(result))
    }
}

impl SubstrOperator {
    pub fn apply(&self, string: &Rule, start: &Rule, length: Option<&Rule>, data: &Value) -> JsonLogicResult {
        let string = string.apply(data)?;
        let string = match string {
            Value::String(s) => s,
            _ => return Ok(Value::String(String::new())),
        };

        let chars: Vec<char> = string.chars().collect();
        let str_len = chars.len() as i64;

        let start = start.apply(data)?;
        let start_idx = match start {
            Value::Number(n) => {
                let start = n.as_i64().unwrap_or(0);
                if start < 0 {
                    (str_len + start).max(0) as usize
                } else {
                    start.min(str_len) as usize
                }
            },
            _ => return Ok(Value::String(String::new())),
        };

        let length = if length.is_some() {
            Some(length.unwrap().apply(data)?)
        } else {
            None
        };
        match length {
            Some(Value::Number(n)) => {
                let len = n.as_i64().unwrap_or(0);
                let end_idx = if len < 0 {
                    (str_len + len) as usize
                } else {
                    (start_idx + len as usize).min(chars.len())
                };
                
                if end_idx <= start_idx {
                    Ok(Value::String(String::new()))
                } else {
                    Ok(Value::String(chars[start_idx..end_idx].iter().collect()))
                }
            },
            None => {
                Ok(Value::String(chars[start_idx..].iter().collect()))
            },
            _ => Ok(Value::String(String::new())),
        }
    }
}