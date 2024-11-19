use crate::operators::operator::Operator;
use crate::{Error, JsonLogic, JsonLogicResult};
use serde_json::Value;

pub struct EqualsOperator;

impl Operator for EqualsOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(arr) = args {
            if arr.len() != 2 {
                return Err(Error::InvalidArguments("== requires 2 arguments".into()));
            }
            
            let left = logic.apply(&arr[0], data)?;
            let right = logic.apply(&arr[1], data)?;
            
            // Convert numbers to strings if comparing with strings
            let result = match (&left, &right) {
                (Value::Number(n), Value::String(s)) => {
                    n.to_string() == *s
                },
                (Value::String(s), Value::Number(n)) => {
                    *s == n.to_string()
                },
                _ => left == right
            };
            
            Ok(Value::Bool(result))
        } else {
            Err(Error::InvalidArguments("== requires array argument".into()))
        }
    }
}


pub struct StrictEqualsOperator;

impl Operator for StrictEqualsOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(arr) = args {
            if arr.len() != 2 {
                return Err(Error::InvalidArguments("=== requires 2 arguments".into()));
            }
            
            let left = logic.apply(&arr[0], data)?;
            let right = logic.apply(&arr[1], data)?;
            
            // Strict equality - type and value must match
            let result = match (&left, &right) {
                (Value::Number(n1), Value::Number(n2)) => n1 == n2,
                (Value::String(s1), Value::String(s2)) => s1 == s2,
                (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
                (Value::Null, Value::Null) => true,
                _ => false // Different types are never strictly equal
            };
            
            Ok(Value::Bool(result))
        } else {
            Err(Error::InvalidArguments("=== requires array argument".into()))
        }
    }
}

pub struct NotEqualsOperator;

impl Operator for NotEqualsOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(arr) = args {
            if arr.len() != 2 {
                return Err(Error::InvalidArguments("!= requires 2 arguments".into()));
            }
            
            let left = logic.apply(&arr[0], data)?;
            let right = logic.apply(&arr[1], data)?;
            
            // Use same logic as EqualsOperator but negate the result
            let result = match (&left, &right) {
                (Value::Number(n), Value::String(s)) => {
                    n.to_string() != *s
                },
                (Value::String(s), Value::Number(n)) => {
                    *s != n.to_string()
                },
                _ => left != right
            };
            
            Ok(Value::Bool(result))
        } else {
            Err(Error::InvalidArguments("!= requires array argument".into()))
        }
    }
}

pub struct StrictNotEqualsOperator;

impl Operator for StrictNotEqualsOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(arr) = args {
            if arr.len() != 2 {
                return Err(Error::InvalidArguments("!== requires 2 arguments".into()));
            }
            
            let left = logic.apply(&arr[0], data)?;
            let right = logic.apply(&arr[1], data)?;
            
            // Negate strict equality comparison
            let result = match (&left, &right) {
                (Value::Number(n1), Value::Number(n2)) => n1 != n2,
                (Value::String(s1), Value::String(s2)) => s1 != s2,
                (Value::Bool(b1), Value::Bool(b2)) => b1 != b2,
                (Value::Null, Value::Null) => false,
                _ => true // Different types are never strictly equal
            };
            
            Ok(Value::Bool(result))
        } else {
            Err(Error::InvalidArguments("!== requires array argument".into()))
        }
    }
}


pub struct GreaterThanOperator;

impl Operator for GreaterThanOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(arr) = args {
            if arr.len() != 2 {
                return Err(Error::InvalidArguments("> requires 2 arguments".into()));
            }
            
            let left = logic.apply(&arr[0], data)?;
            let right = logic.apply(&arr[1], data)?;
            
            let result = match (&left, &right) {
                (Value::Number(n1), Value::Number(n2)) => n1.as_f64() > n2.as_f64(),
                (Value::String(s), Value::Number(n)) => s.parse::<f64>().unwrap_or(0.0) > n.as_f64().unwrap(),
                (Value::Number(n), Value::String(s)) => n.as_f64().unwrap() > s.parse::<f64>().unwrap_or(0.0),
                _ => false
            };
            
