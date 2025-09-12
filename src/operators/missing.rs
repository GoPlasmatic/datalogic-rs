use serde_json::Value;

use crate::value_helpers::access_path;
use crate::{ContextStack, Evaluator, Operator, Result};

/// Missing operator - checks for missing variables
pub struct MissingOperator;

impl Operator for MissingOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        let mut missing = Vec::new();

        for arg in args {
            let path_val = evaluator.evaluate(arg, context)?;

            match &path_val {
                Value::Array(arr) => {
                    for v in arr {
                        if let Some(path) = v.as_str()
                            && access_path(context.current().data(), path).is_none()
                        {
                            missing.push(Value::String(path.to_string()));
                        }
                    }
                }
                Value::String(s) => {
                    if access_path(context.current().data(), s).is_none() {
                        missing.push(Value::String(s.clone()));
                    }
                }
                _ => {}
            }
        }

        Ok(Value::Array(missing))
    }
}

/// MissingSome operator - returns empty array if minimum present fields are met,
/// or array of missing fields otherwise
pub struct MissingSomeOperator;

impl Operator for MissingSomeOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Array(vec![]));
        }

        // First argument is the minimum number of fields that must be PRESENT
        let min_present_val = evaluator.evaluate(&args[0], context)?;
        let min_present = min_present_val.as_u64().unwrap_or(1) as usize;

        let paths_val = evaluator.evaluate(&args[1], context)?;

        let mut missing = Vec::new();
        let mut present_count = 0;

        if let Value::Array(arr) = &paths_val {
            for v in arr {
                if let Some(path) = v.as_str() {
                    if access_path(context.current().data(), path).is_none() {
                        missing.push(Value::String(path.to_string()));
                    } else {
                        present_count += 1;
                        // Early exit if we've found enough present fields
                        if present_count >= min_present {
                            return Ok(Value::Array(vec![]));
                        }
                    }
                }
            }
        }

        // Return empty array if minimum present requirement is met,
        // otherwise return the array of missing fields
        if present_count >= min_present {
            Ok(Value::Array(vec![]))
        } else {
            Ok(Value::Array(missing))
        }
    }
}
