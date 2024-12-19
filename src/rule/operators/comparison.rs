use serde_json::Value;
use crate::Error;
use super::{Operator, Rule, ValueCoercion};

pub struct EqualsOperator;
pub struct StrictEqualsOperator;
pub struct NotEqualsOperator;
pub struct StrictNotEqualsOperator;
pub struct GreaterThanOperator;
pub struct LessThanOperator;
pub struct GreaterThanEqualOperator;
pub struct LessThanEqualOperator;


impl Operator for EqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("== requires 2 arguments".to_string()));
        }
        let left = args[0].apply(data)?;
        let right = args[1].apply(data)?;
        
        Ok(Value::Bool(left.coerce_to_number() == right.coerce_to_number()))
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
                    return Ok(Value::Bool(left.coerce_to_number() != right.coerce_to_number()));
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
        Ok(Value::Bool(left.coerce_to_number() > right.coerce_to_number()))
    }
}

impl Operator for LessThanOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::InvalidArguments("< requires at least 2 arguments".to_string()));
        }
        let mut current = args[0].apply(data)?.coerce_to_number();
        for arg in &args[1..] {
            let next = arg.apply(data)?.coerce_to_number();
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
        let mut current = args[0].apply(data)?.coerce_to_number();
        for arg in &args[1..] {
            let next = arg.apply(data)?.coerce_to_number();
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
        let mut current = args[0].apply(data)?.coerce_to_number();
        for arg in &args[1..] {
            let next = arg.apply(data)?.coerce_to_number();
            if current < next {
                return Ok(Value::Bool(false));
            }
            current = next;
        }
        Ok(Value::Bool(true))
    }
}