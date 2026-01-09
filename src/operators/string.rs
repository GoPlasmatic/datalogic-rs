use serde_json::Value;

use super::helpers::to_string;
use crate::constants::INVALID_ARGS;
use crate::{CompiledNode, ContextStack, DataLogic, Result, error::Error};

/// String concatenation operator function (cat) - variadic
#[inline]
pub fn evaluate_cat(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let mut result = String::new();

    for arg in args {
        let value = engine.evaluate_node(arg, context)?;
        // If the value is an array, concatenate its elements
        if let Value::Array(arr) = value {
            for item in arr {
                result.push_str(&to_string(&item));
            }
        } else {
            result.push_str(&to_string(&value));
        }
    }

    Ok(Value::String(result))
}

/// Substring operator function (substr)
#[inline]
pub fn evaluate_substr(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::String(String::new()));
    }

    let string_val = engine.evaluate_node(&args[0], context)?;
    let string = match &string_val {
        Value::String(s) => s.clone(),
        _ => string_val.to_string(),
    };

    // Get character count for proper bounds checking
    let char_count = string.chars().count();

    let start = if args.len() > 1 {
        let start_val = engine.evaluate_node(&args[1], context)?;
        start_val.as_i64().unwrap_or(0)
    } else {
        0
    };

    let length = if args.len() > 2 {
        let length_val = engine.evaluate_node(&args[2], context)?;
        length_val.as_i64()
    } else {
        None
    };

    // Safe bounds checking with overflow protection
    let actual_start = if start < 0 {
        // Safely handle negative indices
        let abs_start = start.saturating_abs() as usize;
        char_count.saturating_sub(abs_start)
    } else {
        // Safely handle positive indices
        (start as usize).min(char_count)
    };

    let result = if let Some(len) = length {
        if len < 0 {
            // Special case: negative length means use it as end position (like slice)
            // This mimics JSONLogic's behavior which differs from JavaScript's substr
            let end_pos = if len < 0 {
                // Negative end position counts from end of string
                let abs_end = len.saturating_abs() as usize;
                char_count.saturating_sub(abs_end)
            } else {
                0
            };

            // Take characters from actual_start to end_pos
            if end_pos > actual_start {
                string
                    .chars()
                    .skip(actual_start)
                    .take(end_pos - actual_start)
                    .collect()
            } else {
                String::new()
            }
        } else if len == 0 {
            // Zero length returns empty string
            String::new()
        } else {
            // Positive length - take from start position
            let take_count = (len as usize).min(char_count.saturating_sub(actual_start));
            string.chars().skip(actual_start).take(take_count).collect()
        }
    } else {
        // No length specified - take rest of string
        string.chars().skip(actual_start).collect()
    };

    Ok(Value::String(result))
}

/// In operator function - checks if a value is in a string or array
#[inline]
pub fn evaluate_in(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Bool(false));
    }

    let needle = engine.evaluate_node(&args[0], context)?;
    let haystack = engine.evaluate_node(&args[1], context)?;

    let result = match &haystack {
        Value::String(s) => match &needle {
            Value::String(n) => s.contains(n.as_str()),
            _ => false,
        },
        Value::Array(arr) => arr.iter().any(|v| v == &needle),
        _ => false,
    };

    Ok(Value::Bool(result))
}

/// Length operator function - returns the length of a string or array
#[inline]
pub fn evaluate_length(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() || args.len() > 1 {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    // First evaluate the argument
    let value = engine.evaluate_node(&args[0], context)?;

    match value {
        Value::String(s) => {
            // Count Unicode code points (characters)
            let char_count = s.chars().count();
            // Ensure count fits in i64 (though this is practically impossible to overflow)
            if char_count > i64::MAX as usize {
                return Err(Error::InvalidArguments("String too long".to_string()));
            }
            Ok(Value::Number(serde_json::Number::from(char_count as i64)))
        }
        Value::Array(arr) => {
            // Ensure array length fits in i64
            if arr.len() > i64::MAX as usize {
                return Err(Error::InvalidArguments("Array too long".to_string()));
            }
            Ok(Value::Number(serde_json::Number::from(arr.len() as i64)))
        }
        // For null, numbers, booleans, and objects, length is invalid
        Value::Null | Value::Number(_) | Value::Bool(_) | Value::Object(_) => {
            Err(Error::InvalidArguments(INVALID_ARGS.to_string()))
        }
    }
}
