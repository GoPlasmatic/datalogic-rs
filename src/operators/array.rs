use crate::operators::operator::Operator;
use crate::{JsonLogic, JsonLogicResult};
use serde_json::{json, Value};

pub struct FilterOperator;

impl FilterOperator {
    fn validate_args(args: &Value) -> Option<(&Value, &Value)> {
        if let Value::Array(values) = args {
            if values.len() == 2 {
                return Some((&values[0], &values[1]));
            }
        }
        None
    }
}

impl Operator for FilterOperator {
    fn auto_traverse(&self) -> bool {
        false
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let (source, condition) = match Self::validate_args(args) {
            Some(args) => args,
            None => return Ok(Value::Array(Vec::new())),
        };
    
        let array = match logic.apply(source, data)? {
            Value::Array(arr) => arr,
            _ => return Ok(Value::Array(Vec::new())),
        };
        
        let mut result = Vec::with_capacity(array.len() / 2);
        
        for item in array {
            let condition_result = logic.apply(condition, &item)?;
            if crate::operators::logic::is_truthy(&condition_result) {
                result.push(item);
            }
        }

        Ok(Value::Array(result))
    }
}

pub struct MapOperator;

impl MapOperator {
    fn validate_args(args: &Value) -> Option<(&Value, &Value)> {
        if let Value::Array(values) = args {
            if values.len() == 2 {
                return Some((&values[0], &values[1]));
            }
        }
        None
    }
}

impl Operator for MapOperator {
    fn auto_traverse(&self) -> bool {
        false
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let (source, mapper) = match Self::validate_args(args) {
            Some(args) => args,
            None => return Ok(Value::Array(Vec::new())),
        };
    
        let array = match logic.apply(source, data)? {
            Value::Array(arr) => arr,
            _ => return Ok(Value::Array(Vec::new())),
        };
    
        let mut result = Vec::with_capacity(array.len());
        for item in array {
            result.push(logic.apply(mapper, &item)?);
        }
    
        Ok(Value::Array(result))
    }
}

pub struct ReduceOperator;

impl ReduceOperator {
    fn validate_args(args: &Value) -> Option<(&Value, &Value, &Value)> {
        if let Value::Array(values) = args {
            if values.len() == 3 {
                return Some((&values[0], &values[1], &values[2]));
            }
        }
        None
    }
}

impl Operator for ReduceOperator {
    fn auto_traverse(&self) -> bool {
        false
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let (source, reducer, initial) = match Self::validate_args(args) {
            Some(args) => args,
            None => return Ok(Value::Null),
        };

        let initial_value = logic.apply(initial, data)?;

        let array = match logic.apply(source, data)? {
            Value::Array(arr) if arr.is_empty() => return Ok(initial_value),
            Value::Array(arr) => arr,
            _ => return Ok(initial_value),
        };

        array.into_iter().try_fold(initial_value, |acc, current| {
            let context = json!({
                "current": current,
                "accumulator": acc
            });
            logic.apply(reducer, &context)
        })
    }
}

pub struct AllOperator;

impl AllOperator {
    fn validate_args(args: &Value) -> Option<(&Value, &Value)> {
        if let Value::Array(values) = args {
            if values.len() == 2 {
                return Some((&values[0], &values[1]));
            }
        }
        None
    }
}

impl Operator for AllOperator {
    fn auto_traverse(&self) -> bool {
        false
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let (source, condition) = match Self::validate_args(args) {
            Some(args) => args,
            None => return Ok(Value::Bool(false)),
        };

        let array = match logic.apply(source, data)? {
            Value::Array(arr) => arr,
            _ => return Ok(Value::Bool(false)),
        };

        if array.is_empty() {
            return Ok(Value::Bool(false));
        }

        let result = array.iter().all(|item| {
            logic.apply(condition, item)
                .map(|v| crate::operators::logic::is_truthy(&v))
                .unwrap_or(false)
        });
        Ok(Value::Bool(result))
    }
}

pub struct NoneOperator;

impl NoneOperator {
    fn validate_args(args: &Value) -> Option<(&Value, &Value)> {
        if let Value::Array(values) = args {
            if values.len() == 2 {
                return Some((&values[0], &values[1]));
            }
        }
        None
    }
}

impl Operator for NoneOperator {
    fn auto_traverse(&self) -> bool {
        false
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let (source, condition) = match Self::validate_args(args) {
            Some(args) => args,
            None => return Ok(Value::Bool(true)),
        };

        let array = match logic.apply(source, data)? {
            Value::Array(arr) if arr.is_empty() => return Ok(Value::Bool(true)),
            Value::Array(arr) => arr,
            _ => return Ok(Value::Bool(true)),
        };

        // Use any() for short-circuiting
        let result = !array.iter().any(|item| {
            logic.apply(condition, item)
                .map(|v| crate::operators::logic::is_truthy(&v))
                .unwrap_or(false)
        });

        Ok(Value::Bool(result))
    }
}

pub struct SomeOperator;

impl SomeOperator {
    fn validate_args(args: &Value) -> Option<(&Value, &Value)> {
        if let Value::Array(values) = args {
            if values.len() == 2 {
                return Some((&values[0], &values[1]));
            }
        }
        None
    }
}

impl Operator for SomeOperator {
    fn auto_traverse(&self) -> bool {
        false 
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let (source, condition) = match Self::validate_args(args) {
            Some(args) => args,
            None => return Ok(Value::Bool(false)),
        };

        let array = match logic.apply(source, data)? {
            Value::Array(arr) if arr.is_empty() => return Ok(Value::Bool(false)),
            Value::Array(arr) => arr,
            _ => return Ok(Value::Bool(false)),
        };

        // Use any() for short-circuiting
        let result = array.iter().any(|item| {
            logic.apply(condition, item)
                .map(|v| crate::operators::logic::is_truthy(&v))
                .unwrap_or(false)
        });

        Ok(Value::Bool(result))
    }
}

pub struct MergeOperator;

impl Operator for MergeOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let mut result = Vec::new();

        match args {
            Value::Array(values) => {
                for value in values {
                    let evaluated = logic.apply(value, data)?;
                    match evaluated {
                        Value::Array(arr) => result.extend(arr),
                        other => result.push(other),
                    }
                }
            },
            // Non-array arguments are converted to single-element arrays
            other => {
                let evaluated = logic.apply(other, data)?;
                result.push(evaluated);
            }
        }

        Ok(Value::Array(result))
    }
}