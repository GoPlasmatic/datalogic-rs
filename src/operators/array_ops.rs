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

    fn get_array(logic: &JsonLogic, source: &Value, data: &Value) -> JsonLogicResult {
        match logic.apply(source, data)? {
            Value::Array(arr) => Ok(Value::Array(arr)),
            _ => Ok(Value::Array(vec![]))
        }
    }

    fn test_condition(logic: &JsonLogic, condition: &Value, item: &Value) -> JsonLogicResult {
        let result = logic.apply(condition, item)?;
        Ok(Value::Bool(crate::operators::logic::is_truthy(&result)))
    }
}

impl Operator for FilterOperator {
    fn auto_traverse(&self) -> bool {
        false
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let (source, condition) = match Self::validate_args(args) {
            Some(args) => args,
            None => return Ok(Value::Array(vec![]))
        };

        let array = Self::get_array(logic, source, data)?;
        
        if let Value::Array(items) = array {
            let result = items
                .into_iter()
                .filter(|item| {
                    Self::test_condition(logic, condition, item)
                        .map(|v| crate::operators::logic::is_truthy(&v))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            
            Ok(Value::Array(result))
        } else {
            Ok(Value::Array(vec![]))
        }
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
        result.extend(
            array
                .into_iter()
                .map(|item| logic.apply(mapper, &item))
                .collect::<Result<Vec<_>, _>>()?
        );

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

        // Evaluate initial value in data context
        let initial_value = logic.apply(initial, data)?;

        // Get array from source
        let array = match logic.apply(source, data)? {
            Value::Array(arr) => arr,
            _ => return Ok(initial_value),
        };

        // Handle empty array case
        if array.is_empty() {
            return Ok(initial_value);
        }

        // Fold with proper context for accumulator and current
        array.into_iter().fold(Ok(initial_value), |acc, current| {
            let accumulator = acc?;
            let context = json!({
                "current": current,
                "accumulator": accumulator
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

        // Empty array returns false
        if array.is_empty() {
            return Ok(Value::Bool(false));
        }

        let result = array
            .iter()
            .all(|item| {
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
            Value::Array(arr) => arr,
            _ => return Ok(Value::Bool(true)),
        };

        if array.is_empty() {
            return Ok(Value::Bool(true));
        }

        let result = !array
            .iter()
            .any(|item| {
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
            Value::Array(arr) => arr,
            _ => return Ok(Value::Bool(false)),
        };

        if array.is_empty() {
            return Ok(Value::Bool(false));
        }

        let result = array
            .iter()
            .any(|item| {
                logic.apply(condition, item)
                    .map(|v| crate::operators::logic::is_truthy(&v))
                    .unwrap_or(false)
            });

        Ok(Value::Bool(result))
    }
}