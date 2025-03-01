use serde_json::Value;
use super::ArgType;
use super::Rule;
use super::Error;

pub mod arithmetic;
pub mod array;
pub mod comparison;
pub mod logic;
pub mod missing;
pub mod string;
pub mod var;
pub mod control;
pub mod val;
pub mod tryop;
pub mod custom;

pub use arithmetic::*;
pub use array::*;
pub use comparison::*;
pub use logic::*;
pub use missing::*;
pub use string::*;
pub use var::*;
pub use control::*;
pub use val::*;
pub use tryop::*;
pub use custom::*;


trait ValueCoercion {
    fn is_null_value(&self) -> bool;
    fn coerce_to_bool(&self) -> bool;
    fn coerce_to_number(&self) -> Result<f64, Error>;
    fn coerce_to_string(&self) -> String;
    fn coerce_append(result: &mut String, value: &Value);
}

impl ValueCoercion for Value {
    fn coerce_to_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => {
                let num = n.as_f64().unwrap_or(0.0);
                num != 0.0
            },
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(_) => true,
            Value::Null => false,
        }
    }

    fn coerce_to_number(&self) -> Result<f64, Error> {
        match self {
            Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
            Value::Null => Ok(0.0),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::String(s) => {
                if s.is_empty() {
                    return Ok(0.0);
                }
                s.parse::<f64>().map_err(|_| Error::Custom("NaN".to_string()))
            },
            _ => Err(Error::Custom("NaN".to_string())),
        }
    }

    fn coerce_to_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Array(arr) => {
                let mut result = String::with_capacity(arr.len() * 8);
                for item in arr.iter() {
                    Self::coerce_append(&mut result, item);
                }
                result
            },
            Value::Object(_) => "[object Object]".to_string(),
        }
    }

    fn coerce_append(result: &mut String, value: &Value) {
        match value {
            Value::String(s) => result.push_str(s),
            Value::Number(n) => result.push_str(&n.to_string()),
            Value::Bool(b) => result.push_str(if *b { "true" } else { "false" }),
            Value::Null => result.push_str(""),
            Value::Array(arr) => {
                for item in arr.iter() {
                    Self::coerce_append(result, item);
                }
            },
            Value::Object(_) => result.push_str("[object Object]"),
        }
    }

    fn is_null_value(&self) -> bool {
        match self {
            Value::Bool(_) => false,
            Value::Number(_) => false,
            Value::String(s) => s.is_empty(),
            Value::Array(a) => a.is_empty(),
            Value::Object(o) => o.is_empty(),
            Value::Null => true,
        }
    }
}

trait ValueConvert {
    fn to_value(&self) -> Value;
}

impl ValueConvert for f64 {
    fn to_value(&self) -> Value {
        const ZERO_FRACT: f64 = 0.0;
        if self.fract() == ZERO_FRACT {
            Value::Number(serde_json::Number::from(*self as i64))
        } else {
            Value::Number(serde_json::Number::from_f64(*self).unwrap())
        }
    }
}

trait ValueExt {
    fn strict_equals(&self, other: &Value) -> Result<bool, Error>;
    fn strict_not_equals(&self, other: &Value) -> Result<bool, Error>;
    fn equals(&self, other: &Value) -> Result<bool, Error>;
    fn not_equals(&self, other: &Value) -> Result<bool, Error>;
    fn greater_than(&self, other: &Value) -> Result<bool, Error>;
    fn greater_than_equal(&self, other: &Value) -> Result<bool, Error>;
    fn less_than(&self, other: &Value) -> Result<bool, Error>;
    fn less_than_equal(&self, other: &Value) -> Result<bool, Error>;
}

impl ValueExt for Value {
    fn strict_equals(&self, other: &Value) -> Result<bool, Error> {
        Ok(std::mem::discriminant(self) == std::mem::discriminant(other) && self.equals(other)?)
    }

    fn strict_not_equals(&self, other: &Value) -> Result<bool, Error> {
        Ok(std::mem::discriminant(self) != std::mem::discriminant(other) || self.not_equals(other)?)
    }

