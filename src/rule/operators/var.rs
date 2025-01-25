use crate::{Error, JsonLogicResult};
use super::Rule;
use serde_json::Value;

const ERR_NOT_FOUND: &str = "Variable not found: ";
const ERR_OUT_OF_BOUNDS: &str = "Index out of bounds: ";
const ERR_INVALID_INDEX: &str = "Invalid array index: ";
const ERR_INVALID_PATH: &str = "Invalid path";

pub struct VarOperator;

impl VarOperator {
    #[inline]
    pub fn apply(&self, path: &Rule, default: Option<&Rule>, data: &Value) -> JsonLogicResult {
        let path_value = match path {
            Rule::Value(v) => v,
            _ => &path.apply(data)?
        };

        // Fast path for numbers - direct array access
        if let Value::Number(n) = path_value {
            if let Some(idx) = n.as_u64() {
                return self.get_array_direct(data, idx as usize)
                    .or_else(|_| default.map_or(Ok(Value::Null), |d| d.apply(data)));
            }
        }

        // Fast path for empty path
        if matches!(path_value, Value::String(s) if s.is_empty()) {
            return Ok(data.clone());
        }

        // Main path resolution
        let path_str = path_value.as_str().unwrap_or("");
        match self.get_value_ref(data, path_str) {
            Ok(value) => Ok(value.clone()),
            Err(_) => default.map_or(Ok(Value::Null), |d| d.apply(data))
        }
    }

    #[inline]
    fn get_array_direct(&self, data: &Value, idx: usize) -> Result<Value, Error> {
        match data {
            Value::Array(arr) => arr.get(idx).cloned()
                .ok_or_else(|| Error::InvalidArguments(ERR_OUT_OF_BOUNDS.into())),
            _ => Err(Error::InvalidArguments(ERR_INVALID_INDEX.into()))
        }
    }

    #[inline]
    fn get_value_ref<'a>(&self, data: &'a Value, path: &str) -> Result<&'a Value, Error> {
        // Existing fast paths...
        if path.is_empty() {
            return Ok(data);
        }

        // Simple key lookup optimization
        if !path.contains('.') {
            return self.get_simple_key(data, path);
        }

        // Existing nested lookup...
        let mut current = data;
        for part in path.split('.') {
            current = match current {
                Value::Object(obj) => obj.get(part)
                    .ok_or_else(|| Error::InvalidArguments(ERR_NOT_FOUND.into()))?,
                Value::Array(arr) => self.get_array_index(arr, part)?,
                _ => return Err(Error::InvalidArguments(ERR_INVALID_PATH.into()))
            };
        }
        Ok(current)
    }

    #[inline]
    fn get_array_index<'a>(&self, arr: &'a [Value], idx_str: &str) -> Result<&'a Value, Error> {
        idx_str.parse::<usize>()
            .map_err(|_| Error::InvalidArguments(ERR_INVALID_INDEX.into()))
            .and_then(|idx| arr.get(idx)
                .ok_or_else(|| Error::InvalidArguments(ERR_OUT_OF_BOUNDS.into())))
    }

    #[inline]
    fn get_simple_key<'a>(&self, data: &'a Value, key: &str) -> Result<&'a Value, Error> {
        match data {
            Value::Object(obj) => obj.get(key)
                .ok_or_else(|| Error::InvalidArguments(ERR_NOT_FOUND.into())),
            Value::Array(arr) => self.get_array_index(arr, key),
            _ => Err(Error::InvalidArguments(ERR_INVALID_PATH.into()))
        }
    }
}