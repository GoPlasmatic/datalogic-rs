use serde_json::Value;
use crate::{rule::ArgType, Error};
use super::{Rule, ValueCoercion};
use std::borrow::Cow;

pub struct IfOperator;
pub struct TernaryOperator;
pub struct CoalesceOperator;

impl IfOperator {
    pub fn apply<'a>(&self, args: &'a ArgType, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match args {
            ArgType::Multiple(args) => {
                match args.as_slice() {
                    [] => Ok(Cow::Owned(Value::Null)),
                    [single] => single.apply(data),
                    [condition, consequent] => {
                        let cond = condition.apply(data)?;
                        if cond.coerce_to_bool() {
                            consequent.apply(data)
                        } else {
                            Ok(Cow::Owned(Value::Null))
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
                        let chunks = args.chunks_exact(2);
                        let remainder = chunks.remainder();

                        for chunk in chunks {
                            if chunk[0].apply(data)?.coerce_to_bool() {
                                return chunk[1].apply(data);
                            }
                        }

                        match remainder {
                            [default] => default.apply(data),
                            _ => Ok(Cow::Owned(Value::Null)),
                        }
                    }
                }        
            },
            ArgType::Unary(_) => Err(Error::Custom("Invalid Arguments".into())),
        }
    }
}

impl TernaryOperator {
    pub fn apply<'a>(&self, args: &'a [Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match args {
            [condition, consequent, alternative] => {
                let cond = condition.apply(data)?;
                if cond.coerce_to_bool() {
                    consequent.apply(data)
                } else {
                    alternative.apply(data)
                }
            }
            _ => Err(Error::Custom("Invalid Arguments".into()))
        }
    }
}

impl CoalesceOperator {
    pub fn apply<'a>(&self, args: &'a [Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        for arg in args {
            let result = arg.apply(data)?;
            if !result.is_null_value() {
                return Ok(result);
            }
        }
        Ok(Cow::Owned(Value::Null))
    }
}