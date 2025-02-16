use super::{ValueCoercion, ValueConvert};
use crate::rule::ArgType;
use crate::Error;
use crate::JsonLogicResult;
use serde_json::Value;

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
    pub fn apply(&self, arg: &ArgType, data: &Value, op_type: &ArithmeticType) -> JsonLogicResult {
        match op_type {
            ArithmeticType::Add => self.apply_add(arg, data),
            ArithmeticType::Multiply => self.apply_multiply(arg, data),
            ArithmeticType::Subtract => self.apply_subtract(arg, data),
            ArithmeticType::Divide => self.apply_divide(arg, data),
            ArithmeticType::Modulo => self.apply_modulo(arg, data),
            ArithmeticType::Max => self.apply_max(arg, data),
            ArithmeticType::Min => self.apply_min(arg, data),
        }
    }

    #[inline(always)]
    fn apply_add(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => arr
                        .iter()
                        .try_fold(0.0, |acc, v| Ok(acc + v.coerce_to_number()?))
                        .map(|sum| sum.to_value()),
                    _ => Ok(value.coerce_to_number()?.to_value()),
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Ok(Value::Number(0.into())),
                _ => {
                    let sum = rules.iter().try_fold(0.0, |acc, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(acc + num)
                    })?;
                    Ok(sum.to_value())
                }
            },
        }
    }

    #[inline(always)]
    fn apply_multiply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => arr
                        .iter()
                        .try_fold(1.0, |acc, v| {
                            let num = v.coerce_to_number()?;
                            Ok(acc * num)
                        })
                        .map(|product| product.to_value()),
                    _ => {
                        let num = value.coerce_to_number()?;
                        Ok(num.to_value())
                    }
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Ok(Value::Number(1.into())),
                _ => rules
                    .iter()
                    .try_fold(1.0, |acc, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(acc * num)
                    })
                    .map(|product| product.to_value()),
            },
        }
    }

    #[inline(always)]
    fn apply_subtract(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Err(Error::Custom("Invalid Arguments".to_string()));
                        } else if arr.len() == 1 {
                            let num = arr[0].coerce_to_number()?;
                            return Ok((-num).to_value());
                        }

                        let mut iter = arr.iter();
                        // Get first number
                        let first = iter
                            .next()
                            .ok_or_else(|| {
                                Error::InvalidArguments(
                                    "Subtract operation requires at least one argument".to_string(),
                                )
                            })?
                            .coerce_to_number()?;

                        // Subtract remaining numbers
                        let result = iter.try_fold(first, |acc, v| {
                            let num = v.coerce_to_number()?;
                            Ok(acc - num)
                        })?;

                        Ok(result.to_value())
                    }
                    _ => {
                        let num = value.coerce_to_number()?;
                        Ok((-num).to_value())
                    }
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Err(Error::Custom("Invalid Arguments".to_string())),
                1 => {
                    let value = rules[0].apply(data)?;
                    let num = value.coerce_to_number()?;
                    Ok((-num).to_value())
                }
                _ => {
                    let first = rules[0].apply(data)?.coerce_to_number()?;

                    let result = rules.iter().skip(1).try_fold(first, |acc, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(acc - num)
                    })?;

                    Ok(result.to_value())
                }
            },
        }
    }

    #[inline(always)]
    fn apply_divide(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Err(Error::Custom("Invalid Arguments".to_string()));
                        } else if arr.len() == 1 {
                            let num = arr[0].coerce_to_number()?;
                            if num == 0.0 {
                                return Err(Error::Custom("NaN".to_string()));
                            }
                            return Ok((1.0 / num).to_value());
                        }

                        let mut iter = arr.iter();
                        // Get first number
                        let first = iter
                            .next()
                            .ok_or_else(|| {
                                Error::InvalidArguments(
                                    "Division requires at least one argument".to_string(),
                                )
                            })?
                            .coerce_to_number()?;

                        // Multiply remaining numbers
                        let rest = iter.try_fold(1.0, |acc, v| {
                            let num = v.coerce_to_number()?;
                            if num == 0.0 {
                                Err(Error::Custom("NaN".to_string()))
                            } else {
                                Ok(acc * num)
                            }
                        })?;

                        let result = first / rest;
                        Ok(result.to_value())
                    }
                    _ => {
                        let num = value.coerce_to_number()?;
                        if num == 0.0 {
                            Err(Error::Custom("NaN".to_string()))
                        } else {
                            let result = 1.0 / num;
                            Ok(result.to_value())
                        }
                    }
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Err(Error::Custom("Invalid Arguments".to_string())),
                1 => {
                    let value = rules[0].apply(data)?;
                    let num = value.coerce_to_number()?;
                    if num == 0.0 {
                        Err(Error::Custom("NaN".to_string()))
                    } else {
                        let result = 1.0 / num;
                        Ok(result.to_value())
                    }
                }
                _ => {
                    let first = rules[0].apply(data)?.coerce_to_number()?;

                    let rest = rules.iter().skip(1).try_fold(1.0, |acc, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        if num == 0.0 {
                            Err(Error::Custom("NaN".to_string()))
                        } else {
                            Ok(acc * num)
                        }
                    })?;

                    let result = first / rest;
                    Ok(result.to_value())
                }
            },
        }
    }

    #[inline(always)]
    fn apply_modulo(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(_) => Err(Error::Custom("Invalid Arguments".to_string())),
            ArgType::Multiple(rules) => {
                match rules.len() {
                    0 | 1 => {
                        Err(Error::Custom("Invalid Arguments".to_string()))
                    }
                    _ => {
                        let first = rules[0].apply(data)?.coerce_to_number()?;

                        let rest = rules.iter().skip(1).try_fold(first, |acc, rule| {
                            let value = rule.apply(data)?;
                            let num = value.coerce_to_number()?;
                            if num == 0.0 {
                                return Err(Error::Custom("NaN".to_string()));
                            }
                            Ok(acc % num)
                        })?;

                        Ok(rest.to_value())
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn apply_max(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Ok(Value::Null);
                        }

                        arr.iter()
                            .try_fold(f64::NEG_INFINITY, |max, v| {
                                let num = v.coerce_to_number()?;
                                Ok(max.max(num))
                            })
                            .map(|max| max.to_value())
                    }
                    _ => value.coerce_to_number().map(|num| num.to_value()),
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Ok(Value::Null),
                _ => rules
                    .iter()
                    .try_fold(f64::NEG_INFINITY, |max, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(max.max(num))
                    })
                    .map(|max| max.to_value()),
            },
        }
    }

    #[inline(always)]
    fn apply_min(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Ok(Value::Null);
                        }

                        arr.iter()
                            .try_fold(f64::INFINITY, |min, v| {
                                let num = v.coerce_to_number()?;
                                Ok(min.min(num))
                            })
                            .map(|min| min.to_value())
                    }
                    _ => value.coerce_to_number().map(|num| num.to_value()),
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Ok(Value::Null),
                _ => rules
                    .iter()
                    .try_fold(f64::INFINITY, |min, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(min.min(num))
                    })
                    .map(|min| min.to_value()),
            },
        }
    }
}
