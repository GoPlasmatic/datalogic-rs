use serde_json::Value;
use crate::rule::ArgType;
use super::{Rule, ValueCoercion};
use crate::Error;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub enum LogicType { And, Or, Not, DoubleBang }

pub struct LogicOperator;

impl LogicOperator {
    pub fn apply<'a>(&self, args: &'a ArgType, data: &'a Value, logic_type: &LogicType) -> Result<Cow<'a, Value>, Error> {
        if let ArgType::Multiple(arg_arr) = args {
            match logic_type {
                LogicType::And => self.apply_and(arg_arr, data),
                LogicType::Or => self.apply_or(arg_arr, data),
                LogicType::Not => self.apply_not(arg_arr, data),
                LogicType::DoubleBang => self.apply_double_bang(arg_arr, data),
            }
        } else if let ArgType::Unary(arg) = args {
            match logic_type {
                LogicType::Not => self.apply_not(std::slice::from_ref(arg), data),
                LogicType::DoubleBang => self.apply_double_bang(std::slice::from_ref(arg), data),
                _ => Err(Error::Custom("Invalid Arguments".into()))
            }
        } else {
            Err(Error::Custom("Invalid Arguments".into()))
        }
    }

    #[inline(always)]
    fn apply_and<'a>(&self, args: &'a [Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 => Ok(Cow::Owned(Value::Bool(false))),
            1 => args[0].apply(data),
            _ => {
                for arg in &args[..args.len()-1] {
                    let value = arg.apply(data)?;
                    if !value.coerce_to_bool() {
                        return Ok(value);
                    }
                }
                args.last().unwrap().apply(data)
            }
        }
    }

    #[inline(always)]
    fn apply_or<'a>(&self, args: &'a [Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 => Ok(Cow::Owned(Value::Bool(false))),
            1 => args[0].apply(data),
            _ => {
                for arg in &args[..args.len()-1] {
                    let value = arg.apply(data)?;
                    if value.coerce_to_bool() {
                        return Ok(value);
                    }
                }
                args.last().unwrap().apply(data)
            }
        }
    }

    #[inline(always)]
    fn apply_not<'a>(&self, args: &[Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 => Ok(Cow::Owned(Value::Bool(true))),
            _ => {
                let value = args[0].apply(data)?;
                Ok(Cow::Owned(Value::Bool(!value.coerce_to_bool())))
            }
        }
    }

    #[inline(always)]
    fn apply_double_bang<'a>(&self, args: &[Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 => Ok(Cow::Owned(Value::Bool(false))),
            _ => {
                let value = args[0].apply(data)?;
                Ok(Cow::Owned(Value::Bool(value.coerce_to_bool())))
            }
        }
    }
}