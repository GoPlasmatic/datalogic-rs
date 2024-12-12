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

        self.get_value_by_path(data, &path_str).or({
            if let Some(default) = default_value {
                Ok(default)
            } else {
                Ok(Value::Null)
            }
        })
    }
}

impl VarOperator {
    fn get_value_by_path(&self, data: &Value, path: &str) -> Result<Value, Error> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = data;

        for part in parts {
            match current {
                Value::Object(obj) => {
                    if let Some(val) = obj.get(part) {
                        current = val;
                    } else {
                        return Err(Error::InvalidArguments(format!(
                            "Variable '{}' not found",
                            part
                        )));
                    }
                }
                Value::Array(arr) => {
                    if let Ok(index) = part.parse::<usize>() {
                        if let Some(val) = arr.get(index) {
                            current = val;
                        } else {
                            return Err(Error::InvalidArguments(format!(
                                "Index '{}' out of bounds",
                                index
                            )));
                        }
                    } else {
                        return Err(Error::InvalidArguments(format!(
                            "Invalid array index '{}'",
                            part
                        )));
                    }
                }
                _ => {
                    return Err(Error::InvalidArguments("Invalid path".to_string()));
                }
            }
        }

        Ok(current.clone())
    }
}
