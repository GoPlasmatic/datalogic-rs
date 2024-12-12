use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

pub struct AddOperator;
pub struct MultiplyOperator;
pub struct SubtractOperator;
pub struct DivideOperator;
pub struct ModuloOperator;
pub struct MaxOperator;
pub struct MinOperator;

fn to_number(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        Value::Bool(b) => if *b { 1.0 } else { 0.0 },
        _ => 0.0,
    }
}

fn to_value(num: f64) -> Value {
    if num.fract() == 0.0 {
        Value::Number(serde_json::Number::from(num as i64))
    } else {
        Value::Number(serde_json::Number::from_f64(num).unwrap())
    }
}

impl Operator for AddOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        let mut sum = 0.0;
        for arg in args {
            let val = arg.apply(data)?;
            sum += to_number(&val);
        }
        Ok(to_value(sum))
    }
}

impl Operator for MultiplyOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Number(1.into()));
        }
        let mut product = 1.0;
        for arg in args {
            let val = arg.apply(data)?;
            product *= to_number(&val);
        }
        Ok(to_value(product))
    }
}

impl Operator for SubtractOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Number(0.into()));
        }
        let first = to_number(&args[0].apply(data)?);
        if args.len() == 1 {
            return Ok(to_value(-first));
        }
        let mut result = first;
        for arg in &args[1..] {
            let val = arg.apply(data)?;
            result -= to_number(&val);
        }
        Ok(to_value(result))
    }
}

impl Operator for DivideOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("divide requires 2 arguments".to_string()));
        }
        let numerator = to_number(&args[0].apply(data)?);
        let denominator = to_number(&args[1].apply(data)?);
        if denominator == 0.0 {
            return Ok(Value::Null);
        }
        Ok(to_value(numerator / denominator))
    }
}

impl Operator for ModuloOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("modulo requires 2 arguments".to_string()));
        }
        let a = to_number(&args[0].apply(data)?);
        let b = to_number(&args[1].apply(data)?);
        if b == 0.0 {
            return Ok(Value::Null);
        }
        Ok(to_value(a % b))
    }
}

impl Operator for MaxOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Null);
        }
        let mut max = f64::NEG_INFINITY;
        for arg in args {
            let val = arg.apply(data)?;
            max = max.max(to_number(&val));
        }
        Ok(to_value(max))
    }
}

impl Operator for MinOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Null);
        }
        let mut min = f64::INFINITY;
        for arg in args {
            let val = arg.apply(data)?;
            min = min.min(to_number(&val));
        }
        Ok(to_value(min))
    }
}