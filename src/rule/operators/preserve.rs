use serde_json::Value;
use crate::{rule::ArgType, Error};
use std::borrow::Cow;

pub struct PreserveOperator;

impl PreserveOperator {
    pub fn apply<'a>(&self, arg: &'a ArgType, context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => rule.apply(context, root, path),
            ArgType::Multiple(rules) => {
                if rules.is_empty() {
                    return Ok(Cow::Owned(Value::Array(Vec::new())));
                }

                let mut result_arr = Vec::with_capacity(rules.len());
                for rule in rules {
                    let value = rule.apply(context, root, path)?;
                    result_arr.push(value.into_owned());
                }
                
                Ok(Cow::Owned(Value::Array(result_arr)))
            }
        }
    }
}