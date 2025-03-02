use serde_json::Value;
use crate::Error;
use super::{Rule, ValueCoercion, StaticEvaluable};
use std::borrow::Cow;

pub struct InOperator;
pub struct CatOperator;
pub struct SubstrOperator;

impl InOperator {
    pub fn apply<'a>(&self, search: &Rule, target: &Rule, context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        let search = search.apply(context, root, path)?;
        let target = target.apply(context, root, path)?;
        
        Ok(Cow::Owned(Value::Bool(match (&*search, &*target) {
            (Value::String(s), Value::String(t)) => t.contains(s),
            (_, Value::Array(arr)) => arr.contains(&*search),
            _ => false,
        })))
    }
}

impl StaticEvaluable for InOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        match rule {
            Rule::In(search, target) => search.is_static() && target.is_static(),
            _ => false,
        }
    }
}

impl CatOperator {
    pub fn apply<'a>(&self, args: &[Rule], context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        // Fast paths
        match args.len() {
            0 => return Ok(Cow::Owned(Value::String(String::new()))),
            1 => {
                let value = args[0].apply(context, root, path)?;
                return Ok(Cow::Owned(Value::String(value.coerce_to_string())));
            }
            _ => {}
        }

        // Pre-allocate with estimated capacity
        let capacity = args.len() * 16;
        let mut result = String::with_capacity(capacity);

        for arg in args {
            let value = arg.apply(context, root, path)?;
            Value::coerce_append(&mut result, &value);
        }

        Ok(Cow::Owned(Value::String(result)))
    }
}

impl StaticEvaluable for CatOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        match rule {
            Rule::Cat(args) => args.iter().all(|r| r.is_static()),
            _ => false,
        }
    }
}

impl SubstrOperator {
    pub fn apply<'a>(&self, string: &Rule, start: &Rule, length: Option<&Rule>, context: &Value, root: &Value, path: &str) 
        -> Result<Cow<'a, Value>, Error> 
    {
        let string = string.apply(context, root, path)?;
        let string = match &*string {
            Value::String(s) => s,
            v => &v.coerce_to_string(),
        };

        let chars: Vec<char> = string.chars().collect();
        let str_len = chars.len() as i64;

        let start = start.apply(context, root, path)?;
        let start_idx = match &*start {
            Value::Number(n) => {
                let start = n.as_i64().unwrap_or(0);
                if start < 0 {
                    (str_len + start).max(0) as usize
                } else {
                    start.min(str_len) as usize
                }
            },
            _ => return Ok(Cow::Owned(Value::String(String::new()))),
        };

        let length = if let Some(length_rule) = length {
            Some(length_rule.apply(context, root, path)?)
        } else {
            None
        };

        match length.as_deref() {
            Some(Value::Number(n)) => {
                let len = n.as_i64().unwrap_or(0);
                let end_idx = if len < 0 {
                    (str_len + len) as usize
                } else {
                    (start_idx + len as usize).min(chars.len())
                };
                
                if end_idx <= start_idx {
                    Ok(Cow::Owned(Value::String(String::new())))
                } else {
                    Ok(Cow::Owned(Value::String(chars[start_idx..end_idx].iter().collect())))
                }
            },
            None => {
                Ok(Cow::Owned(Value::String(chars[start_idx..].iter().collect())))
            },
            _ => Ok(Cow::Owned(Value::String(String::new()))),
        }
    }
}

impl StaticEvaluable for SubstrOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        match rule {
            Rule::Substr(string, start, length) => 
                string.is_static() && start.is_static() && 
                length.as_ref().map_or(true, |l| l.is_static()),
            _ => false,
        }
    }
}