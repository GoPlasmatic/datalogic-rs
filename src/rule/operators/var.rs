use crate::{Error, JsonLogicResult};
use super::Rule;
use serde_json::Value;

const ERR_NOT_FOUND: &str = "Variable not found: ";
const ERR_OUT_OF_BOUNDS: &str = "Index out of bounds: ";
const ERR_INVALID_INDEX: &str = "Invalid array index: ";
const ERR_INVALID_PATH: &str = "Invalid path";

pub struct VarOperator;

impl VarOperator {
    pub fn apply(&self, path: &Rule, default: Option<&Rule>, data: &Value) -> JsonLogicResult {
        let path_value = match path {
            Rule::Value(v) => v,
            _ => &path.apply(data)?
        };

        // Fast path for numbers - direct array access
        if let Value::Number(n) = path_value {
            if let Some(idx) = n.as_u64() {
                if let Value::Array(arr) = data {
                    return match self.get_array_index(arr, &idx.to_string()) {
                        Ok(value) => match value {
                            Value::String(s) => Ok(Value::String(s.clone())),
                            Value::Number(n) => Ok(Value::Number(n.clone())),
                            Value::Bool(b) => Ok(Value::Bool(*b)),
                            Value::Null => Ok(Value::Null),
                            _ => Ok(value.clone())
                        },
                        Err(_) => default.map_or(Ok(Value::Null), |d| d.apply(data))
                    };
                }
                return Err(Error::InvalidArguments(ERR_INVALID_INDEX.into()));
            }
        }

        // Fast path for empty path
        if matches!(path_value, Value::String(s) if s.is_empty()) {
            return match data {
                Value::Object(_) | Value::Array(_) => Ok(data.clone()),
                _ => Ok(data.to_owned()) // More efficient for primitive types
            };
        }

        // Main path resolution
        let path_str = path_value.as_str().unwrap_or("");
        match self.get_value_ref(data, path_str) {
            Ok(value) => match value {
                Value::String(s) => Ok(Value::String(s.clone())),
                Value::Number(n) => Ok(Value::Number(n.clone())),
                Value::Bool(b) => Ok(Value::Bool(*b)),
                Value::Null => Ok(Value::Null),
                _ => Ok(value.clone()) // Fall back to clone for complex types
            },
            Err(_) => default.map_or(Ok(Value::Null), |d| d.apply(data))
        }
    }

    #[inline(always)]
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

    #[inline(always)]
    fn get_array_index<'a>(&self, arr: &'a [Value], idx_str: &str) -> Result<&'a Value, Error> {
        idx_str.parse::<usize>()
            .map_err(|_| Error::InvalidArguments(ERR_INVALID_INDEX.into()))
            .and_then(|idx| arr.get(idx)
                .ok_or_else(|| Error::InvalidArguments(ERR_OUT_OF_BOUNDS.into())))
    }

    #[inline(always)]
    fn get_simple_key<'a>(&self, data: &'a Value, key: &str) -> Result<&'a Value, Error> {
        match data {
            Value::Object(obj) => obj.get(key)
                .ok_or_else(|| Error::InvalidArguments(ERR_NOT_FOUND.into())),
            Value::Array(arr) => self.get_array_index(arr, key),
            _ => Err(Error::InvalidArguments(ERR_INVALID_PATH.into()))
        }
    }
}