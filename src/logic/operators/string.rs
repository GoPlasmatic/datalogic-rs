//! String operators for logic expressions.
//!
//! This module provides implementations for string operators
//! such as cat, substr, etc.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

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
            }
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
        char_count.saturating_sub(abs_idx)
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
                    if chars_to_remove >= char_count || chars_to_remove > char_count - start_pos {
                        0 // If we'd remove all characters or more than we have after start_pos, return empty
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
    use crate::DataLogicCore;
    use serde_json::json;

    #[test]
    fn test_cat() {
        // Create JSONLogic instance
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"a": 10, "b": "hello", "c": true});

        // Test concatenating strings
        // Use StringBuilder's concat method (note: it's called concat not cat in the builder)
        let rule = builder
            .string_ops()
            .concat_op()
            .string("hello")
            .string(" ")
            .string("world")
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello world"));

        // Test concatenating different types
        let rule = builder
            .string_ops()
            .concat_op()
            .var("b")
            .string(" ")
            .var("a")
            .string(" ")
            .var("c")
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello 10 true"));

        // Test empty cat
        let rule = builder.string_ops().concat_op().build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(""));
    }

    #[test]
    fn test_substr() {
        // Create JSONLogic instance
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({"text": "hello world"});

        // Test basic substring
        let rule = builder
            .string_ops()
            .substr_op()
            .var("text")
            .start_at(0)
            .take(5)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello"));

        // Test negative start
        let rule = builder
            .string_ops()
            .substr_op()
            .var("text")
            .start_at(-5)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("world"));

        // Test negative length
        let rule = builder
            .string_ops()
            .substr_op()
            .var("text")
            .start_at(0)
            .take(-6)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello"));

        // Test out of bounds
        let rule = builder
            .string_ops()
            .substr_op()
            .var("text")
            .start_at(20)
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(""));
    }
}
