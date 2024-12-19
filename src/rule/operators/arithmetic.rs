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
        let mut sum = 0.0;
        for arg in args {
            let val = arg.apply(data)?;
            sum += val.coerce_to_number();
        }
        Ok(sum.to_value())
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
        if args.is_empty() {
            return Ok(Value::Number(0.into()));
        }
        let first = &args[0].apply(data)?.coerce_to_number();
        if args.len() == 1 {
            return Ok((-first).to_value());
        }
        let mut result: f64 = *first;
        for arg in &args[1..] {
            let val = arg.apply(data)?;
            result -= val.coerce_to_number();
        }
        Ok(result.to_value())
    }
}

impl Operator for DivideOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("divide requires 2 arguments".to_string()));
        }
        let numerator = args[0].apply(data)?.coerce_to_number();
        let denominator = args[1].apply(data)?.coerce_to_number();
        if denominator == 0.0 {
            return Ok(Value::Null);
        }
        Ok((numerator / denominator).to_value())
    }
}

impl Operator for ModuloOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("modulo requires 2 arguments".to_string()));
        }
        let a = args[0].apply(data)?.coerce_to_number();
        let b = args[1].apply(data)?.coerce_to_number();
        if b == 0.0 {
            return Ok(Value::Null);
        }
        Ok((a % b).to_value())
    }
}

impl Operator for MaxOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Null);
        }
        let mut max = f64::NEG_INFINITY;
        for arg in args {
            let val = arg.apply(data)?.coerce_to_number();
            max = max.max(val);
        }
        Ok(max.to_value())
    }
}

impl Operator for MinOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Null);
        }
        let mut min = f64::INFINITY;
        for arg in args {
            let val = arg.apply(data)?.coerce_to_number();
            min = min.min(val);
        }
        Ok(min.to_value())
    }
}