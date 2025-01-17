use serde_json::Value;
use crate::{Error, JsonLogicResult};
use super::{Rule, ValueCoercion};

pub struct EqualsOperator;
pub struct StrictEqualsOperator;
pub struct NotEqualsOperator;
pub struct StrictNotEqualsOperator;
pub struct GreaterThanOperator;
pub struct LessThanOperator;
pub struct GreaterThanEqualOperator;
pub struct LessThanEqualOperator;


impl EqualsOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(match (&left, &right) {
                    (Value::Number(n1), Value::Number(n2)) => n1 == n2,
                    (Value::String(s1), Value::String(s2)) => s1 == s2,
                    (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
                    _ => left.coerce_to_number() == right.coerce_to_number()
                }))
            },
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if std::mem::discriminant(&prev) == std::mem::discriminant(&curr) || prev == curr {
                        return Ok(Value::Bool(false));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(true))
            }
            args if args.len() < 2 => {
                Ok(Value::Bool(false))
            }
            _ => Err(Error::InvalidArguments("==".into()))
        }
    }
}

impl StrictEqualsOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(std::mem::discriminant(&left) == std::mem::discriminant(&right) && left == right))
            }
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if std::mem::discriminant(&prev) != std::mem::discriminant(&curr) || prev != curr {
                        return Ok(Value::Bool(false));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(true))
            }
            args if args.len() < 2 => {
                Ok(Value::Bool(false))
            }
            _ => Err(Error::InvalidArguments("===".into()))
        }
    }
}

impl NotEqualsOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(match (&left, &right) {
                    (Value::Number(n1), Value::Number(n2)) => n1 != n2,
                    (Value::String(s1), Value::String(s2)) => s1 != s2,
                    (Value::Bool(b1), Value::Bool(b2)) => b1 != b2,
                    _ => left.coerce_to_number() != right.coerce_to_number()
                }))
            }
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    let not_equal = match (&prev, &curr) {
                        (Value::Number(n1), Value::Number(n2)) => n1 != n2,
                        (Value::String(s1), Value::String(s2)) => s1 != s2,
                        (Value::Bool(b1), Value::Bool(b2)) => b1 != b2,
                        _ => prev.coerce_to_number() != curr.coerce_to_number()
                    };
                    if not_equal {
                        return Ok(Value::Bool(true));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(false))
            }
            args if args.len() < 2 => {
                Ok(Value::Bool(false))
            }
            _ => Err(Error::InvalidArguments("!=".into()))
        }
    }
}

impl StrictNotEqualsOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(std::mem::discriminant(&left) != std::mem::discriminant(&right) || left != right))
            }
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if std::mem::discriminant(&prev) != std::mem::discriminant(&curr) || prev != curr {
                        return Ok(Value::Bool(true));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(false))
            }
            args if args.len() < 2 => {
                Ok(Value::Bool(false))
            }
            _ => Err(Error::InvalidArguments("!==".into()))
        }
    }
}

impl GreaterThanOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(left.coerce_to_number() > right.coerce_to_number()))
            },
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if prev.coerce_to_number() <= curr.coerce_to_number() {
                        return Ok(Value::Bool(false));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(true))
            }
            args if args.len() < 2 => {
                Ok(Value::Bool(false))
            }
            _ => Err(Error::InvalidArguments(">".into()))
        }
    }
}

impl LessThanOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(left.coerce_to_number() < right.coerce_to_number()))
            }
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if prev.coerce_to_number() >= curr.coerce_to_number() {
                        return Ok(Value::Bool(false));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(true))
            }
            args if args.len() < 2 => {
                Ok(Value::Bool(false))
            }
            _ => Err(Error::InvalidArguments("<".into()))
        }
    }
}

impl LessThanEqualOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(left.coerce_to_number() <= right.coerce_to_number()))
            }
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if prev.coerce_to_number() > curr.coerce_to_number() {
                        return Ok(Value::Bool(false));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(true))
            }
            args if args.len() < 2 => {
                Ok(Value::Bool(false))
            }
            _ => Err(Error::InvalidArguments("<=".into()))
        }
    }
}

impl GreaterThanEqualOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(left.coerce_to_number() >= right.coerce_to_number()))
            }
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if prev.coerce_to_number() < curr.coerce_to_number() {
                        return Ok(Value::Bool(false));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(true))
            }
            args if args.len() < 2 => {
                Ok(Value::Bool(false))
            }
            _ => Err(Error::InvalidArguments(">=".into()))
        }
    }
}