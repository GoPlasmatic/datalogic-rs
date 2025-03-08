//! String operators for logic expressions.
//!
//! This module provides implementations for string operators
//! such as cat, substr, etc.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;

/// Enumeration of string operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringOp {
    /// String concatenation
    Cat,
    /// Substring extraction
    Substr,
}

/// Evaluates a cat operation (string concatenation).
/// Concatenates all arguments into a single string.
pub fn eval_cat<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // If no arguments, return empty string
    if args.is_empty() {
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(""))));
    }

    // If only one argument, convert it to string directly
    if args.len() == 1 {
        let value = evaluate(&args[0], data, arena)?;
        
        // If it's already a string, return it directly
        if let DataValue::String(_) = value {
            return Ok(value);
        }
        
        // Otherwise, convert to string
        let string_value = value.to_string();
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(&string_value))));
    }
    
    // For multiple arguments, concatenate them
    let mut result = String::new();
    
    for arg in args {
        let value = evaluate(arg, data, arena)?;
        match value {
            DataValue::String(s) => result.push_str(s),
            _ => {
                let string_value = value.to_string();
                result.push_str(&string_value);
            }
        }
    }
    
    // Allocate the result string in the arena
    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))))
}

/// Evaluates a substr operation (substring extraction).
/// Extracts a substring from a string based on start and length arguments.
pub fn eval_substr<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check that we have at least 2 arguments
    if args.len() < 2 {
        return Err(LogicError::OperatorError {
            operator: "substr".to_string(),
            reason: format!("Expected at least 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument (the string)
    let string_value = evaluate(&args[0], data, arena)?;
    
    // Get the string from the value
    let string = match string_value {
        DataValue::String(s) => s,
        _ => {
            // Convert to string
            let s = string_value.to_string();
            arena.alloc_str(&s)
        }
    };
    
    // If the string is empty, return empty string
    if string.is_empty() {
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(""))));
    }
    
    // Evaluate the second argument (start index)
    let start_value = evaluate(&args[1], data, arena)?;
    
    // Get the start index
    let start = match start_value {
        DataValue::Number(n) => n.as_i64().unwrap_or(0),
        _ => {
            // Try to convert to number
            if let Some(n) = start_value.coerce_to_number() {
                n.as_i64().unwrap_or(0)
            } else {
                0
            }
        }
    };
    
    // Get the length argument if provided
    let length = if args.len() > 2 {
        let length_value = evaluate(&args[2], data, arena)?;
        
        // Get the length
        match length_value {
            DataValue::Number(n) => n.as_i64().unwrap_or(i64::MAX),
            _ => {
                // Try to convert to number
                if let Some(n) = length_value.coerce_to_number() {
                    n.as_i64().unwrap_or(i64::MAX)
                } else {
                    i64::MAX
                }
            }
        }
    } else {
        // If no length provided, use the rest of the string
        i64::MAX
    };
    
    // Handle negative start index (count from end)
    let string_len = string.chars().count() as i64;
    let abs_start = if start < 0 { (-start).min(string_len) } else { start };
    
    let start_idx = if start < 0 {
        // Negative start means count from end
        string_len.saturating_sub(abs_start) as usize
    } else {
        // Positive start means count from beginning
        abs_start as usize
    };
    
    // If start is beyond the end of the string, return empty string
    if start_idx >= string_len as usize {
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(""))));
    }
    
    // Handle negative length (count from end)
    let end_idx = if length < 0 {
        // Negative length means count from end
        (string_len as i64 + length).max(start_idx as i64) as usize
    } else {
        // Positive length means count from start
        (start_idx + length as usize).min(string_len as usize)
    };
    
    // If end is before start, return empty string
    if end_idx <= start_idx {
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(""))));
    }
    
    // Extract the substring
    let mut char_iter = string.chars();
    let mut result = String::new();
    
    // Skip characters before start
    for _ in 0..start_idx {
        char_iter.next();
    }
    
    // Take characters until end
    for _ in start_idx..end_idx {
        if let Some(c) = char_iter.next() {
            result.push(c);
        } else {
            break;
        }
    }
    
    // Allocate the result string in the arena
    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use serde_json::json;
    use crate::value::FromJson;
    
    #[test]
    fn test_cat() {
        let arena = DataArena::new();
        let data_json = json!({"a": 10, "b": "hello", "c": true});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test concatenating strings
        let token = parse_str(r#"{"cat": ["hello", " ", "world"]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("hello world"));
        
        // Test concatenating different types
        let token = parse_str(r#"{"cat": [{"var": "b"}, " ", {"var": "a"}, " ", {"var": "c"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("hello 10 true"));
        
        // Test empty cat
        let token = parse_str(r#"{"cat": []}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some(""));
    }
    
    #[test]
    fn test_substr() {
        let arena = DataArena::new();
        let data_json = json!({"text": "hello world"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Test basic substring
        let token = parse_str(r#"{"substr": [{"var": "text"}, 0, 5]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("hello"));
        
        // Test negative start
        let token = parse_str(r#"{"substr": [{"var": "text"}, -5]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("world"));
        
        // Test negative length
        let token = parse_str(r#"{"substr": [{"var": "text"}, 0, -6]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("hello"));
        
        // Test out of bounds
        let token = parse_str(r#"{"substr": [{"var": "text"}, 20]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some(""));
    }
} 