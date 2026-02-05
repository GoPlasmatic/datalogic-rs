use regex::Regex;
use serde_json::{Value, json};

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
    let string: std::borrow::Cow<str> = match &string_val {
        Value::String(s) => std::borrow::Cow::Borrowed(s.as_str()),
        _ => std::borrow::Cow::Owned(string_val.to_string()),
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
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
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
            Err(Error::InvalidArguments(INVALID_ARGS.into()))
        }
    }
}

/// StartsWithOperator function - checks if a string starts with a prefix
#[inline]
pub fn evaluate_starts_with(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let text_str = text.as_str().unwrap_or("");

    // Fast path: pattern is a literal string (most common case) — avoid clone
    if let CompiledNode::Value {
        value: Value::String(p),
        ..
    } = &args[1]
    {
        return Ok(Value::Bool(text_str.starts_with(p.as_str())));
    }

    let pattern = engine.evaluate_node(&args[1], context)?;
    let pattern_str = pattern.as_str().unwrap_or("");
    Ok(Value::Bool(text_str.starts_with(pattern_str)))
}

/// EndsWithOperator function - checks if a string ends with a suffix
#[inline]
pub fn evaluate_ends_with(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let text_str = text.as_str().unwrap_or("");

    // Fast path: pattern is a literal string (most common case) — avoid clone
    if let CompiledNode::Value {
        value: Value::String(p),
        ..
    } = &args[1]
    {
        return Ok(Value::Bool(text_str.ends_with(p.as_str())));
    }

    let pattern = engine.evaluate_node(&args[1], context)?;
    let pattern_str = pattern.as_str().unwrap_or("");
    Ok(Value::Bool(text_str.ends_with(pattern_str)))
}

/// UpperOperator function - converts a string to uppercase
#[inline]
pub fn evaluate_upper(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    // Fast path: if ASCII and already uppercase, return original value (no allocation)
    let already_upper = value
        .as_str()
        .is_some_and(|s| s.is_ascii() && !s.bytes().any(|b| b.is_ascii_lowercase()));
    if already_upper {
        return Ok(value);
    }
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
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    // Fast path: if ASCII and already lowercase, return original value (no allocation)
    let already_lower = value
        .as_str()
        .is_some_and(|s| s.is_ascii() && !s.bytes().any(|b| b.is_ascii_uppercase()));
    if already_lower {
        return Ok(value);
    }
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
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let value = engine.evaluate_node(&args[0], context)?;
    // Fast path: check if trimming is needed before allocating
    let needs_trim = value.as_str().is_some_and(|s| {
        !s.is_empty() && {
            // chars().next() and next_back() are O(1) for valid UTF-8
            s.starts_with(|c: char| c.is_whitespace()) || s.ends_with(|c: char| c.is_whitespace())
        }
    });
    if !needs_trim {
        return match &value {
            Value::String(_) => Ok(value),
            _ => Ok(Value::String(String::new())),
        };
    }
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
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let text_str = text.as_str().unwrap_or("");

    // Fast path: delimiter is a literal string — skip regex check entirely.
    // Valid regex patterns are already handled at compile-time via CompiledSplitRegex,
    // so any remaining literal delimiter is guaranteed to be a plain string split.
    if let CompiledNode::Value {
        value: Value::String(delim),
        ..
    } = &args[1]
    {
        return split_normal(text_str, delim.as_str());
    }

    let delimiter = engine.evaluate_node(&args[1], context)?;
    let delimiter_str = delimiter.as_str().unwrap_or("");

    // Check if delimiter is a regex pattern with named groups (dynamic delimiter case)
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
    split_normal(text_str, delimiter_str)
}

/// Split with a pre-compiled regex (used when regex is known at compile time)
#[inline]
pub fn evaluate_split_with_regex(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    regex: &Regex,
    capture_names: &[Box<str>],
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let text = engine.evaluate_node(&args[0], context)?;
    let text_str = text.as_str().unwrap_or("");

    if let Some(captures) = regex.captures(text_str) {
        let mut result = serde_json::Map::new();
        for name in capture_names {
            if let Some(m) = captures.name(name) {
                result.insert(name.to_string(), Value::String(m.as_str().to_string()));
            }
        }
        Ok(Value::Object(result))
    } else {
        Ok(Value::Object(serde_json::Map::new()))
    }
}

#[inline]
fn split_normal(text_str: &str, delimiter_str: &str) -> Result<Value> {
    if text_str.is_empty() {
        Ok(json!([""]))
    } else if delimiter_str.is_empty() {
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
