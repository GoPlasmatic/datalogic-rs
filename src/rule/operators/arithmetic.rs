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

#[derive(Debug, Clone, Copy)]
struct ArithmeticConfig {
    acc_default: Option<f64>,
    acc_single: f64,
    min_args: usize,
    operation: fn(f64, f64) -> Result<f64, Error>,
}

pub struct ArithmeticOperator;

impl ArithmeticOperator {
    pub fn apply<'a>(&self, arg: &ArgType, context: &Value, root: &Value, path: &str, op_type: &ArithmeticType) -> Result<Cow<'a, Value>, Error> {
        let config = match op_type {
            ArithmeticType::Add => ArithmeticConfig {
                acc_default: Some(0.0),
                acc_single: 0.0,
                min_args: 0,
                operation: |a, b| Ok(a + b),
            },
            ArithmeticType::Multiply => ArithmeticConfig {
                acc_default: Some(1.0),
                acc_single: 1.0,
                min_args: 0,
                operation: |a, b| Ok(a * b),
            },
            ArithmeticType::Subtract => ArithmeticConfig {
                acc_default: None,
                acc_single: 0.0,
                min_args: 1,
                operation: |a, b| Ok(a - b),
            },
            ArithmeticType::Divide => ArithmeticConfig {
                acc_default: None,
                acc_single: 1.0,
                min_args: 1,
                operation: |a, b| if b == 0.0 {
                    Err(Error::Custom("NaN".to_string()))
                } else {
                    Ok(a / b)
                },
            },
            ArithmeticType::Modulo => ArithmeticConfig {
                acc_default: None,
                acc_single: 1.0,
                min_args: 2,
                operation: |a, b| Ok(a % b),
            },
            ArithmeticType::Max => ArithmeticConfig {
                acc_default: Some(f64::NEG_INFINITY),
                acc_single: 1.0,
                min_args: 0,
                operation: |a, b| Ok(a.max(b)),
            },
            ArithmeticType::Min => ArithmeticConfig {
                acc_default: Some(f64::INFINITY),
                acc_single: 1.0,
                min_args: 0,
                operation: |a, b| Ok(a.min(b)),
            },
        };
        self.apply_simple(arg, context, root, path, config)
    }

    fn apply_simple<'a>(&self, arg: &ArgType, context: &Value, root: &Value, path: &str, config: ArithmeticConfig) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(context, root, path)?;
                if let Value::Array(ref arr) = *value {
                    if arr.len() < config.min_args {
                        return Err(Error::Custom("Invalid Arguments".to_string()));
                    } else if arr.len() == 1 {
                        let v = (config.operation)(config.acc_single, arr.first().unwrap().coerce_to_number()?)?;
                        return Ok(Cow::Owned(v.to_value()))
                    }
                    let mut acc: f64;
                    if config.acc_default.is_none() {
                        acc = arr.first().unwrap().coerce_to_number()?;
                        for v in arr.iter().skip(1) {
                            acc = (config.operation)(acc, v.coerce_to_number()?)?;
                        }
                    } else {
                        acc = config.acc_default.unwrap();
                        for v in arr {
                            acc = (config.operation)(acc, v.coerce_to_number()?)?;
                        }
                    }
                    Ok(Cow::Owned(acc.to_value()))
                } else if config.min_args <= 1 {
                    let v = (config.operation)(config.acc_single, value.coerce_to_number()?)?;
                    Ok(Cow::Owned(v.to_value()))
                } else {
                    Err(Error::Custom("Invalid Arguments".to_string()))
                }
            }
            ArgType::Multiple(rules) => {
                if rules.len() < config.min_args {
                    return Err(Error::Custom("Invalid Arguments".to_string()));
                } else if rules.len() == 1 {
                    let v = (config.operation)(config.acc_single, rules.first().unwrap().apply(context, root, path)?.coerce_to_number()?)?;
                    return Ok(Cow::Owned(v.to_value()));
                }
                let mut acc: f64;
                if config.acc_default.is_none() {
                    acc = rules.first().unwrap().apply(context, root, path)?.coerce_to_number()?;
                    for rule in rules.iter().skip(1) {
                        let value = rule.apply(context, root, path)?;
                        acc = (config.operation)(acc, value.coerce_to_number()?)?;
                        }
                } else {
                    acc = config.acc_default.unwrap();
                    for rule in rules {
                        let value = rule.apply(context, root, path)?;
                        acc = (config.operation)(acc, value.coerce_to_number()?)?;
                    }
                }
                Ok(Cow::Owned(acc.to_value()))
            }
        }
    }
}