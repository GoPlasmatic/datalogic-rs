use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

pub struct EqualsOperator;
pub struct StrictEqualsOperator;
pub struct NotEqualsOperator;
pub struct StrictNotEqualsOperator;
pub struct GreaterThanOperator;
pub struct LessThanOperator;
pub struct GreaterThanEqualOperator;
pub struct LessThanEqualOperator;

#[inline]
fn to_number(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        Value::Bool(true) => 1.0,
        Value::Bool(false) => 0.0,
        _ => 0.0,
    }
}

#[inline]
fn check_string_number(left: &Value, right: &Value) -> Option<bool> {
    match (left, right) {
        (Value::String(s), Value::Number(_)) | (Value::Number(_), Value::String(s)) => {
            s.parse::<f64>().ok().map(|_| true)
        },
        _ => None
    }
}

impl Operator for EqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("== requires 2 arguments".to_string()));
        }
        let left = args[0].apply(data)?;
        let right = args[1].apply(data)?;
        
        Ok(Value::Bool(if check_string_number(&left, &right).is_some() {
            to_number(&left) == to_number(&right)
        } else {
            left == right
        }))
    }
}

impl Operator for StrictEqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("=== requires 2 arguments".to_string()));
        }
        let left = args[0].apply(data)?;
        let right = args[1].apply(data)?;
        Ok(Value::Bool(std::mem::discriminant(&left) == std::mem::discriminant(&right) && left == right))
    }
}

impl Operator for NotEqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::InvalidArguments("!= requires at least 2 arguments".to_string()));
        }
        let left = args[0].apply(data)?;
        let right = args[1].apply(data)?;
        
        match (&left, &right) {
            (Value::String(s), Value::Number(_)) | (Value::Number(_), Value::String(s)) => {
                if s.parse::<f64>().is_ok() {
                    return Ok(Value::Bool(to_number(&left) != to_number(&right)));
                }
            },
            _ => {}
        }
        Ok(Value::Bool(left != right))
    }
}

impl Operator for StrictNotEqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::InvalidArguments("!== requires at least 2 arguments".to_string()));
        }
        let left = args[0].apply(data)?;
        let right = args[1].apply(data)?;
        Ok(Value::Bool(std::mem::discriminant(&left) != std::mem::discriminant(&right) || left != right))
    }
}

impl Operator for GreaterThanOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::InvalidArguments("> requires at least 2 arguments".to_string()));
        }
        let left = args[0].apply(data)?;
        let right = args[1].apply(data)?;
        Ok(Value::Bool(to_number(&left) > to_number(&right)))
    }
}

impl Operator for LessThanOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::InvalidArguments("< requires at least 2 arguments".to_string()));
        }
        let mut current = to_number(&args[0].apply(data)?);
        for arg in &args[1..] {
            let next = to_number(&arg.apply(data)?);
            if current >= next {
                return Ok(Value::Bool(false));
            }
            current = next;
        }
        Ok(Value::Bool(true))
    }
}

impl Operator for LessThanEqualOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::InvalidArguments("<= requires at least 2 arguments".to_string()));
        }
        let mut current = to_number(&args[0].apply(data)?);
        for arg in &args[1..] {
            let next = to_number(&arg.apply(data)?);
            if current > next {
                return Ok(Value::Bool(false));
            }
            current = next;
        }
        Ok(Value::Bool(true))
    }
}

impl Operator for GreaterThanEqualOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::InvalidArguments(">= requires at least 2 arguments".to_string()));
        }
        let mut current = to_number(&args[0].apply(data)?);
        for arg in &args[1..] {
            let next = to_number(&arg.apply(data)?);
            if current < next {
                return Ok(Value::Bool(false));
            }
            current = next;
        }
        Ok(Value::Bool(true))
    }
}