use serde_json::Value;
use std::borrow::Cow;

use crate::value_helpers::access_path;
use crate::{ContextStack, Evaluator, Operator, Result};

/// Missing operator - checks for missing variables
pub struct MissingOperator;

impl Operator for MissingOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        let mut missing = Vec::new();

        for arg in args {
            let path_val = evaluator.evaluate(arg, context)?;

            let paths = match path_val.as_ref() {
                Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
                Value::String(s) => vec![s.clone()],
                _ => vec![],
            };

            for path in paths {
                if access_path(&context.current().data, &path).is_none() {
                    missing.push(Value::String(path));
                }
            }
        }

        Ok(Cow::Owned(Value::Array(missing)))
    }
}

/// MissingSome operator - returns empty array if minimum present fields are met,
/// or array of missing fields otherwise
pub struct MissingSomeOperator;

impl Operator for MissingSomeOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Array(vec![])));
        }

        // First argument is the minimum number of fields that must be PRESENT
        let min_present = evaluator
            .evaluate(&args[0], context)?
            .as_ref()
            .as_u64()
            .unwrap_or(1) as usize;

        let paths_val = evaluator.evaluate(&args[1], context)?;
        let paths = match paths_val.as_ref() {
            Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>(),
            _ => vec![],
        };

        let mut missing = Vec::new();
        let mut present_count = 0;

        for path in &paths {
            if access_path(&context.current().data, path).is_none() {
                missing.push(Value::String(path.clone()));
            } else {
                present_count += 1;
            }
        }

        // Return empty array if minimum present requirement is met,
        // otherwise return the array of missing fields
        if present_count >= min_present {
            Ok(Cow::Owned(Value::Array(vec![])))
        } else {
            Ok(Cow::Owned(Value::Array(missing)))
        }
    }
}
