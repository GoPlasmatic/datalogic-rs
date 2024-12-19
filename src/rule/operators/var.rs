use super::{Operator, Rule};
use crate::Error;
use serde_json::Value;

pub struct VarOperator;

impl Operator for VarOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(data.clone());
        }

        let path_value = args[0].apply(data)?;
        let default_value = if args.len() > 1 {
            Some(args[1].apply(data)?)
        } else {
            None
        };

        let path_str = match path_value {
            Value::String(ref s) => s.clone(),
            Value::Number(ref n) => n.to_string(),
            _ => "".to_string(),
        };

        if path_str.is_empty() {
            return Ok(data.clone());
        }

        self.get_value_ref(data, &path_str).cloned().or({
            if let Some(default) = default_value {
                Ok(default)
            } else {
                Ok(Value::Null)
            }
        })
    }
}

const ERR_NOT_FOUND: &str = "Variable not found: ";
const ERR_OUT_OF_BOUNDS: &str = "Index out of bounds: ";
const ERR_INVALID_INDEX: &str = "Invalid array index: ";
const ERR_INVALID_PATH: &str = "Invalid path";

impl VarOperator {
    fn get_value_ref<'a>(&self, data: &'a Value, path: &str) -> Result<&'a Value, Error> {
        // Fast path for empty or root path
        if path.is_empty() {
            return Ok(data);
        }

        // Fast path for simple key lookup
        if !path.contains('.') {
            return match data {
                Value::Object(obj) => obj.get(path)
                    .ok_or_else(|| Error::InvalidArguments(format!("{}{}", ERR_NOT_FOUND, path))),
                Value::Array(arr) => path.parse::<usize>()
                    .ok()
                    .and_then(|i| arr.get(i))
                    .ok_or_else(|| Error::InvalidArguments(format!("{}{}", ERR_OUT_OF_BOUNDS, path))),
                _ => Err(Error::InvalidArguments(ERR_INVALID_PATH.into())),
            };
        }

        let mut current = data;
        
        for part in path.split('.') {
            current = match current {
                Value::Object(obj) => obj.get(part)
                    .ok_or_else(|| Error::InvalidArguments(format!("{}{}", ERR_NOT_FOUND, part)))?,
                    
                Value::Array(arr) => {
                    let index = part.parse::<usize>()
                        .map_err(|_| Error::InvalidArguments(format!("{}{}", ERR_INVALID_INDEX, part)))?;
                    arr.get(index)
                        .ok_or_else(|| Error::InvalidArguments(format!("{}{}", ERR_OUT_OF_BOUNDS, index)))?
                },
                
                _ => return Err(Error::InvalidArguments(ERR_INVALID_PATH.into()))
            };
        }

        Ok(current)
    }
}
