use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

pub struct AndOperator;
pub struct OrOperator;
pub struct NotOperator;
pub struct DoubleBangOperator;


impl Operator for OrOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Bool(false));
        }

        for arg in args {
            let value = arg.apply(data)?;
            if is_truthy(&value) {
                return Ok(value);
            }
        }
        args.last().unwrap().apply(data)
    }
}

impl Operator for AndOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Bool(true));
        }

        let mut last_value = Value::Bool(true);
        for arg in args {
            let value = arg.apply(data)?;
            if !is_truthy(&value) {
                return Ok(value);
            }
            last_value = value;
        }
        Ok(last_value)
    }
}

impl Operator for NotOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 1 {
            return Err(Error::InvalidArguments("! requires 1 argument".to_string()));
        }

        let value = args[0].apply(data)?;
        Ok(Value::Bool(!is_truthy(&value)))
    }
}

impl Operator for DoubleBangOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 1 {
            return Err(Error::InvalidArguments("!! requires 1 argument".to_string()));
        }

        let value = args[0].apply(data)?;
        Ok(Value::Bool(is_truthy(&value)))
    }
}

/// Helper function to determine if a JSON value is truthy
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}