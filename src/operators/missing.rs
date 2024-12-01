use crate::operators::operator::Operator;
use crate::{JsonLogic, JsonLogicResult};
use crate::operators::var::VarOperator;
use serde_json::Value;

pub struct MissingOperator;
pub struct MissingSomeOperator;

impl MissingOperator {
    fn process_keys(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let mut missing = Vec::new();
        let keys = match args {
            Value::Object(obj) => {
                if let Some(merge_arr) = obj.get("merge") {
                    // For each element in merge array, evaluate it
                    if let Value::Array(elements) = merge_arr {
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
                } else {
                    return Ok(Value::Array(vec![]))
                }
            }
            Value::Array(arr) => Value::Array(arr.clone()),
            Value::String(s) => Value::Array(vec![Value::String(s.clone())]),
            _ => return Ok(Value::Array(vec![])),
        };

        if let Value::Array(key_array) = keys {
            for key in key_array {
                if let Value::String(path) = key {
                    let value = VarOperator::get_nested_value(data, &path);
                    if value == &Value::Null {
                        missing.push(Value::String(path));
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

impl Operator for MissingSomeOperator {
    fn apply(&self, _logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Ok(Value::Array(vec![]));
            }

            let min_required = if let Value::Number(n) = &values[0] {
                n.as_u64().unwrap_or(0) as usize
            } else {
                return Ok(Value::Array(vec![]));
            };

            let keys = if let Value::Array(arr) = &values[1] {
                arr
            } else {
                return Ok(Value::Array(vec![]));
            };

            let mut missing = Vec::new();
            let mut found = 0;

            for key in keys {
                if let Value::String(path) = key {
                    let value = VarOperator::get_nested_value(data, path);
                    if value == &Value::Null {
                        missing.push(Value::String(path.clone()));
                    } else {
                        found += 1;
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