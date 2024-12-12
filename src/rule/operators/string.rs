use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

pub struct InOperator;
pub struct CatOperator;
pub struct SubstrOperator;

impl Operator for InOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("in requires 2 arguments".to_string()));
        }

        let search = args[0].apply(data)?;
        let target = args[1].apply(data)?;

        match (search, target) {
            (Value::String(s), Value::String(t)) => Ok(Value::Bool(t.contains(&s))),
            (search, Value::Array(arr)) => Ok(Value::Bool(arr.contains(&search))),
            _ => Ok(Value::Bool(false)),
        }
    }
}

impl Operator for CatOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        let mut result = String::new();

        for arg in args {
            let value = arg.apply(data)?;
            match value {
                Value::String(s) => result.push_str(&s),
                Value::Number(n) => result.push_str(&n.to_string()),
                Value::Bool(b) => result.push_str(&b.to_string()),
                Value::Null => result.push_str("null"),
                Value::Array(arr) => {
                    for (i, item) in arr.iter().enumerate() {
                        if i > 0 {
                            result.push(',');
                        }
                        match item {
                            Value::String(s) => result.push_str(s),
                            _ => result.push_str(&item.to_string()),
                        }
                    }
                }
                Value::Object(_) => result.push_str("[object Object]"),
            }
        }

        Ok(Value::String(result))
    }
}

impl Operator for SubstrOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 || args.len() > 3 {
            return Err(Error::InvalidArguments("substr requires 2 or 3 arguments".to_string()));
        }

        let string = args[0].apply(data)?;
        let start = args[1].apply(data)?;
        let length = if args.len() == 3 {
            Some(args[2].apply(data)?)
        } else {
            None
        };

        let string = match string {
            Value::String(s) => s,
            _ => return Ok(Value::String(String::new())),
        };

        let chars: Vec<char> = string.chars().collect();
        let str_len = chars.len() as i64;

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