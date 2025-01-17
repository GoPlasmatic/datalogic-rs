use serde_json::Value;
use crate::{rule::ArgType, JsonLogicResult};
use super::{ValueConvert, ValueCoercion};

pub struct AddOperator;
pub struct MultiplyOperator;
pub struct SubtractOperator;
pub struct DivideOperator;
pub struct ModuloOperator;
pub struct MaxOperator;
pub struct MinOperator;

impl AddOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                Ok(value.coerce_to_number().to_value())
            },
            ArgType::Array(rules) => {
                if rules.is_empty() {
                    return Ok(Value::Number(0.into()));
                }
                let sum = rules.iter()
                    .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                    .sum::<f64>();
                Ok(sum.to_value())
            }
        }
    }
}

impl MultiplyOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                Ok(value.coerce_to_number().to_value())
            },
            ArgType::Array(rules) => {
                if rules.is_empty() {
                    return Ok(Value::Number(1.into()));
                }
                let mut product = 1.0;
                for rule in rules {
                    product *= rule.apply(data)?.coerce_to_number();
                    if product == 0.0 {
                        return Ok(Value::Number(0.into()));
                    }
                }
                Ok(product.to_value())
            }
        }
    }
}

impl SubtractOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                Ok((-value.coerce_to_number()).to_value())
            },
            ArgType::Array(rules) => {
                if rules.is_empty() {
                    return Ok(Value::Number(0.into()));
                }
                let first = rules[0].apply(data)?.coerce_to_number();
                if rules.len() == 1 {
                    return Ok((-first).to_value());
                }
                let result = rules.iter().skip(1).fold(first, |acc, rule| {
                    acc - rule.apply(data).unwrap().coerce_to_number()
                });
                Ok(result.to_value())
            }
        }
    }
}

impl DivideOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                Ok((1.0 / value.coerce_to_number()).to_value())
            },
            ArgType::Array(rules) => {
                if rules.is_empty() {
                    return Ok(Value::Number(1.into()));
                }
                let first = rules[0].apply(data)?.coerce_to_number();
                if rules.len() == 1 {
                    return Ok(first.to_value());
                }
                let result = rules.iter().skip(1).fold(first, |acc, rule| {
                    let divisor = rule.apply(data).unwrap().coerce_to_number();
                    if divisor == 0.0 {
                        return 0.0;
                    }
                    acc / divisor
                });
                Ok(result.to_value())
            }
        }
    }
}

impl ModuloOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                Ok(value.coerce_to_number().to_value())
            },
            ArgType::Array(rules) => {
                if rules.is_empty() {
                    return Ok(Value::Number(0.into()));
                }
                let first = rules[0].apply(data)?.coerce_to_number();
                if rules.len() == 1 {
                    return Ok(first.to_value());
                }
                let result = rules.iter().skip(1).fold(first, |acc, rule| {
                    let divisor = rule.apply(data).unwrap().coerce_to_number();
                    if divisor == 0.0 {
                        return 0.0;
                    }
                    acc % divisor
                });
                Ok(result.to_value())
            }
        }
    }
}

impl MaxOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Ok(Value::Null);
                        }
                        let mut max = f64::NEG_INFINITY;
                        for val in arr {
                            max = max.max(val.coerce_to_number());
                        }
                        Ok(max.to_value())
                    },
                    _ => Ok(value.coerce_to_number().to_value())
                }
            },
            ArgType::Array(rules) => match rules.as_slice() {
                [] => Ok(Value::Null),
                [single] => single.apply(data),
                [first, second] => {
                    let a = first.apply(data)?.coerce_to_number();
                    let b = second.apply(data)?.coerce_to_number();
                    Ok(a.max(b).to_value())
                },
                args => {
                    let mut max = f64::NEG_INFINITY;
                    for arg in args {
                        let val = arg.apply(data)?.coerce_to_number();
                        max = max.max(val);
                    }
                    Ok(max.to_value())
                }
    
            }
        }
    }
}

impl MinOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            return Ok(Value::Null);
                        }
                        let mut min = f64::INFINITY;
                        for val in arr {
                            min = min.min(val.coerce_to_number());
                        }
                        Ok(min.to_value())
                    },
                    _ => Ok(value.coerce_to_number().to_value())
                }
            },
            ArgType::Array(rules) => match rules.as_slice() {
                [] => Ok(Value::Null),
                [single] => single.apply(data),
                [first, second] => {
                    let a = first.apply(data)?.coerce_to_number();
                    let b = second.apply(data)?.coerce_to_number();
                    Ok(a.min(b).to_value())
                },
                args => {
                    let mut min = f64::INFINITY;
                    for arg in args {
                        let val = arg.apply(data)?.coerce_to_number();
                        min = min.min(val);
                    }
                    Ok(min.to_value())
                }
            }
        }
    }
}