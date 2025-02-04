use serde_json::Value;
use super::coercion::ValueCoercion;
use super::EvalResult;

#[inline]
pub fn evaluate_if(args: &[Value]) -> EvalResult {
    match args {
        [] => Ok(Value::Null),
        [single] => Ok(single.clone()),
        [condition, consequent] => Ok(if condition.coerce_to_bool() {
            consequent.clone()
        } else {
            Value::Null
        }),
        [condition, consequent, alternative] => Ok(if condition.coerce_to_bool() {
            consequent.clone()
        } else {
            alternative.clone()
        }),
        _ => evaluate_multi_if(args)
    }
}

#[inline]
fn evaluate_multi_if(args: &[Value]) -> EvalResult {
    let (chunks, remainder) = args.split_at(args.len() - (args.len() % 2));
    
    for chunk in chunks.chunks_exact(2) {
        if chunk[0].coerce_to_bool() {
            return Ok(chunk[1].clone());
        }
    }
    
    Ok(remainder.first().cloned().unwrap_or(Value::Null))
}

#[inline]
pub fn evaluate_ternary(args: &[Value]) -> EvalResult {
    match args {
        [condition, consequent, alternative] => Ok(if condition.coerce_to_bool() {
            consequent.clone()
        } else {
            alternative.clone()
        }),
        _ => Ok(Value::Null)
    }
}