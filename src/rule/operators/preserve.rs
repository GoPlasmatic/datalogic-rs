use serde_json::Value;
use crate::{rule::ArgType, Error, JsonLogicResult};

pub struct PreserveOperator;

impl PreserveOperator {
    pub fn apply(&self, arg: &ArgType, data: &Value) -> JsonLogicResult {
        match arg {
            ArgType::Single(rule) => rule.apply(data),
            ArgType::Array(rules) => {
                if rules.is_empty() {
                    return Err(Error::InvalidArguments("preserve requires 1 argument".to_string()));
                }
                rules[0].apply(data)
            }
        }
    }
}