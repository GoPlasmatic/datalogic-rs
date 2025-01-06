use serde_json::Value;
use crate::Error;
use super::{Rule, ValueCoercion};

const ERR_TERNARY: &str = "?: requires 3 arguments";

pub struct IfOperator;
pub struct TernaryOperator;


impl IfOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [] => Ok(Value::Null),
            [single] => single.apply(data),
            [condition, consequent] => {
                let cond = condition.apply(data)?;
                if cond.coerce_to_bool() {
                    consequent.apply(data)
                } else {
                    Ok(Value::Null)
                }
            }
            [condition, consequent, alternative] => {
                let cond = condition.apply(data)?;
                if cond.coerce_to_bool() {
                    consequent.apply(data)
                } else {
                    alternative.apply(data)
                }
            }
            _ => {
                // Optimized multiple condition handling
                let chunks = args.chunks_exact(2);
                let remainder = chunks.remainder();

                // Use iterator instead of collecting into Vec
                for chunk in chunks {
                    if chunk[0].apply(data)?.coerce_to_bool() {
                        return chunk[1].apply(data);
                    }
                }

                // Default case optimization
                match remainder {
                    [default] => default.apply(data),
                    _ => Ok(Value::Null),
                }
            }
        }
    }
}

impl TernaryOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [condition, consequent, alternative] => {
                let cond = condition.apply(data)?;
                if cond.coerce_to_bool() {
                    consequent.apply(data)
                } else {
                    alternative.apply(data)
                }
            }
            _ => Err(Error::InvalidArguments(ERR_TERNARY.into()))
        }
    }
}