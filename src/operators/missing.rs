use crate::operators::operator::Operator;
use crate::{JsonLogic, JsonLogicResult};
use crate::operators::var::VarOperator;
use serde_json::Value;

pub struct MissingOperator;

impl MissingOperator {
    fn process_keys(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let mut missing = Vec::new();
        let keys = match args {
            Value::Object(obj) => {
                if let Some(Value::Array(elements)) = obj.get("merge") {
                    let mut result = Vec::new();
                    for elem in elements {
                        let evaluated = logic.apply(elem, data)?;
                        match evaluated {
                            Value::Array(arr) => result.extend(arr),
                            Value::String(s) => result.push(Value::String(s)),
                            _ => continue,
                        }
                    }
                    Value::Array(result)
                } else {
                    return Ok(Value::Array(vec![]))
                }
            }
            Value::Array(arr) => Value::Array(arr.to_owned()),
            Value::String(s) => Value::Array(vec![Value::String(s.to_owned())]),
            _ => return Ok(Value::Array(vec![])),
        };

        if let Value::Array(key_array) = keys {
            for key in key_array {
                if let Value::String(path) = key {
                    // Check if value is None (missing) or Some(Value::Null)
                    match VarOperator::get_value_at_path(data, &path) {
                        None | Some(Value::Null) => {
                            missing.push(Value::String(path));
                        },
                        Some(_) => {}  // Value exists and is not null
                    }
                }
            }
        }

        Ok(Value::Array(missing))
    }
}

impl Operator for MissingOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        self.process_keys(logic, args, data)
    }
}

pub struct MissingSomeOperator;

impl MissingSomeOperator {
    fn process_keys(&self, _logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Ok(Value::Array(vec![]));
            }

            let min_required = if let Value::Number(n) = &values[0] {
                n.as_u64().unwrap_or(0) as usize
            } else {
                return Ok(Value::Array(vec![]));
            };

            let mut missing = Vec::new();
            let mut found = 0;

            if let Value::Array(key_array) = &values[1] {
                for key in key_array {
                    if let Value::String(path) = key {
                        match VarOperator::get_value_at_path(data, path) {
                            None | Some(Value::Null) => {
                                missing.push(Value::String(path.to_owned()));
                            },
                            Some(_) => {
                                found += 1;
                            }
                        }
                    }
                }
            }

            if found >= min_required {
                Ok(Value::Array(vec![]))
            } else {
                Ok(Value::Array(missing))
            }
        } else {
            Ok(Value::Array(vec![]))
        }
    }
}

impl Operator for MissingSomeOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        self.process_keys(logic, args, data)
    }
}