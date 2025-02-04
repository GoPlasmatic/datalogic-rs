use serde_json::Value;
use super::coercion::{ValueCoercion, ValueConvert};
use super::EvalResult;

#[inline]
pub fn evaluate_add(args: &[Value]) -> EvalResult {
    match args {
        [] => Ok(0.0.to_value()),
        [Value::Array(arr)] => {
            let sum = arr.iter()
                .fold(0.0, |acc, v| acc + v.coerce_to_number());
            Ok(sum.to_value())
        }
        [single] => Ok(single.coerce_to_number().to_value()),
        multiple => {
            let sum = multiple.iter()
                .fold(0.0, |acc, v| acc + v.coerce_to_number());
            Ok(sum.to_value())
        }
    }
}

#[inline]
pub fn evaluate_sub(args: &[Value]) -> EvalResult {
    if args.is_empty() {
        return Ok(0.0.to_value());
    }
    
    let first = args[0].coerce_to_number();
    if args.len() == 1 {
        return Ok((-first).to_value());
    }
    
    let result = args[1..].iter()
        .fold(first, |acc, v| acc - v.coerce_to_number());
    Ok(result.to_value())
}

#[inline]
pub fn evaluate_mul(args: &[Value]) -> EvalResult {
    match args {
        [] => Ok(1.0.to_value()),
        [Value::Array(arr)] => {
            let product = arr.iter()
                .fold(1.0, |acc, v| acc * v.coerce_to_number());
            Ok(product.to_value())
        }
        [single] => Ok(single.coerce_to_number().to_value()),
        multiple => {
            let product = multiple.iter()
                .fold(1.0, |acc, v| acc * v.coerce_to_number());
            Ok(product.to_value())
        }
    }
}

#[inline]
pub fn evaluate_div(args: &[Value]) -> EvalResult {
    if args.is_empty() {
        return Ok(0.0.to_value());
    }
    
    let first = args[0].coerce_to_number();
    if args.len() == 1 {
        return Ok((1.0 / first).to_value());
    }
    
    let result = args[1..].iter()
        .fold(first, |acc, v| acc / v.coerce_to_number());
    Ok(result.to_value())
}

#[inline]
pub fn evaluate_mod(args: &[Value]) -> EvalResult {
    if args.len() < 2 {
        return Ok(args.first()
            .map_or(0.0, |v| v.coerce_to_number())
            .to_value());
    }
    
    Ok((args[0].coerce_to_number() % args[1].coerce_to_number()).to_value())
}

#[inline]
pub fn evaluate_max(args: &[Value]) -> EvalResult {
    let mut max = f64::NEG_INFINITY;
    
    for arg in args {
        match arg {
            Value::Array(arr) => {
                for v in arr {
                    max = max.max(v.coerce_to_number());
                }
            }
            _ => max = max.max(arg.coerce_to_number())
        }
    }
    
    Ok(max.to_value())
}

#[inline]
pub fn evaluate_min(args: &[Value]) -> EvalResult {
    let mut min = f64::INFINITY;
    
    for arg in args {
        match arg {
            Value::Array(arr) => {
                for v in arr {
                    min = min.min(v.coerce_to_number());
                }
            }
            _ => min = min.min(arg.coerce_to_number())
        }
    }
    
    Ok(min.to_value())
}