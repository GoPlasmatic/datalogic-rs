use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

pub struct IfOperator;
pub struct TernaryOperator;

impl Operator for IfOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.is_empty() {
            return Ok(Value::Null);
        }

        for chunk in args.chunks(2) {
            // Last argument is the default case
            if chunk.len() == 1 {
                return chunk[0].apply(data);
            }

            // Evaluate condition
            let condition = chunk[0].apply(data)?;
            if is_truthy(&condition) {
                return chunk[1].apply(data);
            }
        }

        Ok(Value::Null)
    }
}

impl Operator for TernaryOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 3 {
            return Err(Error::InvalidArguments("?: requires 3 arguments".to_string()));
        }

        let condition = args[0].apply(data)?;
        if is_truthy(&condition) {
            args[1].apply(data)
        } else {
            args[2].apply(data)
        }
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}