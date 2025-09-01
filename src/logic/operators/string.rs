//! String operators for logic expressions.
//!
//! This module provides implementations for string operators
//! such as cat, substr, etc.

use crate::arena::DataArena;
use crate::context::EvalContext;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;
use regex::Regex;

/// Enumeration of string operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringOp {
    /// String concatenation
    Cat,
    /// Substring extraction
    Substr,
    /// String starts with
    StartsWith,
    /// String ends with
    EndsWith,
    /// Convert string to uppercase
    Upper,
    /// Convert string to lowercase
    Lower,
    /// Trim whitespace from beginning and end of string
    Trim,
    /// Replace occurrences of a string with another string
    Replace,
    /// Split string into array based on delimiter
    Split,
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
pub fn eval_cat<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(arena.empty_string_value());
    }

    // For a single argument, convert directly to string
    if args.len() == 1 {
        let value = evaluate(args[0], context, arena)?;

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
        let value = evaluate(arg, context, arena)?;
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
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    validate_substr_args(args)?;

    let string = evaluate(args[0], context, arena)?;
    let string_str = value_to_string(string, arena);

    // Convert to char array for proper handling of multi-byte characters
    let chars: Vec<char> = string_str.chars().collect();
    let char_count = chars.len();

    let start = evaluate(args[1], context, arena)?;
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
        let len = evaluate(args[2], context, arena)?;
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

/// Evaluates a "starts with" operation.
pub fn eval_starts_with<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let string = evaluate(args[0], context, arena)?;
    let prefix = evaluate(args[1], context, arena)?;

    let string_str = value_to_string(string, arena);
    let prefix_str = value_to_string(prefix, arena);

    Ok(arena.alloc(DataValue::Bool(string_str.starts_with(prefix_str))))
}

/// Evaluates an "ends with" operation.
pub fn eval_ends_with<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let string = evaluate(args[0], context, arena)?;
    let suffix = evaluate(args[1], context, arena)?;

    let string_str = value_to_string(string, arena);
    let suffix_str = value_to_string(suffix, arena);

    Ok(arena.alloc(DataValue::Bool(string_str.ends_with(suffix_str))))
}

/// Evaluates a string uppercase operation.
pub fn eval_upper<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    let string = evaluate(args[0], context, arena)?;
    let string_str = value_to_string(string, arena);

    // Convert to uppercase
    let result = string_str.to_uppercase();

    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))))
}

/// Evaluates a string lowercase operation.
pub fn eval_lower<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    let string = evaluate(args[0], context, arena)?;
    let string_str = value_to_string(string, arena);

    // Convert to lowercase
    let result = string_str.to_lowercase();

    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))))
}

/// Evaluates a string trim operation.
pub fn eval_trim<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    let string = evaluate(args[0], context, arena)?;
    let string_str = value_to_string(string, arena);

    // Trim whitespace
    let result = string_str.trim();

    Ok(arena.alloc(DataValue::String(arena.alloc_str(result))))
}

/// Evaluates a string replace operation.
pub fn eval_replace<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 3 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let string = evaluate(args[0], context, arena)?;
    let find = evaluate(args[1], context, arena)?;
    let replace_with = evaluate(args[2], context, arena)?;

    let string_str = value_to_string(string, arena);
    let find_str = value_to_string(find, arena);
    let replace_str = value_to_string(replace_with, arena);

    // Replace all occurrences
    let result = string_str.replace(find_str, replace_str);

    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))))
}

