use serde_json::Value;
use super::{Error, EvalResult};

const ERR_NOT_FOUND: &str = "Variable not found: ";
const ERR_OUT_OF_BOUNDS: &str = "Index out of bounds: ";
const ERR_INVALID_INDEX: &str = "Invalid array index: ";
const ERR_INVALID_PATH: &str = "Invalid path";

#[inline]
pub fn evaluate_var(path: &Value, data: &Value, default: Option<&Value>) -> EvalResult {
    if let Value::Number(n) = path {
        if let Some(idx) = n.as_u64() {
            return get_array_direct(data, idx as usize)
                .or_else(|_| Ok(default.cloned().unwrap_or(Value::Null)));
        }
    }

    if matches!(path, Value::String(s) if s.is_empty()) {
        return Ok(data.clone());
    }

    let path_str = path.as_str().unwrap_or("");
    match get_value_ref(data, path_str) {
        Ok(value) => Ok(value.clone()),
        Err(_) => default.map_or(Ok(Value::Null), |d| Ok(d.clone()))
    }
}

#[inline(always)]
fn get_array_direct(data: &Value, idx: usize) -> Result<Value, Error> {
    match data {
        Value::Array(arr) => arr.get(idx).cloned()
            .ok_or_else(|| Error::InvalidArguments(ERR_OUT_OF_BOUNDS.into())),
        _ => Err(Error::InvalidArguments(ERR_INVALID_INDEX.into()))
    }
}

#[inline(always)]
fn get_value_ref<'a>(data: &'a Value, path: &str) -> Result<&'a Value, Error> {
    if path.is_empty() {
        return Ok(data);
    }

    if !path.contains('.') {
        return get_simple_key(data, path);
    }

    let mut current = data;
    for part in path.split('.') {
        current = match current {
            Value::Object(obj) => obj.get(part)
                .ok_or_else(|| Error::InvalidArguments(ERR_NOT_FOUND.into()))?,
            Value::Array(arr) => get_array_index(arr, part)?,
            _ => return Err(Error::InvalidArguments(ERR_INVALID_PATH.into()))
        };
    }
    Ok(current)
}

#[inline(always)]
fn get_array_index<'a>(arr: &'a [Value], idx_str: &str) -> Result<&'a Value, Error> {
    idx_str.parse::<usize>()
        .map_err(|_| Error::InvalidArguments(ERR_INVALID_INDEX.into()))
        .and_then(|idx| arr.get(idx)
            .ok_or_else(|| Error::InvalidArguments(ERR_OUT_OF_BOUNDS.into())))
}

#[inline(always)]
fn get_simple_key<'a>(data: &'a Value, key: &str) -> Result<&'a Value, Error> {
    match data {
        Value::Object(obj) => obj.get(key)
            .ok_or_else(|| Error::InvalidArguments(ERR_NOT_FOUND.into())),
        Value::Array(arr) => get_array_index(arr, key),
        _ => Err(Error::InvalidArguments(ERR_INVALID_PATH.into()))
    }
}