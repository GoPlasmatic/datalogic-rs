use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

const ERR_IN: &str = "in requires 2 arguments";
const ERR_SUBSTR: &str = "substr requires 2 or 3 arguments";

pub struct InOperator;
pub struct CatOperator;
pub struct SubstrOperator;

impl Operator for InOperator {
    #[inline]
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments(ERR_IN.into()));
        }

        let search = args[0].apply(data)?;
        let target = args[1].apply(data)?;

        Ok(Value::Bool(match (&search, &target) {
            (Value::String(s), Value::String(t)) => t.contains(s),
            (_, Value::Array(arr)) => arr.contains(&search),
            _ => false,
        }))
    }
}

impl Operator for CatOperator {
    #[inline]
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        // Fast paths
        match args.len() {
            0 => return Ok(Value::String(String::new())),
            1 => return Ok(Value::String(self.value_to_string(&args[0].apply(data)?))),
            _ => {}
        }

        // Pre-allocate with estimated capacity
        let capacity = args.len() * 8;
        let mut result = String::with_capacity(capacity);

        for arg in args {
            let value = arg.apply(data)?;
            self.append_value_to_string(&mut result, &value);
        }

        Ok(Value::String(result))
    }
}

impl CatOperator {
    #[inline]
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Array(arr) => {
                let mut result = String::with_capacity(arr.len() * 8);
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    self.append_value_to_string(&mut result, item);
                }
                result
            },
            Value::Object(_) => "[object Object]".to_string(),
        }
    }

    #[inline]
    fn append_value_to_string(&self, result: &mut String, value: &Value) {
        match value {
            Value::String(s) => result.push_str(s),
            Value::Number(n) => result.push_str(&n.to_string()),
            Value::Bool(b) => result.push_str(if *b { "true" } else { "false" }),
            Value::Null => result.push_str("null"),
            Value::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    self.append_value_to_string(result, item);
                }
            },
            Value::Object(_) => result.push_str("[object Object]"),
        }
    }
}

impl Operator for SubstrOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 || args.len() > 3 {
            return Err(Error::InvalidArguments(ERR_SUBSTR.into()));
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