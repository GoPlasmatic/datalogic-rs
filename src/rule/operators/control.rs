use serde_json::Value;
use crate::Error;
use super::{Operator, Rule, ValueCoercion};

const ERR_TERNARY: &str = "?: requires 3 arguments";

pub struct IfOperator;
pub struct TernaryOperator;


impl Operator for IfOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        // Fast paths
        match args.len() {
            0 => return Ok(Value::Null),
            1 => return args[0].apply(data),
            2 => {
                return if args[0].apply(data)?.coerce_to_bool() {
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
            if chunk[0].apply(data)?.coerce_to_bool() {
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

        if args[0].apply(data)?.coerce_to_bool() {
            args[1].apply(data)
        } else {
            args[2].apply(data)
        }
    }
}