            Ok(Value::Bool(result))
        } else {
            Err(Error::InvalidArguments("> requires array argument".into()))
        }
    }
}

pub struct GreaterThanEqualOperator; 

impl Operator for GreaterThanEqualOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(arr) = args {
            if arr.len() != 2 {
                return Err(Error::InvalidArguments(">= requires 2 arguments".into()));
            }
            
            let left = logic.apply(&arr[0], data)?;
            let right = logic.apply(&arr[1], data)?;
            
            let result = match (&left, &right) {
                (Value::Number(n1), Value::Number(n2)) => n1.as_f64() >= n2.as_f64(),
                (Value::String(s), Value::Number(n)) => s.parse::<f64>().unwrap_or(0.0) >= n.as_f64().unwrap(),
                (Value::Number(n), Value::String(s)) => n.as_f64().unwrap() >= s.parse::<f64>().unwrap_or(0.0),
                _ => false
            };
            
            Ok(Value::Bool(result))
        } else {
            Err(Error::InvalidArguments(">= requires array argument".into()))
        }
    }
}

pub struct LessThanOperator;

impl Operator for LessThanOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(arr) = args {
            if arr.len() < 2 {
                return Err(Error::InvalidArguments("< requires at least 2 arguments".into()));
            }
            
            // Check if all elements form an ascending chain
            let mut values = Vec::new();
            for arg in arr {
                values.push(logic.apply(arg, data)?);
            }
            
            let mut ascending = true;
            for i in 1..values.len() {
                let prev = &values[i-1];
                let curr = &values[i];
                
                let is_less = match (prev, curr) {
                    (Value::Number(n1), Value::Number(n2)) => n1.as_f64() < n2.as_f64(),
                    (Value::String(s), Value::Number(n)) => s.parse::<f64>().unwrap_or(0.0) < n.as_f64().unwrap(),
                    (Value::Number(n), Value::String(s)) => n.as_f64().unwrap() < s.parse::<f64>().unwrap_or(0.0),
                    _ => false
                };
                
                if !is_less {
                    ascending = false;
                    break;
                }
            }
            
            Ok(Value::Bool(ascending))
        } else {
            Err(Error::InvalidArguments("< requires array argument".into()))
        }
    }
}

pub struct LessThanEqualOperator;

impl Operator for LessThanEqualOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(arr) = args {
            if arr.len() < 2 {
                return Err(Error::InvalidArguments("<= requires at least 2 arguments".into()));
            }
            
            let mut values = Vec::new();
            for arg in arr {
                values.push(logic.apply(arg, data)?);
            }
            
            let mut ascending = true;
            for i in 1..values.len() {
                let prev = &values[i-1];
                let curr = &values[i];
                
                let is_less_equal = match (prev, curr) {
                    (Value::Number(n1), Value::Number(n2)) => n1.as_f64() <= n2.as_f64(),
                    (Value::String(s), Value::Number(n)) => s.parse::<f64>().unwrap_or(0.0) <= n.as_f64().unwrap(),
                    (Value::Number(n), Value::String(s)) => n.as_f64().unwrap() <= s.parse::<f64>().unwrap_or(0.0),
                    _ => false
                };
                
                if !is_less_equal {
                    ascending = false;
                    break;
                }
            }
            
            Ok(Value::Bool(ascending))
        } else {
            Err(Error::InvalidArguments("<= requires array argument".into()))
        }
    }
}

pub struct NotOperator;

impl Operator for NotOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let value = match args {
            Value::Array(arr) if arr.len() == 1 => logic.apply(&arr[0], data)?,
            Value::Array(_) => return Err(Error::InvalidArguments("! requires 1 argument".into())),
            _ => logic.apply(args, data)?
        };
        
        let result = !match value {
            Value::Bool(b) => b,
            Value::Number(n) => n.as_f64().unwrap() != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Null => false,
            Value::Array(arr) => !arr.is_empty(),
            Value::Object(obj) => !obj.is_empty(),
        };
        
        Ok(Value::Bool(result))
    }
}