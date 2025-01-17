use serde_json::Value;
use crate::JsonLogicResult;
use super::{Rule, ValueCoercion};

#[derive(Debug, Clone)]
pub enum LogicType { And, Or, Not, DoubleBang }

pub struct LogicOperator;

impl LogicOperator {
    pub fn apply(&self, args: &[Rule], data: &Value, logic_type: LogicType) -> JsonLogicResult {
        match logic_type {
            LogicType::And => self.apply_and(args, data),
            LogicType::Or => self.apply_or(args, data),
            LogicType::Not => self.apply_not(args, data),
            LogicType::DoubleBang => self.apply_double_bang(args, data),
        }
    }

    fn apply_and(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
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

    fn apply_or(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
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

    fn apply_not(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args.len() {
            0 => Ok(Value::Bool(true)),
            _ => {
                let value = args[0].apply(data)?;
                Ok(Value::Bool(!value.coerce_to_bool()))
            }
        }
    }

    fn apply_double_bang(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args.len() {
            0 => Ok(Value::Bool(false)),
            _ => {
                let value = args[0].apply(data)?;
                Ok(Value::Bool(value.coerce_to_bool()))
            }
        }
    }
}