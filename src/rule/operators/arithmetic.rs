use super::{ValueCoercion, ValueConvert};
use crate::rule::ArgType;
use crate::Error;
use serde_json::Value;
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
            ArithmeticType::Add => self.apply_add(arg, data),
            ArithmeticType::Multiply => self.apply_multiply(arg, data),
            ArithmeticType::Subtract => self.apply_subtract(arg, data),
            ArithmeticType::Divide => self.apply_divide(arg, data),
            ArithmeticType::Modulo => self.apply_modulo(arg, data),
            ArithmeticType::Max => self.apply_max(arg, data),
            ArithmeticType::Min => self.apply_min(arg, data),
        }
    }

    fn apply_add<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match &*value {  // Dereference Cow to get &Value
                    Value::Array(arr) => arr
                        .iter()
                        .try_fold(0.0, |acc, v| Ok(acc + v.coerce_to_number()?))
                        .map(|sum| Cow::Owned(sum.to_value())),
                    _ => value.coerce_to_number().map(|n| Cow::Owned(n.to_value())),
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Ok(Cow::Owned(Value::Number(0.into()))),
                _ => {
                    let sum = rules.iter().try_fold(0.0, |acc, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(acc + num)
                    })?;
                    Ok(Cow::Owned(sum.to_value()))
                }
            },
        }
    }

    fn apply_multiply<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match &*value {
                    Value::Array(arr) => arr
                        .iter()
                        .try_fold(1.0, |acc, v| Ok(acc * v.coerce_to_number()?))
                        .map(|product| Cow::Owned(product.to_value())),
                    _ => value.coerce_to_number().map(|n| Cow::Owned(n.to_value())),
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Ok(Cow::Owned(Value::Number(1.into()))),
                _ => rules
                    .iter()
                    .try_fold(1.0, |acc, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(acc * num)
                    })
                    .map(|product| Cow::Owned(product.to_value())),
            },
        }
    }

    fn apply_subtract<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match &*value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Err(Error::Custom("Invalid Arguments".to_string()));
                        }
                        
                        if arr.len() == 1 {
                            let num = arr[0].coerce_to_number()?;
                            return Ok(Cow::Owned((-num).to_value()));
                        }
    
                        let mut iter = arr.iter();
                        let first = iter.next()
                            .ok_or_else(|| Error::InvalidArguments(
                                "Subtract operation requires at least one argument".to_string()
                            ))?
                            .coerce_to_number()?;
    
                        iter.try_fold(first, |acc, v| {
                            let num = v.coerce_to_number()?;
                            Ok(acc - num)
                        })
                        .map(|result| Cow::Owned(result.to_value()))
                    }
                    _ => value.coerce_to_number()
                        .map(|num| Cow::Owned((-num).to_value()))
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Err(Error::Custom("Invalid Arguments".to_string())),
                1 => {
                    let value = rules[0].apply(data)?;
                    value.coerce_to_number()
                        .map(|num| Cow::Owned((-num).to_value()))
                }
                _ => {
                    let first = rules[0].apply(data)?.coerce_to_number()?;
                    rules.iter()
                        .skip(1)
                        .try_fold(first, |acc, rule| {
                            let value = rule.apply(data)?;
                            let num = value.coerce_to_number()?;
                            Ok(acc - num)
                        })
                        .map(|result| Cow::Owned(result.to_value()))
                }
            }
        }
    }

    fn apply_divide<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match &*value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Err(Error::Custom("Invalid Arguments".to_string()));
                        } else if arr.len() == 1 {
                            let num = arr[0].coerce_to_number()?;
                            if num == 0.0 {
                                return Err(Error::Custom("NaN".to_string()));
                            }
                            return Ok(Cow::Owned((1.0 / num).to_value()));
                        }
    
                        let mut iter = arr.iter();
                        let first = iter
                            .next()
                            .ok_or_else(|| Error::InvalidArguments(
                                "Division requires at least one argument".to_string()
                            ))?
                            .coerce_to_number()?;
    
                        let rest = iter.try_fold(1.0, |acc, v| {
                            let num = v.coerce_to_number()?;
                            if num == 0.0 {
                                Err(Error::Custom("NaN".to_string()))
                            } else {
                                Ok(acc * num)
                            }
                        })?;
    
                        if rest == 0.0 {
                            return Err(Error::Custom("NaN".to_string()));
                        }
                        Ok(Cow::Owned((first / rest).to_value()))
                    }
                    _ => {
                        let num = value.coerce_to_number()?;
                        if num == 0.0 {
                            Err(Error::Custom("NaN".to_string()))
                        } else {
                            Ok(Cow::Owned((1.0 / num).to_value()))
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
                        Ok(Cow::Owned((1.0 / num).to_value()))
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
    
                    if rest == 0.0 {
                        return Err(Error::Custom("NaN".to_string()));
                    }
                    Ok(Cow::Owned((first / rest).to_value()))
                }
            },
        }
    }

    fn apply_modulo<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
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
    
                        Ok(Cow::Owned(rest.to_value()))
                    }
                }
            }
        }
    }
    
    fn apply_max<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match &*value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Ok(Cow::Owned(Value::Null));
                        }
    
                        arr.iter()
                            .try_fold(f64::NEG_INFINITY, |max, v| {
                                let num = v.coerce_to_number()?;
                                Ok(max.max(num))
                            })
                            .map(|max| Cow::Owned(max.to_value()))
                    }
                    _ => value.coerce_to_number()
                        .map(|num| Cow::Owned(num.to_value()))
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Ok(Cow::Owned(Value::Null)),
                _ => rules
                    .iter()
                    .try_fold(f64::NEG_INFINITY, |max, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(max.max(num))
                    })
                    .map(|max| Cow::Owned(max.to_value()))
            },
        }
    }
    
    fn apply_min<'a>(&self, arg: &ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(data)?;
                match &*value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Ok(Cow::Owned(Value::Null));
                        }
    
                        arr.iter()
                            .try_fold(f64::INFINITY, |min, v| {
                                let num = v.coerce_to_number()?;
                                Ok(min.min(num))
                            })
                            .map(|min| Cow::Owned(min.to_value()))
                    }
                    _ => value.coerce_to_number()
                        .map(|num| Cow::Owned(num.to_value()))
                }
            }
            ArgType::Multiple(rules) => match rules.len() {
                0 => Ok(Cow::Owned(Value::Null)),
                _ => rules
                    .iter()
                    .try_fold(f64::INFINITY, |min, rule| {
                        let value = rule.apply(data)?;
                        let num = value.coerce_to_number()?;
                        Ok(min.min(num))
                    })
                    .map(|min| Cow::Owned(min.to_value()))
            },
        }
    }
}