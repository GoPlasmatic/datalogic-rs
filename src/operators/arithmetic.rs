use crate::operators::operator::Operator;
use crate::{Error, JsonLogic, JsonLogicResult};
use serde_json::Value;

pub struct AddOperator;

impl Operator for AddOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        match args {
            // Handle unary plus operation (type conversion)
            Value::String(s) => {
                match s.parse::<f64>() {
                    Ok(n) => {
                        if n.fract() == 0.0 {
                            Ok(Value::Number(serde_json::Number::from(n as i64)))
                        } else {
                            Ok(Value::Number(serde_json::Number::from_f64(n).unwrap()))
                        }
                    },
                    Err(_) => Err(Error::InvalidArguments("Invalid number string".into()))
                }
            },
            Value::Number(n) => Ok(Value::Number(n.clone())),
            // Handle array of values for addition
            Value::Array(values) => {
                let sum = values
                    .iter()
                    .map(|v| logic.apply(v, data))
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .map(|v| match v {
                        Value::Number(n) => Ok(n.as_f64().unwrap()),
                        Value::String(s) => s.parse::<f64>()
                            .map_err(|_| Error::InvalidArguments("Invalid number string".into())),
                        _ => Ok(0.0),
                    })
                    .sum::<Result<f64, _>>()?;

                if sum.fract() == 0.0 {
                    Ok(Value::Number(serde_json::Number::from(sum as i64)))
                } else {
                    Ok(Value::Number(serde_json::Number::from_f64(sum).unwrap()))
                }
            },
            _ => Err(Error::InvalidArguments("+ requires string or array argument".into()))
        }
    }
}

pub struct ModuloOperator;

impl Operator for ModuloOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Err(Error::InvalidArguments("% requires 2 arguments".into()));
            }

            let left = logic.apply(&values[0], data)?;
            let right = logic.apply(&values[1], data)?;

            match (left, right) {
                (Value::Number(n1), Value::Number(n2)) => {
                    let dividend = n1.as_f64().unwrap();
                    let divisor = n2.as_f64().unwrap();
                    
                    if divisor == 0.0 {
                        return Err(Error::InvalidArguments("Division by zero".into()));
                    }

                    let result = dividend % divisor;
                    
                    // Convert to integer if result has no decimal part
                    if result.fract() == 0.0 {
                        Ok(Value::Number(serde_json::Number::from(result as i64)))
                    } else {
                        Ok(Value::Number(serde_json::Number::from_f64(result).unwrap()))
                    }
                },
                _ => Err(Error::InvalidArguments("% requires numeric arguments".into()))
            }
        } else {
            Err(Error::InvalidArguments("% requires array argument".into()))
        }
    }
}


pub struct MaxOperator;

impl Operator for MaxOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.is_empty() {
                return Err(Error::InvalidArguments("max requires at least 1 argument".into()));
            }

            let numbers: Result<Vec<f64>, _> = values
                .iter()
                .map(|v| logic.apply(v, data))
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .map(|v| match v {
                    Value::Number(n) => Ok(n.as_f64().unwrap()),
                    _ => Err(Error::InvalidArguments("max requires numeric arguments".into())),
                })
                .collect();

            let max = numbers?.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            if max.fract() == 0.0 {
                Ok(Value::Number(serde_json::Number::from(max as i64)))
            } else {
                Ok(Value::Number(serde_json::Number::from_f64(max).unwrap()))
            }
        } else {
            Err(Error::InvalidArguments("max requires array argument".into()))
        }
    }
}

pub struct MinOperator;

impl Operator for MinOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.is_empty() {
                return Err(Error::InvalidArguments("min requires at least 1 argument".into()));
            }

            let numbers: Result<Vec<f64>, _> = values
                .iter()
                .map(|v| logic.apply(v, data))
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .map(|v| match v {
                    Value::Number(n) => Ok(n.as_f64().unwrap()),
                    _ => Err(Error::InvalidArguments("min requires numeric arguments".into())),
                })
                .collect();

            let min = numbers?.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            
            if min.fract() == 0.0 {
                Ok(Value::Number(serde_json::Number::from(min as i64)))
            } else {
                Ok(Value::Number(serde_json::Number::from_f64(min).unwrap()))
            }
        } else {
            Err(Error::InvalidArguments("min requires array argument".into()))
        }
    }
}


pub struct MultiplyOperator;

impl Operator for MultiplyOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.is_empty() {
                return Ok(Value::Number(0.into()));
            }

            let product = values
                .iter()
                .map(|v| logic.apply(v, data))
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .map(|v| match v {
                    Value::Number(n) => Ok(n.as_f64().unwrap()),
                    Value::String(s) => s.parse::<f64>()
                        .map_err(|_| Error::InvalidArguments("Invalid number string".into())),
                    _ => Ok(0.0),
                })
                .try_fold(1.0, |acc, x| Ok(acc * x?))?;
            
            if product.fract() == 0.0 {
                Ok(Value::Number(serde_json::Number::from(product as i64)))
            } else {
                Ok(Value::Number(serde_json::Number::from_f64(product).unwrap()))
            }
        } else {
            Err(Error::InvalidArguments("* requires array argument".into()))
        }
    }
}

pub struct SubtractOperator;

impl Operator for SubtractOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.is_empty() {
                return Ok(Value::Number(0.into()));
            }

            let numbers: Result<Vec<f64>, _> = values
                .iter()
                .map(|v| logic.apply(v, data))
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .map(|v| match v {
                    Value::Number(n) => Ok(n.as_f64().unwrap()),
                    Value::String(s) => s.parse::<f64>()
                        .map_err(|_| Error::InvalidArguments("Invalid number string".into())),
                    _ => Ok(0.0),
                })
                .collect();

            let nums = numbers?;
            let result = if nums.len() == 1 {
                -nums[0] // Negation when single argument
            } else {
                nums[0] - nums[1..].iter().sum::<f64>()
            };

            if result.fract() == 0.0 {
                Ok(Value::Number(serde_json::Number::from(result as i64)))
            } else {
                Ok(Value::Number(serde_json::Number::from_f64(result).unwrap()))
            }
        } else {
            Err(Error::InvalidArguments("- requires array argument".into()))
        }
    }
}

pub struct DivideOperator;

impl Operator for DivideOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        if let Value::Array(values) = args {
            if values.len() != 2 {
                return Err(Error::InvalidArguments("/ requires 2 arguments".into()));
            }

            let numbers: Result<Vec<f64>, _> = values
                .iter()
                .map(|v| logic.apply(v, data))
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .map(|v| match v {
                    Value::Number(n) => Ok(n.as_f64().unwrap()),
                    Value::String(s) => s.parse::<f64>()
                        .map_err(|_| Error::InvalidArguments("Invalid number string".into())),
                    _ => Ok(0.0),
                })
                .collect();

            let nums = numbers?;
            if nums[1] == 0.0 {
                return Err(Error::InvalidArguments("Division by zero".into()));
            }

            let result = nums[0] / nums[1];
            if result.fract() == 0.0 {
                Ok(Value::Number(serde_json::Number::from(result as i64)))
            } else {
                Ok(Value::Number(serde_json::Number::from_f64(result).unwrap()))
            }
        } else {
            Err(Error::InvalidArguments("/ requires array argument".into()))
        }
    }
}