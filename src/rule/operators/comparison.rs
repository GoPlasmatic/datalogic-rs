use serde_json::Value;
use crate::Error;
use super::{Operator, Rule, ValueCoercion};

pub struct EqualsOperator;
pub struct StrictEqualsOperator;
pub struct NotEqualsOperator;
pub struct StrictNotEqualsOperator;
pub struct GreaterThanOperator;
pub struct LessThanOperator;
pub struct GreaterThanEqualOperator;
pub struct LessThanEqualOperator;


impl Operator for EqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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
            _ => Err(Error::InvalidArguments("==".into()))
        }
    }
}

impl Operator for StrictEqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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
            _ => Err(Error::InvalidArguments("===".into()))
        }
    }
}

impl Operator for NotEqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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
            _ => Err(Error::InvalidArguments("!=".into()))
        }
    }
}

impl Operator for StrictNotEqualsOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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
            _ => Err(Error::InvalidArguments("!==".into()))
        }
    }
}

impl Operator for GreaterThanOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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
            _ => Err(Error::InvalidArguments(">".into()))
        }
    }
}

impl Operator for LessThanOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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
            _ => Err(Error::InvalidArguments("<".into()))
        }
    }
}

impl Operator for LessThanEqualOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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
            _ => Err(Error::InvalidArguments("<=".into()))
        }
    }
}

impl Operator for GreaterThanEqualOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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
            _ => Err(Error::InvalidArguments(">=".into()))
        }
    }
}