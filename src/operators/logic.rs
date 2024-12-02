use crate::operators::operator::Operator;
use crate::{Error, JsonLogic, JsonLogicResult};
use serde_json::Value;

pub struct OrOperator;

impl Operator for OrOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            // If no arguments, return false
            if values.is_empty() {
                return Ok(Value::Bool(false));
            }

            for value in values {
                let result = logic.apply(value, data)?;
                // Return first truthy value
                if is_truthy(&result) {
                    return Ok(result);
                }
            }
            
            // If no truthy values found, return last value or false
            Ok(values.last()
                .map(|v| logic.apply(v, data))
                .transpose()?
                .unwrap_or(Value::Bool(false)))
        } else {
            Err(Error::InvalidArguments("or requires array argument".into()))
        }
    }
}

// Helper function to determine truthiness according to JSONLogic rules
pub(crate) fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
    }
}

pub struct AndOperator;

impl Operator for AndOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            // If no arguments, return true
            if values.is_empty() {
                return Ok(Value::Bool(true));
            }

            for value in values {
                let result = logic.apply(value, data)?;
                // Return first falsy value
                if !is_truthy(&result) {
                    return Ok(result);
                }
            }
            
            // If all values are truthy, return last value
            Ok(values.last()
                .map(|v| logic.apply(v, data))
                .transpose()?
                .unwrap_or(Value::Bool(true)))
        } else {
            Err(Error::InvalidArguments("and requires array argument".into()))
        }
    }
}


pub struct TernaryOperator;

impl Operator for TernaryOperator {
    fn auto_traverse(&self) -> bool {
        false // Prevent double evaluation
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        match args {
            Value::Array(arr) if arr.len() == 3 => {
                let condition = logic.apply(&arr[0], data)?;
                
                if crate::operators::logic::is_truthy(&condition) {
                    logic.apply(&arr[1], data)
                } else {
                    logic.apply(&arr[2], data)
                }
            },
            _ => Ok(Value::Null)
        }
    }
}

pub struct IfOperator;

impl Operator for IfOperator {
    fn auto_traverse(&self) -> bool {
        false
    }

    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.is_empty() {
                return Ok(Value::Null);
            }

            // Single value case - return the value itself
            if values.len() == 1 {
                return logic.apply(&values[0], data);
            }

            // Handle if/then/elseif/then chains
            let mut i = 0;
            while i < values.len() {
                // Get condition
                let condition = logic.apply(&values[i], data)?;
                
                if is_truthy(&condition) {
                    // If condition is true and we have a next value, return it
                    return if i + 1 < values.len() {
                        logic.apply(&values[i + 1], data)
                    } else {
                        // If no next value, return condition itself
                        Ok(condition)
                    };
                }
                
                // Skip condition and its value, move to next pair
                i += 2;
            }

            // If we've exhausted all conditions and have one value left, it's the else
            if i == values.len() - 1 {
                logic.apply(&values[i], data)
            } else {
                Ok(Value::Null)
            }
        } else {
            Err(Error::InvalidArguments("if requires array argument".into()))
        }
    }
}

pub struct DoubleBangOperator;

impl Operator for DoubleBangOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let value = match args {
            Value::Array(arr) if arr.len() == 1 => logic.apply(&arr[0], data)?,
            value => logic.apply(value, data)?,
        };

        // Convert to boolean using is_truthy
        Ok(Value::Bool(is_truthy(&value)))
    }
}
