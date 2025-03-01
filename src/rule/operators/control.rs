use serde_json::Value;
use crate::{rule::ArgType, Error};
use super::{Rule, ValueCoercion, StaticEvaluable};
use std::borrow::Cow;

pub struct IfOperator;
pub struct CoalesceOperator;

impl IfOperator {
    pub fn apply<'a>(&self, args: &'a ArgType, context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match args {
            ArgType::Multiple(args) => {
                match args.as_slice() {
                    [] => Ok(Cow::Owned(Value::Null)),
                    [single] => single.apply(context, root, path),
                    [condition, consequent] => {
                        let cond = condition.apply(context, root, path)?;
                        if cond.coerce_to_bool() {
                            consequent.apply(context, root, path)
                        } else {
                            Ok(Cow::Owned(Value::Null))
                        }
                    }
                    [condition, consequent, alternative] => {
                        let cond = condition.apply(context, root, path)?;
                        if cond.coerce_to_bool() {
                            consequent.apply(context, root, path)
                        } else {
                            alternative.apply(context, root, path)
                        }
                    }
                    _ => {
                        let chunks = args.chunks_exact(2);
                        let remainder = chunks.remainder();

                        for chunk in chunks {
                            if chunk[0].apply(context, root, path)?.coerce_to_bool() {
                                return chunk[1].apply(context, root, path);
                            }
                        }

                        match remainder {
                            [default] => default.apply(context, root, path),
                            _ => Ok(Cow::Owned(Value::Null)),
                        }
                    }
                }        
            },
            ArgType::Unary(_) => Err(Error::Custom("Invalid Arguments".into())),
        }
    }
}

impl StaticEvaluable for IfOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        match rule {
            Rule::If(args) => match args {
                ArgType::Multiple(arr) => arr.iter().all(|r| r.is_static()),
                ArgType::Unary(r) => r.is_static(),
            },
            _ => false,
        }
    }
}

impl CoalesceOperator {
    pub fn apply<'a>(&self, args: &'a [Rule], context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        for arg in args {
            let result = arg.apply(context, root, path)?;
            if !result.is_null_value() {
                return Ok(result);
            }
        }
        Ok(Cow::Owned(Value::Null))
    }
}

impl StaticEvaluable for CoalesceOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        match rule {
            Rule::Coalesce(args) => args.iter().all(|r| r.is_static()),
            _ => false,
        }
    }
}