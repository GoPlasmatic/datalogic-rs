use serde_json::Value;
use std::borrow::Cow;

use crate::{ContextStack, Evaluator, Operator, Result};

/// String concatenation operator (cat) - variadic
pub struct CatOperator;

impl Operator for CatOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        let mut result = String::new();

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;
            match value.as_ref() {
                Value::String(s) => result.push_str(s),
                Value::Number(n) => result.push_str(&n.to_string()),
                Value::Bool(b) => result.push_str(&b.to_string()),
                Value::Null => result.push_str("null"),
                _ => result.push_str(&value.to_string()),
            }
        }

        Ok(Cow::Owned(Value::String(result)))
    }
}

/// Substring operator (substr)
pub struct SubstrOperator;

impl Operator for SubstrOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.is_empty() {
            return Ok(Cow::Owned(Value::String(String::new())));
        }

        let string_val = evaluator.evaluate(&args[0], context)?;
        let string = match string_val.as_ref() {
            Value::String(s) => s.clone(),
            _ => string_val.to_string(),
        };

        let start = if args.len() > 1 {
            let start_val = evaluator.evaluate(&args[1], context)?;
            start_val.as_i64().unwrap_or(0) as isize
        } else {
            0
        };

        let length = if args.len() > 2 {
            let length_val = evaluator.evaluate(&args[2], context)?;
            Some(length_val.as_i64().unwrap_or(string.len() as i64) as isize)
        } else {
            None
        };

        let str_len = string.len() as isize;
        let actual_start = if start < 0 {
            ((str_len + start).max(0)) as usize
        } else {
            start.min(str_len) as usize
        };

        let result = if let Some(len) = length {
            if len < 0 {
                let end = (str_len + len).max(actual_start as isize) as usize;
                string
                    .chars()
                    .skip(actual_start)
                    .take(end - actual_start)
                    .collect()
            } else {
                string
                    .chars()
                    .skip(actual_start)
                    .take(len as usize)
                    .collect()
            }
        } else {
            string.chars().skip(actual_start).collect()
        };

        Ok(Cow::Owned(Value::String(result)))
    }
}

/// In operator - checks if a value is in a string or array
pub struct InOperator;

impl Operator for InOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let needle = evaluator.evaluate(&args[0], context)?;
        let haystack = evaluator.evaluate(&args[1], context)?;

        let result = match haystack.as_ref() {
            Value::String(s) => match needle.as_ref() {
                Value::String(n) => s.contains(n.as_str()),
                _ => false,
            },
            Value::Array(arr) => arr.iter().any(|v| v == needle.as_ref()),
            _ => false,
        };

        Ok(Cow::Owned(Value::Bool(result)))
    }
}
