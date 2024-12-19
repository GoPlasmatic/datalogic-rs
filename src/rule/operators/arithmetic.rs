use serde_json::Value;
use crate::Error;
use super::{Operator, Rule, ValueCoercion, ValueConvert};

pub struct AddOperator;
pub struct MultiplyOperator;
pub struct SubtractOperator;
pub struct DivideOperator;
pub struct ModuloOperator;
pub struct MaxOperator;
pub struct MinOperator;

impl Operator for AddOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [] => Ok(Value::Number(0.into())),
            _ => {
                let sum = args.iter()
                    .map(|arg| arg.apply(data).unwrap().coerce_to_number())
                    .sum::<f64>();
                Ok(sum.to_value())
            }
        }
    }
}

impl Operator for MultiplyOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args.len() {
            0 => Ok(Value::Number(1.into())),
            1 => Ok(args[0].apply(data)?.coerce_to_number().to_value()),
            _ => {
                let mut product = 1.0;
                for arg in args {
                    product *= &arg.apply(data)?.coerce_to_number();
                    if product == 0.0 {
                        return Ok(Value::Number(0.into()));
                    }
                }
                Ok(product.to_value())
            }
        }
    }
}

impl Operator for SubtractOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [] => Ok(Value::Number(0.into())),
            [single] => {
                let value = single.apply(data)?.coerce_to_number();
                Ok((-value).to_value())
            },
            [first, rest @ ..] => {
                let mut result = first.apply(data)?.coerce_to_number();
                for arg in rest {
                    result -= arg.apply(data)?.coerce_to_number();
                }
                Ok(result.to_value())
            }
        }
    }
}

impl Operator for DivideOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [numerator, denominator] => {
                let num = numerator.apply(data)?.coerce_to_number();
                let den = denominator.apply(data)?.coerce_to_number();
                
                match den {
                    0.0 => Ok(Value::Null),
                    _ => Ok((num / den).to_value())
                }
            },
            _ => Err(Error::InvalidArguments("divide requires 2 arguments".into()))
        }
    }
}

impl Operator for ModuloOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [numerator, denominator] => {
                let num = numerator.apply(data)?.coerce_to_number();
                let den = denominator.apply(data)?.coerce_to_number();
                
                match den {
                    0.0 => Ok(Value::Null),
                    _ => Ok((num % den).to_value())
                }
            },
            _ => Err(Error::InvalidArguments("modulo requires 2 arguments".into()))
        }
    }
}

impl Operator for MaxOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [] => Ok(Value::Null),
            [single] => single.apply(data),
            [first, second] => {
                let a = first.apply(data)?.coerce_to_number();
                let b = second.apply(data)?.coerce_to_number();
                Ok(a.max(b).to_value())
            },
            _ => {
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

impl Operator for MinOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [] => Ok(Value::Null),
            [single] => single.apply(data),
            [first, second] => {
                let a = first.apply(data)?.coerce_to_number();
                let b = second.apply(data)?.coerce_to_number();
                Ok(a.min(b).to_value())
            },
            _ => {
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