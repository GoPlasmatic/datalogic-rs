use serde_json::Value;
use super::EvalResult;

pub fn evaluate_missing(args: &[Value], data: &Value) -> EvalResult {
    if args.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }

    let mut missing = Vec::with_capacity(args.len());
    
    for arg in args {
        match arg {
            Value::String(s) if check_path(data, s) => {
                missing.push(Value::String(s.clone()));
            },
            Value::Array(arr) => {
                for v in arr {
                    if let Value::String(s) = v {
                        if check_path(data, s) {
                            missing.push(Value::String(s.clone()));
                        }
                    }
                }
            },
            _ => continue,
        }
    }
    
    Ok(Value::Array(missing))
}

pub fn evaluate_missing_some(args: &[Value], data: &Value) -> EvalResult {
    if args.len() != 2 {
        return Ok(Value::Array(vec![]));
    }

    let required_count = match &args[0] {
        Value::Number(n) => n.as_u64().unwrap_or(0) as usize,
        _ => return Ok(Value::Array(vec![])),
    };

    let keys = match &args[1] {
        Value::Array(keys) => keys,
        _ => return Ok(Value::Array(vec![])),
    };

    let mut missing = Vec::with_capacity(keys.len());
    let mut found = 0;

    for key in keys {
        if let Value::String(key_str) = key {
            if !check_path(data, key_str) {
                found += 1;
                if found >= required_count {
                    return Ok(Value::Array(vec![]));
                }
            } else {
                missing.push(key.clone());
            }
        }
    }

    Ok(Value::Array(missing))
}

fn check_path(data: &Value, path: &str) -> bool {
    let mut current = data;
    
    for part in path.split('.') {
        match current {
            Value::Object(obj) => {
                if let Some(val) = obj.get(part) {
                    if val.is_null() {
                        return true;
                    }
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