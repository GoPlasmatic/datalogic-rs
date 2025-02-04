use serde_json::Value;
use super::{coercion::ValueCoercion, EvalResult};

pub fn evaluate_in(args: &[Value]) -> EvalResult {
    let needle = args.first().unwrap_or(&Value::Null);
    let haystack = args.get(1).unwrap_or(&Value::Null);
    
    match (needle, haystack) {
        (Value::String(n), Value::String(h)) => {
            Ok(Value::Bool(h.contains(n)))
        }
        (n, Value::Array(arr)) => {
            Ok(Value::Bool(arr.contains(n)))
        }
        (Value::String(_), _) | (_, Value::String(_)) => {
            Ok(Value::Bool(false))
        }
        _ => Ok(Value::Bool(false))
    }
}

pub fn evaluate_cat(args: &[Value]) -> EvalResult {
    let capacity = args.iter()
        .map(|arg| match arg {
            Value::String(s) => s.len(),
            Value::Number(_) => 20,
            _ => 8,
        })
        .sum();

    let mut result = String::with_capacity(capacity);

    for arg in args {
        Value::coerce_append(&mut result, arg);
    }

    Ok(Value::String(result))
}

pub fn evaluate_substr(args: &[Value]) -> EvalResult {
    let string = args.first().unwrap_or(&Value::Null);
    let string = match string {
        Value::String(s) => s,
        _ => return Ok(Value::String(String::new())),
    };

    let chars: Vec<char> = string.chars().collect();
    let str_len = chars.len() as i64;

    let start_idx = match args.get(1).unwrap() {
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

    let length = args.get(2);
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