/// Evaluates a string split operation.
/// When the delimiter contains named groups (regex pattern), extracts those groups as an object.
/// Otherwise, performs normal string splitting.
pub fn eval_split<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let string = evaluate(args[0], context, arena)?;
    let delimiter = evaluate(args[1], context, arena)?;

    let string_str = value_to_string(string, arena);
    let delimiter_str = value_to_string(delimiter, arena);

    // Check if the delimiter looks like a regex pattern with named groups
    if delimiter_str.contains("(?P<") {
        // Try to compile as a regex and extract named groups
        match Regex::new(delimiter_str) {
            Ok(regex) => {
                // Check if there are any named groups
                let group_names: Vec<_> = regex.capture_names().flatten().collect();
                if !group_names.is_empty() {
                    // Try to match the regex and extract named groups
                    if let Some(captures) = regex.captures(string_str) {
                        let mut entries = Vec::new();

                        for name in group_names {
                            let group_value = captures.name(name).map(|m| m.as_str()).unwrap_or("");

                            let key = arena.alloc_str(name);
                            let value = DataValue::String(arena.alloc_str(group_value));
                            entries.push((key, value));
                        }

                        // Create object with extracted groups
                        let result_entries = arena.vec_into_slice(entries);
                        return Ok(arena.alloc(DataValue::Object(result_entries)));
                    } else {
                        // No match found, return empty object
                        let empty_entries: Vec<(&str, DataValue)> = vec![];
                        let result_entries = arena.vec_into_slice(empty_entries);
                        return Ok(arena.alloc(DataValue::Object(result_entries)));
                    }
                }
            }
            Err(_) => {
                // If regex compilation fails, fall through to normal split behavior
            }
        }
    }

    // Normal split behavior (original implementation)
    let parts: Vec<DataValue> = string_str
        .split(delimiter_str)
        .map(|part| DataValue::String(arena.alloc_str(part)))
        .collect();

    // Create array of string parts using vec_into_slice
    let result_array = arena.vec_into_slice(parts);
    Ok(arena.alloc(DataValue::Array(result_array)))
}

#[cfg(test)]
mod tests {
    use crate::logic::Logic;
    use crate::logic::datalogic_core::DataLogicCore;
    use crate::logic::token::{OperatorType, Token};
    use crate::value::DataValue;
    use serde_json::json;

