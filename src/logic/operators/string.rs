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

/// Helper function to convert a value to a string representation
fn value_to_string<'a>(value: &'a DataValue<'a>, arena: &'a DataArena) -> &'a str {
    match value {
        DataValue::String(s) => s,
        _ => arena.alloc_str(&value.to_string()),
    }
}

/// Helper function to append values from an array to a string
fn append_array_to_string(values: &[DataValue<'_>], result: &mut String) {
    for value in values {
        match value {
            DataValue::String(s) => result.push_str(s),
            _ => result.push_str(&value.to_string()),
        }
    }
}

/// Validate arguments for substr operation
fn validate_substr_args(args: &[&Token]) -> Result<()> {
    if args.len() < 2 || args.len() > 3 {
        return Err(LogicError::InvalidArgumentsError);
    }
    Ok(())
}

/// Calculate the starting position for substring extraction
fn calculate_substr_start(start_idx: i64, char_count: usize) -> usize {
    if start_idx < 0 {
        let abs_idx = (-start_idx) as usize;
        char_count.saturating_sub(abs_idx)
    } else if start_idx as usize >= char_count {
        // If start is beyond the string length, we'll return empty string later
        char_count
    } else {
        start_idx as usize
    }
}

/// Calculate the length for substring extraction
fn calculate_substr_length(len_value: i64, char_count: usize, start_pos: usize) -> usize {
    if len_value < 0 {
        // Negative length means "leave this many characters off the end"
        let chars_to_remove = (-len_value) as usize;
        if chars_to_remove >= char_count || chars_to_remove > char_count - start_pos {
            0 // If we'd remove all characters or more than we have after start_pos, return empty
        } else {
            char_count - start_pos - chars_to_remove
        }
    } else {
        len_value as usize
    }
}

/// Evaluates a string concatenation operation.
pub fn eval_cat<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(arena.empty_string_value());
    }

    // For a single argument, convert directly to string
    if args.len() == 1 {
        let value = evaluate(args[0], arena)?;

        // If it's already a string, return it directly
        if let DataValue::String(_) = value {
            return Ok(value);
        }

        // If it's an array, concatenate all elements
        if let DataValue::Array(arr) = value {
            let mut result = String::new();
            append_array_to_string(arr, &mut result);
            return Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))));
        }

        // Otherwise, convert to string
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(&value.to_string()))));
    }

    // For multiple arguments, concatenate them
    let mut result = String::new();

    for arg in args {
        let value = evaluate(arg, arena)?;
        match value {
            DataValue::String(s) => result.push_str(s),
            DataValue::Array(arr) => {
                // If we get an array from a chained operation, concatenate all elements
                append_array_to_string(arr, &mut result);
            }
            _ => {
                result.push_str(&value.to_string());
            }
        }
    }

    // Allocate the result string in the arena
    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))))
}

/// Evaluates a substring operation.
pub fn eval_substr<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    validate_substr_args(args)?;

    let string = evaluate(args[0], arena)?;
    let string_str = value_to_string(string, arena);

    // Convert to char array for proper handling of multi-byte characters
    let chars: Vec<char> = string_str.chars().collect();
    let char_count = chars.len();

    let start = evaluate(args[1], arena)?;
    let start_idx_signed = start
        .coerce_to_number()
        .map(|num| num.as_i64().unwrap_or(0))
        .unwrap_or(0);

    // Handle negative start index (count from end)
    let start_pos = calculate_substr_start(start_idx_signed, char_count);

    // If start is beyond the string length, return empty string
    if start_pos >= char_count {
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(""))));
    }

    let length = if args.len() == 3 {
        let len = evaluate(args[2], arena)?;
        len.coerce_to_number()
            .map(|num| {
                let len_signed = num.as_i64().unwrap_or(0);
                calculate_substr_length(len_signed, char_count, start_pos)
            })
            .unwrap_or(0)
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
    use crate::logic::datalogic_core::DataLogicCore;
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
