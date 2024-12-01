// src/operators/array_ops.rs
use crate::operators::operator::Operator;
use crate::{JsonLogic, JsonLogicResult};
use serde_json::{json, Value};

pub struct FilterOperator;
pub struct MapOperator;
pub struct ReduceOperator;
pub struct AllOperator;
pub struct NoneOperator;
pub struct SomeOperator;

impl Operator for FilterOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Ok(Value::Array(vec![]));
            }

            // Get the array to filter from first argument
            let arr = match logic.apply(&values[0], data)? {
                Value::Array(a) => a,
                _ => return Ok(Value::Array(vec![])),
            };

            let mut result = Vec::new();
            
            // Apply filter predicate to each item
            for item in arr {
                // Test condition against current item
                let test = logic.apply(&values[1], &item)?;
                if crate::operators::logic::is_truthy(&test) {
                    result.push(item.clone());
                }
            }

            Ok(Value::Array(result))
        } else {
            Ok(Value::Array(vec![]))
        }
    }
}

impl Operator for MapOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Ok(Value::Array(vec![]));
            }

            let arr = match logic.apply(&values[0], data)? {
                Value::Array(a) => a,
                _ => return Ok(Value::Array(vec![])),
            };

            let mut result = Vec::new();
            for item in arr {
                let mapped = logic.apply(&values[1], &item)?;
                result.push(mapped);
            }

            Ok(Value::Array(result))
        } else {
            Ok(Value::Array(vec![]))
        }
    }
}

impl Operator for ReduceOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 3 {
                return Ok(Value::Number(0.into()));
            }

            let arr = match logic.apply(&values[0], data)? {
                Value::Array(a) => a,
                _ => return Ok(logic.apply(&values[2], data)?),
            };

            let mut accumulator = logic.apply(&values[2], data)?;
            
            for current in arr {
                let scope = json!({
                    "current": current,
                    "accumulator": accumulator
                });
                accumulator = logic.apply(&values[1], &scope)?;
            }

            Ok(accumulator)
        } else {
            Ok(Value::Number(0.into()))
        }
    }
}

impl Operator for AllOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Ok(Value::Bool(false));
            }

            let arr = match logic.apply(&values[0], data)? {
                Value::Array(a) => a,
                _ => return Ok(Value::Bool(false)),
            };

            if arr.is_empty() {
                return Ok(Value::Bool(false));
            }

            for item in arr {
                let test = logic.apply(&values[1], &item)?;
                if !crate::operators::logic::is_truthy(&test) {
                    return Ok(Value::Bool(false));
                }
            }

            Ok(Value::Bool(true))
        } else {
            Ok(Value::Bool(false))
        }
    }
}

impl Operator for NoneOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Ok(Value::Bool(true));
            }

            let arr = match logic.apply(&values[0], data)? {
                Value::Array(a) => a,
                _ => return Ok(Value::Bool(true)),
            };

            for item in arr {
                let test = logic.apply(&values[1], &item)?;
                if crate::operators::logic::is_truthy(&test) {
                    return Ok(Value::Bool(false));
                }
            }

            Ok(Value::Bool(true))
        } else {
            Ok(Value::Bool(true))
        }
    }
}

impl Operator for SomeOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Ok(Value::Bool(false));
            }

            let arr = match logic.apply(&values[0], data)? {
                Value::Array(a) => a,
                _ => return Ok(Value::Bool(false)),
            };

            for item in arr {
                let test = logic.apply(&values[1], &item)?;
                if crate::operators::logic::is_truthy(&test) {
                    return Ok(Value::Bool(true));
                }
            }

            Ok(Value::Bool(false))
        } else {
            Ok(Value::Bool(false))
        }
    }
}