    fn equals(&self, other: &Value) -> Result<bool, Error> {
        match (self, other) {
            (Value::Number(n1), Value::Number(n2)) => Ok(n1 == n2),
            (Value::String(s1), Value::String(s2)) => Ok(s1 == s2),
            (Value::Bool(b1), Value::Bool(b2)) => Ok(b1 == b2),
            _ => {
                let n1 = self.coerce_to_number()?;
                let n2 = other.coerce_to_number()?;
                Ok(n1 == n2)
            },
        }
    }

    fn not_equals(&self, other: &Value) -> Result<bool, Error> {
        Ok(!self.equals(other)?)
    }

    fn greater_than(&self, other: &Value) -> Result<bool, Error> {
        match (self, other) {
            (Value::Number(n1), Value::Number(n2)) => Ok(n1.as_f64() > n2.as_f64()),
            (Value::String(s1), Value::String(s2)) => Ok(s1 > s2),
            _ => {
                let n1 = self.coerce_to_number()?;
                let n2 = other.coerce_to_number()?;
                Ok(n1 > n2)
            },
        }
    }

    fn greater_than_equal(&self, other: &Value) -> Result<bool, Error> {
        match (self, other) {
            (Value::Number(n1), Value::Number(n2)) => Ok(n1.as_f64() >= n2.as_f64()),
            (Value::String(s1), Value::String(s2)) => Ok(s1 >= s2),
            _ => {
                let n1 = self.coerce_to_number()?;
                let n2 = other.coerce_to_number()?;
                Ok(n1 >= n2)
            },
        }
    }

    fn less_than(&self, other: &Value) -> Result<bool, Error> {
        match (self, other) {
            (Value::Number(n1), Value::Number(n2)) => Ok(n1.as_f64() < n2.as_f64()),
            (Value::String(s1), Value::String(s2)) => Ok(s1 < s2),
            _ => {
                let n1 = self.coerce_to_number()?;
                let n2 = other.coerce_to_number()?;
                Ok(n1 < n2)
            },
        }
    }

    fn less_than_equal(&self, other: &Value) -> Result<bool, Error> {
        match (self, other) {
            (Value::Number(n1), Value::Number(n2)) => Ok(n1.as_f64() <= n2.as_f64()),
            (Value::String(s1), Value::String(s2)) => Ok(s1 <= s2),
            _ => {
                let n1 = self.coerce_to_number()?;
                let n2 = other.coerce_to_number()?;
                Ok(n1 <= n2)
            },
        }
    }
}

#[inline(always)]
fn is_current_var(var_name: &Rule) -> bool {
    match var_name {
        Rule::Value(Value::String(name)) => name == "current",
        Rule::Value(Value::Array(arr)) if !arr.is_empty() => {
            if let Some(Value::String(first)) = arr.first() {
                first == "current"
            } else {
                false
            }
        }
        _ => false
    }
}

#[inline]
pub fn is_flat_arithmetic_predicate(rule: &Rule) -> bool {
    if let Rule::Arithmetic(op_type, ArgType::Multiple(args)) = rule {
        if args.len() == 2 {
            // Check if operation is arithmetic
            if !matches!(op_type, 
                ArithmeticType::Add | 
                ArithmeticType::Subtract | 
                ArithmeticType::Multiply | 
                ArithmeticType::Divide | 
                ArithmeticType::Modulo
            ) {
                return false;
            }

            // Check if we have one current/current.* and one accumulator using array_windows
            let (has_current, has_accumulator) = args.iter().fold(
                (false, false),
                |mut acc, arg| {
                    if let Rule::Var(var_name, _) = arg {
                        match &**var_name {
                            Rule::Value(Value::String(name)) if name == "accumulator" => {
                                acc.1 = true;
                            }
                            _ if is_current_var(var_name) => {
                                acc.0 = true;
                            }
                            _ => {}
                        }
                    }
                    acc
                }
            );

            return has_current && has_accumulator;
        }
    }
    false
}