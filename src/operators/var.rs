// src/operators/var.rs
use crate::operators::operator::Operator;
use crate::{JsonLogic, JsonLogicResult};
use serde_json::Value;

pub struct VarOperator;

impl VarOperator {
    pub(crate) fn get_value_at_path(data: &Value, path: &str) -> Option<Value> {
        if path.is_empty() {
            return Some(data.clone());
        }

        let mut current = data;
        for part in path.split('.') {
            current = match (current, part.parse::<usize>()) {
                (Value::Object(map), _) => map.get(part).unwrap_or(&Value::Null),
                (Value::Array(arr), Ok(index)) => arr.get(index).unwrap_or(&Value::Null),
                _ => return None
            };

            if current == &Value::Null {
                return None;
            }
        }

        Some(current.clone())
    }

    fn get_default<'a>(default: Option<&'a Value>) -> JsonLogicResult {
        default.map_or(Ok(Value::Null), |d| Ok(d.clone()))
    }

    fn handle_numeric_index(index: usize, data: &Value, default: Option<&Value>) -> JsonLogicResult {
        if let Value::Array(arr) = data {
            match arr.get(index) {
                Some(v) => Ok(v.clone()),
                None => Self::get_default(default)
            }
        } else {
            Self::get_default(default)
        }
    }

    fn handle_string_path(path_str: &str, data: &Value, default: Option<&Value>) -> JsonLogicResult {
        if path_str.is_empty() {
            return Ok(data.clone());
        }

        let mut current = data;
        for part in path_str.split('.') {
            current = match (current, part.parse::<usize>()) {
                (Value::Object(map), _) => map.get(part).unwrap_or(&Value::Null),
                (Value::Array(arr), Ok(index)) => arr.get(index).unwrap_or(&Value::Null),
                _ => return Self::get_default(default)
            };

            if current == &Value::Null {
                return Self::get_default(default);
            }
        }

        Ok(current.clone())
    }
}

impl Operator for VarOperator {
    fn auto_traverse(&self) -> bool {
        false
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        // Extract path and default
        let (path_arg, default) = match args {
            Value::Array(arr) if arr.len() >= 2 => (&arr[0], Some(&arr[1])),
            Value::Array(arr) if arr.len() == 1 => (&arr[0], None),
            Value::String(_) | Value::Number(_) => (args, None),
            _ => (&Value::String("".to_string()), None)
        };

        // Evaluate path if needed
        let path = match path_arg {
            Value::Object(_) | Value::Array(_) => logic.apply(path_arg, data)?,
            _ => path_arg.clone()
        };

        // Handle different path types
        match path {
            Value::Number(n) => {
                if let Some(index) = n.as_u64() {
                    Self::handle_numeric_index(index as usize, data, default)
                } else {
                    Self::get_default(default)
                }
            },
            Value::String(path_str) => Self::handle_string_path(&path_str, data, default),
            _ => Self::get_default(default)
        }
    }
}