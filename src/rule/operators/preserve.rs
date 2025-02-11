use serde_json::Value;
use crate::{rule::ArgType, JsonLogicResult};

pub struct PreserveOperator;

impl PreserveOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Unary(rule) => rule.apply(data),
            ArgType::Multiple(rules) => {
                let mut result_arr = Vec::with_capacity(rules.len());
                for rule in rules {
                    result_arr.push(rule.apply(data)?);
                }
                Ok(Value::Array(result_arr))
            }
        }
    }
}