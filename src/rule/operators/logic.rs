use serde_json::Value;
use crate::Error;
use super::{Rule, ValueCoercion};

pub struct AndOperator;
pub struct OrOperator;
pub struct NotOperator;
pub struct DoubleBangOperator;


impl OrOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => Ok(Value::Bool(false)),
            1 => args[0].apply(data),
            _ => {
                for arg in args {
                    let value = arg.apply(data)?;
                    if value.coerce_to_bool() {
                        return Ok(value);
                    }
                }
                args.last().unwrap().apply(data)
            }
        }
    }
}

impl AndOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => Ok(Value::Bool(true)),
            1 => args[0].apply(data),
            _ => {
                for arg in args {
                    let value = arg.apply(data)?;
                    if !value.coerce_to_bool() {
                        return Ok(value);
                    }
                }
                args.last().unwrap().apply(data)
            }
        }
    }
}

impl NotOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => Ok(Value::Bool(true)),
            1 => {
                let value = args[0].apply(data)?;
                Ok(Value::Bool(!value.coerce_to_bool()))
            },
            _ => {
                let value = args[0].apply(data)?;
                Ok(Value::Bool(!value.coerce_to_bool()))
            }
        }
    }
}

impl DoubleBangOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => Ok(Value::Bool(false)),
            _ => {
                let value = args[0].apply(data)?;
                Ok(Value::Bool(value.coerce_to_bool()))
            }
        }
    }
}