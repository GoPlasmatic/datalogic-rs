use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

pub struct AndOperator;
pub struct OrOperator;
pub struct NotOperator;
pub struct DoubleBangOperator;


#[inline]
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,  // Most common case first
        Value::Null => false,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

impl Operator for OrOperator {
    #[inline]
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => return Ok(Value::Bool(false)),
            1 => return args[0].apply(data),
            _ => {
                for arg in args {
                    let value = arg.apply(data)?;
                    if is_truthy(&value) {
                        return Ok(value);
                    }
                }
                args.last().unwrap().apply(data)
            }
        }
    }
}

impl Operator for AndOperator {
    #[inline]
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => return Ok(Value::Bool(true)),
            1 => return args[0].apply(data),
            _ => {
                for arg in args {
                    let value = arg.apply(data)?;
                    if !is_truthy(&value) {
                        return Ok(value);
                    }
                }
                args.last().unwrap().apply(data)
            }
        }
    }
}

impl Operator for NotOperator {
    #[inline]
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => return Ok(Value::Bool(true)),
            1 => {
                let value = args[0].apply(data)?;
                Ok(Value::Bool(!is_truthy(&value)))
            },
            _ => {
                let value = args[0].apply(data)?;
                Ok(Value::Bool(!is_truthy(&value)))
            }
        }
    }
}

impl Operator for DoubleBangOperator {
    #[inline]
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => Ok(Value::Bool(false)),
            _ => {
                let value = args[0].apply(data)?;
                Ok(Value::Bool(is_truthy(&value)))
            }
        }
    }
}