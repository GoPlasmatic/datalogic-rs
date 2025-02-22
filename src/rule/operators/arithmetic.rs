use super::{ValueCoercion, ValueConvert};
use crate::rule::ArgType;
use crate::Error;
use serde_json::Value;
use core::f64;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy)]
pub enum ArithmeticType {
    Add,
    Multiply,
    Subtract,
    Divide,
    Modulo,
    Max,
    Min,
}

pub struct ArithmeticOperator;

impl ArithmeticOperator {
    pub fn apply<'a>(&self, arg: &ArgType, data: &'a Value, op_type: &ArithmeticType) -> Result<Cow<'a, Value>, Error> {
        match op_type {
            ArithmeticType::Add => self.apply_simple(arg, data, Some(0.0), 0.0, |a, b| Ok(a + b), 0),
            ArithmeticType::Multiply => self.apply_simple(arg, data, Some(1.0), 1.0, |a, b| Ok(a * b), 0),
            ArithmeticType::Subtract => self.apply_simple(arg, data, None, 0.0, |a, b| Ok(a - b), 1),
            ArithmeticType::Divide => self.apply_simple(arg, data, None, 1.0, |a, b| {
                if b == 0.0 {
                    Err(Error::Custom("NaN".to_string()))
                } else {
                    Ok(a / b)
                }
            }, 1),
            ArithmeticType::Modulo => self.apply_simple(arg, data, None, 1.0, |a, b| Ok(a % b), 2),
            ArithmeticType::Max => self.apply_simple(arg, data, Some(f64::NEG_INFINITY), 1.0, |a, b| Ok(a.max(b)), 0),
            ArithmeticType::Min => self.apply_simple(arg, data, Some(f64::INFINITY), 1.0, |a, b| Ok(a.min(b)), 0),
        }
    }

    fn apply_simple<'a>(&self, arg: &ArgType, data: &'a Value, acc_default: Option<f64>, acc_single: f64, simple_func: fn(f64, f64) -> Result<f64, Error>, min_args: usize) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                if let Value::Array(ref arr) = *value {
                    if arr.len() < min_args {
                        return Err(Error::Custom("Invalid Arguments".to_string()));
                    } else if arr.len() == 1 {
                        let v = simple_func(acc_single, arr.first().unwrap().coerce_to_number()?)?;
                        return Ok(Cow::Owned(v.to_value()))
                    }
                    let mut acc: f64;
                    if acc_default.is_none() {
                        acc = arr.first().unwrap().coerce_to_number()?;
                        for v in arr.iter().skip(1) {
                            acc = simple_func(acc, v.coerce_to_number()?)?;
                        }
                    } else {
                        acc = acc_default.unwrap();
                        for v in arr {
                            acc = simple_func(acc, v.coerce_to_number()?)?;
                        }
                    }
                    Ok(Cow::Owned(acc.to_value()))
                } else if min_args <= 1 {
                    let v = simple_func(acc_single, value.coerce_to_number()?)?;
                    Ok(Cow::Owned(v.to_value()))
                } else {
                    Err(Error::Custom("Invalid Arguments".to_string()))
                }
            }
            ArgType::Multiple(rules) => {
                if rules.len() < min_args {
                    return Err(Error::Custom("Invalid Arguments".to_string()));
                } else if rules.len() == 1 {
                    let v = simple_func(acc_single, rules.first().unwrap().apply(data)?.coerce_to_number()?)?;
                    return Ok(Cow::Owned(v.to_value()));
                }
                let mut acc: f64;
                if acc_default.is_none() {
                    acc = rules.first().unwrap().apply(data)?.coerce_to_number()?;
                    for rule in rules.iter().skip(1) {
                        let value = rule.apply(data)?;
                        acc = simple_func(acc, value.coerce_to_number()?)?;
                        }
                } else {
                    acc = acc_default.unwrap();
                    for rule in rules {
                        let value = rule.apply(data)?;
                        acc = simple_func(acc, value.coerce_to_number()?)?;
                    }
                }
                Ok(Cow::Owned(acc.to_value()))
            }
        }
    }
}