    #[test]
    fn test_cat() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"a": 10, "b": "hello", "c": true});

        // Test concatenating strings: {"cat": ["hello", " ", "world"]}
        let hello = Token::literal(DataValue::string(arena, "hello"));
        let hello_ref = arena.alloc(hello);

        let space = Token::literal(DataValue::string(arena, " "));
        let space_ref = arena.alloc(space);

        let world = Token::literal(DataValue::string(arena, "world"));
        let world_ref = arena.alloc(world);

        let args = vec![hello_ref, space_ref, world_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let cat_token = Token::operator(OperatorType::String(super::StringOp::Cat), array_ref);
        let cat_ref = arena.alloc(cat_token);

        // Create Logic for result
        let rule = Logic::new(cat_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello world"));

        // Test concatenating different types: {"cat": [{"var": "b"}, " ", {"var": "a"}, " ", {"var": "c"}]}
        let var_b = Token::variable("b", None);
        let var_b_ref = arena.alloc(var_b);

        let var_a = Token::variable("a", None);
        let var_a_ref = arena.alloc(var_a);

        let var_c = Token::variable("c", None);
        let var_c_ref = arena.alloc(var_c);

        let args = vec![var_b_ref, space_ref, var_a_ref, space_ref, var_c_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let cat_token = Token::operator(OperatorType::String(super::StringOp::Cat), array_ref);
        let cat_ref = arena.alloc(cat_token);

        let rule = Logic::new(cat_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello 10 true"));

        // Test empty cat: {"cat": []}
        let empty_args: Vec<&Token> = vec![];
        let empty_array_token = Token::ArrayLiteral(empty_args);
        let empty_array_ref = arena.alloc(empty_array_token);

        let empty_cat_token =
            Token::operator(OperatorType::String(super::StringOp::Cat), empty_array_ref);
        let empty_cat_ref = arena.alloc(empty_cat_token);

        let rule = Logic::new(empty_cat_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(""));
    }

    #[test]
    fn test_substr() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "hello world"});

        // Test basic substring: {"substr": [{"var": "text"}, 0, 5]}
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let start_token = Token::literal(DataValue::integer(0));
        let start_ref = arena.alloc(start_token);

        let length_token = Token::literal(DataValue::integer(5));
        let length_ref = arena.alloc(length_token);

        let args = vec![var_ref, start_ref, length_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let substr_token =
            Token::operator(OperatorType::String(super::StringOp::Substr), array_ref);
        let substr_ref = arena.alloc(substr_token);

        let rule = Logic::new(substr_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello"));

        // Test negative start: {"substr": [{"var": "text"}, -5]}
        let neg_start_token = Token::literal(DataValue::integer(-5));
        let neg_start_ref = arena.alloc(neg_start_token);

        let args = vec![var_ref, neg_start_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let substr_token =
            Token::operator(OperatorType::String(super::StringOp::Substr), array_ref);
        let substr_ref = arena.alloc(substr_token);

        let rule = Logic::new(substr_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("world"));

        // Test negative length: {"substr": [{"var": "text"}, 0, -6]}
        let neg_length_token = Token::literal(DataValue::integer(-6));
        let neg_length_ref = arena.alloc(neg_length_token);

        let args = vec![var_ref, start_ref, neg_length_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let substr_token =
            Token::operator(OperatorType::String(super::StringOp::Substr), array_ref);
        let substr_ref = arena.alloc(substr_token);

        let rule = Logic::new(substr_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello"));

        // Test out of bounds: {"substr": [{"var": "text"}, 20]}
        let out_of_bounds_token = Token::literal(DataValue::integer(20));
        let out_of_bounds_ref = arena.alloc(out_of_bounds_token);

        let args = vec![var_ref, out_of_bounds_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let substr_token =
            Token::operator(OperatorType::String(super::StringOp::Substr), array_ref);
        let substr_ref = arena.alloc(substr_token);

        let rule = Logic::new(substr_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(""));
    }

    #[test]
    fn test_starts_with() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "hello world"});

        // Test positive case: {"starts_with": [{"var": "text"}, "hello"]}
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let prefix_token = Token::literal(DataValue::string(arena, "hello"));
        let prefix_ref = arena.alloc(prefix_token);

        let args = vec![var_ref, prefix_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let starts_with_token =
            Token::operator(OperatorType::String(super::StringOp::StartsWith), array_ref);
        let starts_with_ref = arena.alloc(starts_with_token);

        let rule = Logic::new(starts_with_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test negative case: {"starts_with": [{"var": "text"}, "world"]}
        let world_token = Token::literal(DataValue::string(arena, "world"));
        let world_ref = arena.alloc(world_token);

        let args = vec![var_ref, world_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let starts_with_token =
            Token::operator(OperatorType::String(super::StringOp::StartsWith), array_ref);
        let starts_with_ref = arena.alloc(starts_with_token);

        let rule = Logic::new(starts_with_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));

        // Test case sensitivity: {"starts_with": [{"var": "text"}, "HELLO"]}
        let upper_hello_token = Token::literal(DataValue::string(arena, "HELLO"));
        let upper_hello_ref = arena.alloc(upper_hello_token);

        let args = vec![var_ref, upper_hello_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let starts_with_token =
            Token::operator(OperatorType::String(super::StringOp::StartsWith), array_ref);
        let starts_with_ref = arena.alloc(starts_with_token);

        let rule = Logic::new(starts_with_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_ends_with() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "hello world"});

        // Test positive case: {"ends_with": [{"var": "text"}, "world"]}
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let suffix_token = Token::literal(DataValue::string(arena, "world"));
        let suffix_ref = arena.alloc(suffix_token);

        let args = vec![var_ref, suffix_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let ends_with_token =
            Token::operator(OperatorType::String(super::StringOp::EndsWith), array_ref);
        let ends_with_ref = arena.alloc(ends_with_token);

        let rule = Logic::new(ends_with_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(true));

        // Test negative case: {"ends_with": [{"var": "text"}, "hello"]}
        let hello_token = Token::literal(DataValue::string(arena, "hello"));
        let hello_ref = arena.alloc(hello_token);

        let args = vec![var_ref, hello_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let ends_with_token =
            Token::operator(OperatorType::String(super::StringOp::EndsWith), array_ref);
        let ends_with_ref = arena.alloc(ends_with_token);

        let rule = Logic::new(ends_with_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));

        // Test case sensitivity: {"ends_with": [{"var": "text"}, "WORLD"]}
        let upper_world_token = Token::literal(DataValue::string(arena, "WORLD"));
        let upper_world_ref = arena.alloc(upper_world_token);

        let args = vec![var_ref, upper_world_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let ends_with_token =
            Token::operator(OperatorType::String(super::StringOp::EndsWith), array_ref);
        let ends_with_ref = arena.alloc(ends_with_token);

        let rule = Logic::new(ends_with_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_upper() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "Hello World"});

        // Test uppercase: {"upper": [{"var": "text"}]}
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let args = vec![var_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let upper_token = Token::operator(OperatorType::String(super::StringOp::Upper), array_ref);
        let upper_ref = arena.alloc(upper_token);

        let rule = Logic::new(upper_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("HELLO WORLD"));
    }

    #[test]
    fn test_lower() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "Hello World"});

        // Test lowercase: {"lower": [{"var": "text"}]}
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let args = vec![var_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let lower_token = Token::operator(OperatorType::String(super::StringOp::Lower), array_ref);
        let lower_ref = arena.alloc(lower_token);

        let rule = Logic::new(lower_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello world"));
    }

    #[test]
    fn test_trim() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "  Hello World  "});

        // Test trim: {"trim": [{"var": "text"}]}
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let args = vec![var_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let trim_token = Token::operator(OperatorType::String(super::StringOp::Trim), array_ref);
        let trim_ref = arena.alloc(trim_token);

        let rule = Logic::new(trim_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("Hello World"));
    }

    #[test]
    fn test_replace() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "hello world hello"});

        // Test basic replace: {"replace": [{"var": "text"}, "hello", "hi"]}
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let find_token = Token::literal(DataValue::string(arena, "hello"));
        let find_ref = arena.alloc(find_token);

        let replace_token = Token::literal(DataValue::string(arena, "hi"));
        let replace_ref = arena.alloc(replace_token);

        let args = vec![var_ref, find_ref, replace_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let replace_op_token =
            Token::operator(OperatorType::String(super::StringOp::Replace), array_ref);
        let replace_op_ref = arena.alloc(replace_op_token);

        let rule = Logic::new(replace_op_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hi world hi"));

        // Test case sensitivity: {"replace": [{"var": "text"}, "HELLO", "hi"]}
        let find_upper_token = Token::literal(DataValue::string(arena, "HELLO"));
        let find_upper_ref = arena.alloc(find_upper_token);

        let args = vec![var_ref, find_upper_ref, replace_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let replace_op_token =
            Token::operator(OperatorType::String(super::StringOp::Replace), array_ref);
        let replace_op_ref = arena.alloc(replace_op_token);

        let rule = Logic::new(replace_op_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello world hello")); // No replacement should occur

        // Test replace with empty string: {"replace": [{"var": "text"}, "hello", ""]}
        let empty_token = Token::literal(DataValue::string(arena, ""));
        let empty_ref = arena.alloc(empty_token);

        let args = vec![var_ref, find_ref, empty_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let replace_op_token =
            Token::operator(OperatorType::String(super::StringOp::Replace), array_ref);
        let replace_op_ref = arena.alloc(replace_op_token);

        let rule = Logic::new(replace_op_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(" world "));

        // Test replace non-existent: {"replace": [{"var": "text"}, "xyz", "abc"]}
        let nonexistent_token = Token::literal(DataValue::string(arena, "xyz"));
        let nonexistent_ref = arena.alloc(nonexistent_token);

        let replacement_token = Token::literal(DataValue::string(arena, "abc"));
        let replacement_ref = arena.alloc(replacement_token);

        let args = vec![var_ref, nonexistent_ref, replacement_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let replace_op_token =
            Token::operator(OperatorType::String(super::StringOp::Replace), array_ref);
        let replace_op_ref = arena.alloc(replace_op_token);

        let rule = Logic::new(replace_op_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!("hello world hello")); // No change
    }

    #[test]
    fn test_split() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "apple,banana,cherry"});

        // Test basic split: {"split": [{"var": "text"}, ","]}
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let delimiter_token = Token::literal(DataValue::string(arena, ","));
        let delimiter_ref = arena.alloc(delimiter_token);

        let args = vec![var_ref, delimiter_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(["apple", "banana", "cherry"]));

        // Test split by space: {"split": [{"var": "sentence"}, " "]}
        let sentence_data = json!({"sentence": "hello world test"});

        let sentence_var_token = Token::variable("sentence", None);
        let sentence_var_ref = arena.alloc(sentence_var_token);

        let space_token = Token::literal(DataValue::string(arena, " "));
        let space_ref = arena.alloc(space_token);

        let args = vec![sentence_var_ref, space_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &sentence_data).unwrap();
        assert_eq!(result, json!(["hello", "world", "test"]));

        // Test split with non-existent delimiter: {"split": [{"var": "text"}, ";"]}
        let semicolon_token = Token::literal(DataValue::string(arena, ";"));
        let semicolon_ref = arena.alloc(semicolon_token);

        let args = vec![var_ref, semicolon_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(["apple,banana,cherry"])); // Original string as single element

        // Test split empty string: {"split": ["", ","]}
        let empty_string_token = Token::literal(DataValue::string(arena, ""));
        let empty_string_ref = arena.alloc(empty_string_token);

        let args = vec![empty_string_ref, delimiter_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!([""])); // Empty string results in array with one empty string

        // Test split with empty delimiter: {"split": [{"var": "text"}, ""]}
        let empty_delim_token = Token::literal(DataValue::string(arena, ""));
        let empty_delim_ref = arena.alloc(empty_delim_token);

        let args = vec![var_ref, empty_delim_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        // Splitting by empty string should split into individual characters
        // Note: Rust's split() with empty string includes empty strings at start and end
        assert_eq!(
            result,
            json!([
                "", "a", "p", "p", "l", "e", ",", "b", "a", "n", "a", "n", "a", ",", "c", "h", "e",
                "r", "r", "y", ""
            ])
        );
    }

    #[test]
    fn test_split_with_regex_extraction() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Test IBAN regex extraction: {"split": ["SBININBB101", "^(?P<bank>[A-Z]{4})(?P<country>[A-Z]{2})(?P<location>[A-Z0-9]{2})(?P<branch>[A-Z0-9]{3})?$"]}
        let data_json = json!({"iban": "SBININBB101"});

        let var_token = Token::variable("iban", None);
        let var_ref = arena.alloc(var_token);

        let regex_pattern = "^(?P<bank>[A-Z]{4})(?P<country>[A-Z]{2})(?P<location>[A-Z0-9]{2})(?P<branch>[A-Z0-9]{3})?$";
        let regex_token = Token::literal(DataValue::string(arena, regex_pattern));
        let regex_ref = arena.alloc(regex_token);

        let args = vec![var_ref, regex_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();

        // Should return an object with extracted groups
        let expected = json!({
            "bank": "SBIN",
            "country": "IN",
            "location": "BB",
            "branch": "101"
        });
        assert_eq!(result, expected);

        // Test with non-matching pattern
        let non_match_data = json!({"iban": "invalid"});
        let result = core.apply(&rule, &non_match_data).unwrap();

        // Should return empty object for non-matching patterns
        assert_eq!(result, json!({}));

        // Test with literal string: {"split": ["SBININBB101", "^(?P<bank>[A-Z]{4})(?P<country>[A-Z]{2})(?P<location>[A-Z0-9]{2})(?P<branch>[A-Z0-9]{3})?$"]}
        let literal_string_token = Token::literal(DataValue::string(arena, "SBININBB101"));
        let literal_string_ref = arena.alloc(literal_string_token);

        let args = vec![literal_string_ref, regex_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, expected);

        // Test partial match - country code regex: {"split": ["SBININBB101", "(?P<country>[A-Z]{2})"]}
        let country_regex_token = Token::literal(DataValue::string(arena, "(?P<country>[A-Z]{2})"));
        let country_regex_ref = arena.alloc(country_regex_token);

        let args = vec![literal_string_ref, country_regex_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &json!({})).unwrap();

        // Should extract first match (SB from SBIN)
        let expected_partial = json!({
            "country": "SB"
        });
        assert_eq!(result, expected_partial);
    }

    #[test]
    fn test_split_regex_fallback_to_normal_split() {
        // Create DataLogicCore instance
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({"text": "apple,banana,cherry"});

        // Test with invalid regex that contains (?P< but is malformed
        let var_token = Token::variable("text", None);
        let var_ref = arena.alloc(var_token);

        let invalid_regex_token = Token::literal(DataValue::string(arena, "(?P<invalid"));
        let invalid_regex_ref = arena.alloc(invalid_regex_token);

        let args = vec![var_ref, invalid_regex_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let split_token = Token::operator(OperatorType::String(super::StringOp::Split), array_ref);
        let split_ref = arena.alloc(split_token);

        let rule = Logic::new(split_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();

        // Should fall back to normal split behavior
        assert_eq!(result, json!(["apple,banana,cherry"])); // No split occurs with this "delimiter"
    }
}
