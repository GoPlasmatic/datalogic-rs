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
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let values: Vec<&Value> = match args {
            Value::Array(arr) => arr.iter().collect(),
            value => vec![value],
        };

        let result = values
            .iter()
            .map(|v| logic.apply(v, data))
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .map(|v| match v {
                Value::String(s) => s.to_string(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                _ => "".to_string(),
            })
            .collect::<Vec<String>>()
            .join("");

        Ok(Value::String(result))
    }
}

pub struct SubstrOperator;

impl Operator for SubstrOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            let str_val = logic.apply(&values[0], data)?;
            let str = match str_val {
                Value::String(s) => s,
                _ => return Err(Error::InvalidArguments("substr requires string first argument".into())),
            };

            let start = if let Some(start_val) = values.get(1) {
                let start_num = logic.apply(start_val, data)?;
                match start_num {
                    Value::Number(n) => n.as_i64().unwrap_or(0) as i32,
                    _ => 0,
                }
            } else {
                0
            };

            let length = if let Some(len_val) = values.get(2) {
                let len_num = logic.apply(len_val, data)?;
                match len_num {
                    Value::Number(n) => Some(n.as_i64().unwrap_or(0) as i32),
                    _ => None,
                }
            } else {
                None
            };

            let chars: Vec<char> = str.chars().collect();
            let len = chars.len() as i32;
            
            // Handle negative start index
            let normalized_start = if start < 0 {
                (len + start).max(0) as usize
            } else {
                start as usize
            };

            let result = match length {
                Some(l) => {
                    let normalized_len = if l < 0 {
                        (len - normalized_start as i32 + l).max(0) as usize
                    } else {
                        l as usize
                    };
                    chars[normalized_start..].iter().take(normalized_len).collect()
                },
                None => chars[normalized_start..].iter().collect(),
            };

            Ok(Value::String(result))
        } else {
            Err(Error::InvalidArguments("substr requires array arguments".into()))
        }
    }
}