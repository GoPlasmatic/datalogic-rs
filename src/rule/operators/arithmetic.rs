use serde_json::Value;
use crate::JsonLogicResult;
use crate::rule::{Rule, ArgType};
use super::{ValueCoercion, ValueConvert};

#[derive(Debug, Clone)]
pub enum ArithmeticType {
    Add,
    Multiply,
    Subtract,
    Divide,
    Modulo,
    Max,
    Min
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

    fn apply_add(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(_) => {
                        let sum = value.as_array().unwrap()
                            .iter()
                            .map(|v| v.coerce_to_number())
                            .sum::<f64>();
                        Ok(sum.to_value())
                    },
                    _ => Ok(value.coerce_to_number().to_value())
                }
            },
            ArgType::Array(rules) => {
                match rules.len() {
                    0 => Ok(Value::Number(0.into())),
                    _ => {
                        let sum = rules.iter()
                            .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                            .sum::<f64>();
                        Ok(sum.to_value())
                    }
                }
            }
        }
    }

    fn apply_multiply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                match rule.as_ref() {
                    Rule::Array(rules) => {
                        match rules.len() {
                            0 => Ok(Value::Number(1.into())),
                            _ => {
                                let product = rules.iter()
                                    .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                                    .product::<f64>();
                                Ok(product.to_value())
                            }
                        }
                    },
                    Rule::Value(value) => Ok(value.coerce_to_number().to_value()),
                    _ => unreachable!("Invalid rule type for multiply")
                }
            },
            ArgType::Array(rules) => {
                match rules.len() {
                    0 => Ok(Value::Number(1.into())),
                    _ => {
                        let product = rules.iter()
                            .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                            .product::<f64>();
                        Ok(product.to_value())
                    }
                }
            }
        }
    }

    fn apply_subtract(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(_) => {
                        let first = value.as_array().unwrap()
                            .iter()
                            .map(|v| v.coerce_to_number())
                            .next()
                            .unwrap_or(0.0);
                        let rest = value.as_array().unwrap()
                            .iter()
                            .skip(1)
                            .map(|v| v.coerce_to_number())
                            .sum::<f64>();
                        Ok((first - rest).to_value())
                    },
                    _ => Ok((-value.coerce_to_number()).to_value())
                }
            },
            ArgType::Array(rules) => {
                match rules.len() {
                    0 => Ok(Value::Number(0.into())),
                    1 => {
                        let value = rules[0].apply(data)?;
                        Ok((-value.coerce_to_number()).to_value())
                    },
                    _ => {
                        let first = rules[0].apply(data)?.coerce_to_number();
                        let rest: f64 = rules.iter().skip(1)
                            .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                            .sum();
                        Ok((first - rest).to_value())
                    }
                }
            }
        }
    }

    fn apply_divide(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(_) => {
                        let first = value.as_array().unwrap()
                            .iter()
                            .map(|v| v.coerce_to_number())
                            .next()
                            .unwrap_or(0.0);
                        let rest = value.as_array().unwrap()
                            .iter()
                            .skip(1)
                            .map(|v| v.coerce_to_number())
                            .fold(1.0, |acc, x| acc * if x == 0.0 { 1.0 } else { x });
                        Ok((first / rest).to_value())
                    },
                    _ => {
                        let num = value.coerce_to_number();
                        if num == 0.0 {
                            return Ok(Value::Number(0.into()));
                        }
                        Ok((1.0 / num).to_value())
                    }
                }
            },
            ArgType::Array(rules) => {
                match rules.len() {
                    0 => Ok(Value::Number(1.into())),
                    1 => {
                        let value = rules[0].apply(data)?;
                        let num = value.coerce_to_number();
                        if num == 0.0 {
                            return Ok(Value::Number(0.into()));
                        }
                        Ok((1.0 / num).to_value())
                    },
                    _ => {
                        let first = rules[0].apply(data)?.coerce_to_number();
                        let rest = rules.iter().skip(1)
                            .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                            .fold(1.0, |acc, x| acc * if x == 0.0 { 1.0 } else { x });
                        Ok((first / rest).to_value())
                    }
                }
            }
        }
    }

    fn apply_modulo(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(_) => {
                        let first = value.as_array().unwrap()
                            .iter()
                            .map(|v| v.coerce_to_number())
                            .next()
                            .unwrap_or(0.0);
                        let rest = value.as_array().unwrap()
                            .iter()
                            .skip(1)
                            .map(|v| v.coerce_to_number())
                            .fold(1.0, |acc, x| acc * if x == 0.0 { 1.0 } else { x });
                        Ok((first % rest).to_value())
                    },
                    _ => {
                        let num = value.coerce_to_number();
                        if num == 0.0 {
                            return Ok(Value::Number(0.into()));
                        }
                        Ok((1.0 % num).to_value())
                    }
                }
            },
            ArgType::Array(rules) => {
                match rules.len() {
                    0 => Ok(Value::Number(0.into())),
                    1 => {
                        let value = rules[0].apply(data)?;
                        let num = value.coerce_to_number();
                        if num == 0.0 {
                            return Ok(Value::Number(0.into()));
                        }
                        Ok((1.0 % num).to_value())
                    },
                    _ => {
                        let first = rules[0].apply(data)?.coerce_to_number();
                        let rest = rules.iter().skip(1)
                            .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                            .fold(1.0, |acc, x| acc * if x == 0.0 { 1.0 } else { x });
                        Ok((first % rest).to_value())
                    }
                }
            }
        }
    }

    fn apply_max(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(_) => {
                        let max = value.as_array().unwrap()
                            .iter()
                            .map(|v| v.coerce_to_number())
                            .fold(f64::NEG_INFINITY, f64::max);
                        Ok(max.to_value())
                    },
                    _ => Ok(value.coerce_to_number().to_value())
                }
            },
            ArgType::Array(rules) => {
                match rules.len() {
                    0 => Ok(Value::Null),
                    _ => {
                        let max = rules.iter()
                            .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                            .fold(f64::NEG_INFINITY, f64::max);
                        Ok(max.to_value())
                    }
                }
            }
        }
    }

    fn apply_min(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => {
                let value = rule.apply(data)?;
                match value {
                    Value::Array(_) => {
                        let min = value.as_array().unwrap()
                            .iter()
                            .map(|v| v.coerce_to_number())
                            .fold(f64::INFINITY, f64::min);
                        Ok(min.to_value())
                    },
                    _ => Ok(value.coerce_to_number().to_value())
                }
            },
            ArgType::Array(rules) => {
                match rules.len() {
                    0 => Ok(Value::Null),
                    _ => {
                        let min = rules.iter()
                            .map(|rule| rule.apply(data).unwrap().coerce_to_number())
                            .fold(f64::INFINITY, f64::min);
                        Ok(min.to_value())
                    }
                }
            }
        }
    }
}