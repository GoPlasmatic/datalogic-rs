use crate::Error;
use super::Rule;
use serde_json::Value;
use std::borrow::Cow;

const ERR_NOT_FOUND: &str = "Variable not found: ";
const ERR_OUT_OF_BOUNDS: &str = "Index out of bounds: ";
const ERR_INVALID_INDEX: &str = "Invalid array index: ";
const ERR_INVALID_PATH: &str = "Invalid path";

pub struct VarOperator;

impl VarOperator {
    pub fn apply<'a>(&'a self, path: &Rule, default: Option<&'a Rule>, context: &'a Value, root: &'a Value, rpath: &str) -> Result<Cow<'a, Value>, Error> {
        if let Rule::Value(Value::String(s)) = path {
            if s.is_empty() {
                return Ok(Cow::Borrowed(context));
            }
            // Optimize simple path lookup
            if !s.contains('.') {
                return self.get_simple_key(context, s)
                    .map(Cow::Borrowed)
                    .or_else(|_| match default {
                        Some(d) => d.apply(context, root, rpath),
                        None => Ok(Cow::Owned(Value::Null))
                    });
            }
        }

        let path_value = path.apply(context, root, rpath)?;

        // Fast path for numbers - direct array access
        if let Value::Number(n) = &*path_value {
            if let Some(idx) = n.as_u64() {
                if let Value::Array(arr) = context {
                    return match self.get_array_index(arr, idx as usize) {
                        Ok(value) => Ok(Cow::Borrowed(value)),
                        Err(_) => match default {
                            Some(d) => d.apply(context, root, rpath),
                            None => Ok(Cow::Owned(Value::Null))
                        }
                    };
                }
                return Err(Error::InvalidArguments(ERR_INVALID_INDEX.into()));
            }
        }

        // Main path resolution
        let path_str = path_value.as_ref().as_str().unwrap_or("");
        match self.get_value_ref(context, path_str) {
            Ok(value) => Ok(Cow::Borrowed(value)),
            Err(_) => match default {
                Some(d) => d.apply(context, root, rpath),
                None => Ok(Cow::Owned(Value::Null))
            }
        }
    }

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
                Value::Array(arr) => {
                    let idx = part.parse::<usize>().map_err(|_| Error::InvalidArguments(ERR_INVALID_INDEX.into()))?;
                    self.get_array_index(arr, idx)?
                },
                _ => return Err(Error::InvalidArguments(ERR_INVALID_PATH.into()))
            };
        }
        Ok(current)
    }

    fn get_array_index<'a>(&self, arr: &'a [Value], idx: usize) -> Result<&'a Value, Error> {
        arr.get(idx)
            .ok_or_else(|| Error::InvalidArguments(ERR_OUT_OF_BOUNDS.into()))
    }

    fn get_simple_key<'a>(&self, data: &'a Value, key: &str) -> Result<&'a Value, Error> {
        match data {
            Value::Object(obj) => obj.get(key)
                .ok_or_else(|| Error::InvalidArguments(ERR_NOT_FOUND.into())),
            Value::Array(arr) => {
                let idx = key.parse::<usize>().map_err(|_| Error::InvalidArguments(ERR_INVALID_INDEX.into()))?;
                self.get_array_index(arr, idx)
            },
            _ => Err(Error::InvalidArguments(ERR_INVALID_PATH.into()))
        }
    }
}