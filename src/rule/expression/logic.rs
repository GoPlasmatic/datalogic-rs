use serde_json::Value;
use super::{coercion::ValueCoercion, EvalResult};

#[inline]
pub fn evaluate_and(args: &[Value]) -> EvalResult {
    match args {
        [] => Ok(Value::Bool(true)),
        [single] => Ok(single.clone()),
        _ => {
            let last = args.last().unwrap();
            for arg in &args[..args.len()-1] {
                if !arg.coerce_to_bool() {
                    return Ok(arg.clone());
                }
            }
            Ok(last.clone())
        }
    }
}

#[inline]
pub fn evaluate_or(args: &[Value]) -> EvalResult {
    match args {
        [] => Ok(Value::Bool(false)),
        [single] => Ok(single.clone()),
        _ => {
            let last = args.last().unwrap();
            for arg in &args[..args.len()-1] {
                if arg.coerce_to_bool() {
                    return Ok(arg.clone());
                }
            }
            Ok(last.clone())
        }
    }
}

#[inline]
pub fn evaluate_not(args: &[Value]) -> bool {
    match args {
        [] => true,
        [single] => !single.coerce_to_bool(),
        _ => args.iter().any(|arg| !arg.coerce_to_bool())
    }
}

#[inline]
pub fn evaluate_double_bang(args: &[Value]) -> bool {
    match args {
        [] => false,
        [single] => single.coerce_to_bool(),
        _ => !args.iter().any(|arg| !arg.coerce_to_bool())
    }
}