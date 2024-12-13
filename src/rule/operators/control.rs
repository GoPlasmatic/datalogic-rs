use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

const ERR_TERNARY: &str = "?: requires 3 arguments";

pub struct IfOperator;
pub struct TernaryOperator;

#[inline]
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,  // Most common case first
        Value::Null => false,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

impl Operator for IfOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        // Fast paths
        match args.len() {
            0 => return Ok(Value::Null),
            1 => return args[0].apply(data),
            2 => {
                return if is_truthy(&args[0].apply(data)?) {
                    args[1].apply(data)
                } else {
                    Ok(Value::Null)
                }
            }
            _ => {}
        }

        // Process multiple conditions
        let chunks = args.chunks_exact(2);
        let remainder = chunks.remainder();

        for chunk in chunks {
            if is_truthy(&chunk[0].apply(data)?) {
                return chunk[1].apply(data);
            }
        }

        // Handle default case
        if let [default] = remainder {
            return default.apply(data);
        }

        Ok(Value::Null)
    }
}

impl Operator for TernaryOperator {
    #[inline]
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 3 {
            return Err(Error::InvalidArguments(ERR_TERNARY.into()));
        }

        if is_truthy(&args[0].apply(data)?) {
            args[1].apply(data)
        } else {
            args[2].apply(data)
        }
    }
}