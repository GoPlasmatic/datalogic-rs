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

/// Evaluates a string concatenation operation.
pub fn eval_cat<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(arena.empty_string_value());
    }

    // For a single argument, convert directly to string
    if args.len() == 1 {
        let value = evaluate(args[0], data, arena)?;
        
        // If it's already a string, return it directly
        if let DataValue::String(_) = value {
            return Ok(value);
        }
        
        // If it's an array, concatenate all elements
        if let DataValue::Array(arr) = value {
            let mut result = String::new();
            for item in *arr {
                match item {
                    DataValue::String(s) => result.push_str(s),
                    _ => {
                        let string_value = item.to_string();
                        result.push_str(&string_value);
                    }
                }
            }
            return Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))));
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
            DataValue::Array(arr) => {
                // If we get an array from a chained operation, concatenate all elements
                for item in *arr {
                    match item {
                        DataValue::String(s) => result.push_str(s),
                        _ => {
                            let string_value = item.to_string();
                            result.push_str(&string_value);
                        }
                    }
                }
            },
            _ => {
                let string_value = value.to_string();
                result.push_str(&string_value);
            }
        }
    }
    
    // Allocate the result string in the arena
    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))))
}

/// Evaluates a substring operation.
pub fn eval_substr<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 || args.len() > 3 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let string = evaluate(args[0], data, arena)?;
    let string_str = match string {
        DataValue::String(s) => *s,
        _ => arena.alloc_str(&string.to_string()),
    };

    // Convert to char array for proper handling of multi-byte characters
    let chars: Vec<char> = string_str.chars().collect();
    let char_count = chars.len();

    let start = evaluate(args[1], data, arena)?;
    let start_idx_signed = match start.coerce_to_number() {
        Some(num) => num.as_i64().unwrap_or(0),
        None => 0,
    };

    // Handle negative start index (count from end)
    let start_pos = if start_idx_signed < 0 {
        let abs_idx = (-start_idx_signed) as usize;
        if abs_idx >= char_count {
            0 // If negative index is too large, start from beginning
        } else {
            char_count - abs_idx
        }
    } else if start_idx_signed as usize >= char_count {
        // If start is beyond the string length, return empty string
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(""))));
    } else {
        start_idx_signed as usize
    };

    let length = if args.len() == 3 {
        let len = evaluate(args[2], data, arena)?;
        match len.coerce_to_number() {
            Some(num) => {
                let len_signed = num.as_i64().unwrap_or(0);
                if len_signed < 0 {
                    // Negative length means "leave this many characters off the end"
                    let chars_to_remove = (-len_signed) as usize;
                    if chars_to_remove >= char_count {
                        0 // If we'd remove all characters, return empty string
                    } else if chars_to_remove > char_count - start_pos {
                        0 // If we'd remove more than we have after start_pos, return empty
                    } else {
                        char_count - start_pos - chars_to_remove
                    }
                } else {
                    len_signed as usize
                }
            }
            None => 0,
        }
    } else {
        // If no length provided, use the rest of the string
        char_count - start_pos
    };

    // Extract the substring (note: using chars to handle multi-byte characters)
    let result: String = chars.iter().skip(start_pos).take(length).collect();
    
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