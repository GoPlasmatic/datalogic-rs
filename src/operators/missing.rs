use serde_json::Value;

use crate::value_helpers::access_path_ref;
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// Missing operator function - checks for missing variables
#[inline]
pub fn evaluate_missing(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let mut missing = Vec::new();

    for arg in args {
        let path_val = engine.evaluate_node_cow(arg, context)?;

        match path_val.as_ref() {
            Value::Array(arr) => {
                for v in arr {
                    if let Some(path) = v.as_str()
                        && access_path_ref(context.current().data(), path).is_none()
                    {
                        missing.push(Value::String(path.to_string()));
                    }
                }
            }
            Value::String(s) => {
                if access_path_ref(context.current().data(), s).is_none() {
                    missing.push(Value::String(s.clone()));
                }
            }
            _ => {}
        }
    }

    Ok(Value::Array(missing))
}

/// MissingSome operator function - returns empty array if minimum present fields are met,
/// or array of missing fields otherwise
#[inline]
pub fn evaluate_missing_some(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Array(vec![]));
    }

    // First argument is the minimum number of fields that must be PRESENT
    let min_present_val = engine.evaluate_node_cow(&args[0], context)?;
    let min_present = min_present_val.as_u64().unwrap_or(1) as usize;

    let paths_val = engine.evaluate_node_cow(&args[1], context)?;

    let mut missing = Vec::new();
    let mut present_count = 0;

    if let Value::Array(arr) = paths_val.as_ref() {
        for v in arr {
            if let Some(path) = v.as_str() {
                if access_path_ref(context.current().data(), path).is_none() {
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
