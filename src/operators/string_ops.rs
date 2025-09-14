use regex::Regex;
use serde_json::{Value, json};

use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// StartsWithOperator function - checks if a string starts with a prefix
#[inline]
pub fn evaluate_starts_with(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let prefix = engine.evaluate_node(&args[1], context)?;

    let text_str = text.as_str().unwrap_or("");
    let prefix_str = prefix.as_str().unwrap_or("");

    Ok(Value::Bool(text_str.starts_with(prefix_str)))
}

/// EndsWithOperator function - checks if a string ends with a suffix
#[inline]
pub fn evaluate_ends_with(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let suffix = engine.evaluate_node(&args[1], context)?;

    let text_str = text.as_str().unwrap_or("");
    let suffix_str = suffix.as_str().unwrap_or("");

    Ok(Value::Bool(text_str.ends_with(suffix_str)))
}

/// UpperOperator function - converts a string to uppercase
#[inline]
pub fn evaluate_upper(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    let text = value.as_str().unwrap_or("");

    Ok(Value::String(text.to_uppercase()))
}

/// LowerOperator function - converts a string to lowercase
#[inline]
pub fn evaluate_lower(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    let text = value.as_str().unwrap_or("");

    Ok(Value::String(text.to_lowercase()))
}

/// TrimOperator function - removes leading and trailing whitespace from a string
#[inline]
pub fn evaluate_trim(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    let text = value.as_str().unwrap_or("");

    Ok(Value::String(text.trim().to_string()))
}

/// SplitOperator function - splits a string by delimiter or extracts regex groups
#[inline]
pub fn evaluate_split(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let delimiter = engine.evaluate_node(&args[1], context)?;

    let text_str = text.as_str().unwrap_or("");
    let delimiter_str = delimiter.as_str().unwrap_or("");

    // Check if delimiter is a regex pattern with named groups
    if delimiter_str.contains("(?P<") {
        // Try to parse as regex
        match Regex::new(delimiter_str) {
            Ok(re) => {
                // Check if regex has named groups
                let capture_names: Vec<_> = re.capture_names().flatten().collect();

                if !capture_names.is_empty() {
                    // Extract named groups
                    if let Some(captures) = re.captures(text_str) {
                        let mut result = serde_json::Map::new();

                        for name in capture_names {
                            if let Some(m) = captures.name(name) {
                                result.insert(
                                    name.to_string(),
                                    Value::String(m.as_str().to_string()),
                                );
                            }
                        }

                        return Ok(Value::Object(result));
                    } else {
                        // No match, return empty object
                        return Ok(Value::Object(serde_json::Map::new()));
                    }
                }
            }
            Err(_) => {
                // Invalid regex, fall back to normal split
            }
        }
    }

    // Normal string split
    if text_str.is_empty() {
        Ok(json!([""]))
    } else if delimiter_str.is_empty() {
        // Split into individual characters if delimiter is empty
        let chars: Vec<Value> = text_str
            .chars()
            .map(|c| Value::String(c.to_string()))
            .collect();
        Ok(Value::Array(chars))
    } else {
        let parts: Vec<Value> = text_str
            .split(delimiter_str)
            .map(|s| Value::String(s.to_string()))
            .collect();
        Ok(Value::Array(parts))
    }
}
