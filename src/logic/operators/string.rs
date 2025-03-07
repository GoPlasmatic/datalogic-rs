//! String operators for logic expressions.
//!
//! This module provides implementations for string operators
//! such as cat, substr, etc.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::value::{NumberValue};

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
) -> Result<DataValue<'a>> {
    // If no arguments, return empty string
    if args.is_empty() {
        return Ok(DataValue::String(arena.alloc_str("")));
    }

    // First pass: calculate total length to avoid reallocations
    let mut total_length = 0;
    
    // Calculate the total length without creating temporary strings
    for arg in args {
        let value = evaluate(arg, data, arena)?;
        match value {
            DataValue::String(s) => total_length += s.len(),
            DataValue::Number(_) => total_length += 20, // Conservative estimate for numbers
            DataValue::Bool(b) => total_length += if b { 4 } else { 5 }, // "true" or "false"
            DataValue::Null => total_length += 4, // "null"
            DataValue::Array(arr) => {
                // Rough estimate for array: 2 chars for brackets, 2 chars per element for separator
                total_length += 2 + arr.len() * 2;
                for item in arr {
                    match item {
                        DataValue::String(s) => total_length += s.len() + 2, // +2 for quotes
                        DataValue::Number(_) => total_length += 10, // Rough estimate
                        DataValue::Bool(b) => total_length += if *b { 4 } else { 5 },
                        DataValue::Null => total_length += 4,
                        _ => total_length += 15, // Rough estimate for complex values
                    }
                }
            },
            DataValue::Object(_) => total_length += 8, // "[object]"
        }
    }
    
    // Second pass: build the string directly with pre-allocated capacity
    let mut result = String::with_capacity(total_length);
    
    // Append each value directly to the result string
    for arg in args {
        let value = evaluate(arg, data, arena)?;
        match value {
            DataValue::String(s) => result.push_str(s),
            DataValue::Number(n) => {
                match n {
                    NumberValue::Integer(i) => {
                        use std::fmt::Write;
                        write!(result, "{}", i).unwrap();
                    },
                    NumberValue::Float(f) => {
                        use std::fmt::Write;
                        write!(result, "{}", f).unwrap();
                    }
                }
            },
            DataValue::Bool(b) => result.push_str(if b { "true" } else { "false" }),
            DataValue::Null => result.push_str("null"),
            DataValue::Array(arr) => {
                result.push('[');
                let mut first = true;
                for item in arr {
                    if !first {
                        result.push_str(", ");
                    }
                    match item {
                        DataValue::String(s) => {
                            result.push('"');
                            result.push_str(s);
                            result.push('"');
                        },
                        DataValue::Number(n) => {
                            match n {
                                NumberValue::Integer(i) => {
                                    use std::fmt::Write;
                                    write!(result, "{}", i).unwrap();
                                },
                                NumberValue::Float(f) => {
                                    use std::fmt::Write;
                                    write!(result, "{}", f).unwrap();
                                }
                            }
                        },
                        DataValue::Bool(b) => result.push_str(if *b { "true" } else { "false" }),
                        DataValue::Null => result.push_str("null"),
                        _ => result.push_str("[complex value]"),
                    }
                    first = false;
                }
                result.push(']');
            },
            DataValue::Object(_) => result.push_str("[object]"),
        }
    }
    
    // Return the concatenated string
    Ok(DataValue::String(arena.alloc_str(&result)))
}

/// Evaluates a substr operation.
/// Gets a portion of a string based on start position and length.
pub fn eval_substr<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Check that we have at least 1 argument (the string)
    if args.is_empty() {
        return Err(LogicError::OperatorError {
            operator: "substr".to_string(),
            reason: "Expected at least 1 argument".to_string(),
        });
    }

    // Fast path for empty string result
    let empty_str = || DataValue::String(arena.alloc_str(""));

    // Evaluate the string argument
    let string_value = evaluate(&args[0], data, arena)?;
    let string = match string_value {
        DataValue::String(s) => s,
        DataValue::Number(n) => {
            // Avoid allocating a new string if possible by checking for common cases
            let num_str = arena.alloc_str(&n.to_string());
            return if args.len() <= 2 {
                // If only start is provided, use the whole string
                match args.get(1) {
                    Some(token) => {
                        let start_value = evaluate(token, data, arena)?;
                        if let DataValue::Number(n) = start_value {
                            let start = n.as_i64().unwrap_or(0) as isize;
                            if start <= 0 || start as usize >= num_str.len() {
                                if start < 0 && (-start as usize) < num_str.len() {
                                    // Negative index from end
                                    let start_idx = num_str.len() - (-start as usize);
                                    Ok(DataValue::String(arena.alloc_str(&num_str[start_idx..])))
                                } else {
                                    // Start is beyond the string bounds
                                    Ok(empty_str())
                                }
                            } else {
                                // Normal substring from start to end
                                Ok(DataValue::String(arena.alloc_str(&num_str[start as usize..])))
                            }
                        } else {
                            Err(LogicError::OperatorError {
                                operator: "substr".to_string(),
                                reason: format!("Expected number for start position, got {:?}", start_value),
                            })
                        }
                    }
                    None => Ok(DataValue::String(num_str)),
                }
            } else {
                // Handle start and length case
                DataValue::String(num_str).substr_with_args(&args[1..], data, arena, empty_str)
            };
        }
        DataValue::Bool(b) => {
            let bool_str = if b { "true" } else { "false" };
            return Ok(DataValue::String(arena.alloc_str(bool_str)));
        }
        DataValue::Null => return Ok(DataValue::String(arena.alloc_str("null"))),
        _ => return Err(LogicError::OperatorError {
            operator: "substr".to_string(),
            reason: format!("Expected string, got {:?}", string_value),
        }),
    };

    // If string is empty, return empty string
    if string.is_empty() {
        return Ok(empty_str());
    }

    // If only start argument provided
    if args.len() <= 2 {
        return if let Some(start_token) = args.get(1) {
            let start_value = evaluate(start_token, data, arena)?;
            if let DataValue::Number(n) = start_value {
                let start = n.as_i64().unwrap_or(0) as isize;
                
                // Fast path for ASCII-only strings
                if string.is_ascii() {
                    let string_len = string.len();
                    let start_idx = if start < 0 {
                        // Negative start means count from the end
                        let abs_start = (-start) as usize;
                        string_len.saturating_sub(abs_start)
                    } else {
                        // Positive start
                        let start_usize = start as usize;
                        if start_usize >= string_len {
                            return Ok(empty_str()); // Start beyond end returns empty string
                        }
                        start_usize
                    };
                    
                    Ok(DataValue::String(arena.alloc_str(&string[start_idx..])))
                } else {
                    // Non-ASCII path (Unicode) - need char indices
                    let string_len = string.chars().count();
                    let start_idx = if start < 0 {
                        // Negative start means count from the end
                        let abs_start = (-start) as usize;
                        string_len.saturating_sub(abs_start)
                    } else {
                        // Positive start
                        let start_usize = start as usize;
                        if start_usize >= string_len {
                            return Ok(empty_str()); // Start beyond end returns empty string
                        }
                        start_usize
                    };
                    
                    // Convert from char index to byte index
                    let byte_start = string.char_indices()
                        .nth(start_idx)
                        .map(|(i, _)| i)
                        .unwrap_or(string.len());
                    
                    Ok(DataValue::String(arena.alloc_str(&string[byte_start..])))
                }
            } else {
                Err(LogicError::OperatorError {
                    operator: "substr".to_string(),
                    reason: format!("Expected number for start position, got {:?}", start_value),
                })
            }
        } else {
            // No start position provided, return entire string
            Ok(DataValue::String(string))
        };
    }

    // If we have both start and length, handle it with the helper
    DataValue::String(string).substr_with_args(&args[1..], data, arena, empty_str)
}

// Extension trait for DataValue to handle substr operations
trait SubstrOps<'a> {
    fn substr_with_args(
        &self,
        args: &[Token<'a>],
        data: &'a DataValue<'a>,
        arena: &'a DataArena,
        empty_str: impl Fn() -> DataValue<'a>
    ) -> Result<DataValue<'a>>;
}

impl<'a> SubstrOps<'a> for DataValue<'a> {
    fn substr_with_args(
        &self,
        args: &[Token<'a>],
        data: &'a DataValue<'a>,
        arena: &'a DataArena,
        empty_str: impl Fn() -> DataValue<'a>
    ) -> Result<DataValue<'a>> {
        let string = self.as_str().unwrap();
        if string.is_empty() {
            return Ok(empty_str());
        }
        
        // Get start position
        let start_value = evaluate(&args[0], data, arena)?;
        let start = match start_value {
            DataValue::Number(n) => n.as_i64().unwrap_or(0) as isize,
            _ => return Err(LogicError::OperatorError {
                operator: "substr".to_string(),
                reason: format!("Expected number for start position, got {:?}", start_value),
            }),
        };
        
        // Fast path for ASCII strings
        if string.is_ascii() {
            let string_len = string.len();
            let start_idx = if start < 0 {
                let abs_start = (-start) as usize;
                string_len.saturating_sub(abs_start)
            } else {
                let start_usize = start as usize;
                if start_usize >= string_len { return Ok(empty_str()); }
                start_usize
            };
            
            // If no length provided
            if args.len() <= 1 {
                return Ok(DataValue::String(arena.alloc_str(&string[start_idx..])));
            }
            
            // Get length
            let length_value = evaluate(&args[1], data, arena)?;
            let length = match length_value {
                DataValue::Number(n) => n.as_i64().unwrap_or(0) as isize,
                _ => return Err(LogicError::OperatorError {
                    operator: "substr".to_string(),
                    reason: format!("Expected number for length, got {:?}", length_value),
                }),
            };
            
            let end_idx = if length < 0 {
                let abs_length = (-length) as usize;
                if start_idx + abs_length >= string_len {
                    start_idx // Negative length would go beyond start
                } else {
                    string_len - abs_length
                }
            } else {
                let end = start_idx + length as usize;
                if end > string_len { string_len } else { end }
            };
            
            if end_idx <= start_idx {
                return Ok(empty_str());
            }
            
            Ok(DataValue::String(arena.alloc_str(&string[start_idx..end_idx])))
        } else {
            // Non-ASCII path (Unicode characters)
            let string_len = string.chars().count();
            let start_idx = if start < 0 {
                let abs_start = (-start) as usize;
                string_len.saturating_sub(abs_start)
            } else {
                let start_usize = start as usize;
                if start_usize >= string_len { return Ok(empty_str()); }
                start_usize
            };
            
            // If no length provided
            if args.len() <= 1 {
                let byte_start = string.char_indices()
                    .nth(start_idx)
                    .map(|(i, _)| i)
                    .unwrap_or(string.len());
                
                return Ok(DataValue::String(arena.alloc_str(&string[byte_start..])));
            }
            
            // Get length
            let length_value = evaluate(&args[1], data, arena)?;
            let length = match length_value {
                DataValue::Number(n) => n.as_i64().unwrap_or(0) as isize,
                _ => return Err(LogicError::OperatorError {
                    operator: "substr".to_string(),
                    reason: format!("Expected number for length, got {:?}", length_value),
                }),
            };
            
            let end_idx = if length < 0 {
                let abs_length = (-length) as usize;
                if start_idx + abs_length >= string_len {
                    start_idx // Negative length would go beyond start
                } else {
                    string_len - abs_length
                }
            } else {
                let end = start_idx + length as usize;
                if end > string_len { string_len } else { end }
            };
            
            if end_idx <= start_idx {
                return Ok(empty_str());
            }
            
            // Convert char indices to byte indices efficiently
            let mut char_indices = string.char_indices();
            let byte_start = char_indices.nth(start_idx).map(|(i, _)| i).unwrap_or(0);
            
            // Skip ahead to end_idx - start_idx
            let chars_to_skip = end_idx - start_idx - 1;
            let byte_end = if chars_to_skip > 0 {
                char_indices.nth(chars_to_skip).map(|(i, _)| i).unwrap_or(string.len())
            } else {
                string.len()
            };
            
            Ok(DataValue::String(arena.alloc_str(&string[byte_start..byte_end])))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::DataValue;
    use crate::arena::DataArena;
    use crate::logic::token::Token;

    #[test]
    fn test_cat() {
        let arena = DataArena::new();
        
        // Test with string literals
        let args = &[
            Token::literal(DataValue::String(arena.alloc_str("Hello"))),
            Token::literal(DataValue::String(arena.alloc_str(", "))),
            Token::literal(DataValue::String(arena.alloc_str("World"))),
        ];
        
        let result = eval_cat(args, &DataValue::Null, &arena).unwrap();
        assert_eq!(result.as_str().unwrap(), "Hello, World");
        
        // Test with mixed types
        let args = &[
            Token::literal(DataValue::String(arena.alloc_str("Count: "))),
            Token::literal(DataValue::integer(42)),
            Token::literal(DataValue::String(arena.alloc_str(", Boolean: "))),
            Token::literal(DataValue::Bool(true)),
        ];
        
        let result = eval_cat(args, &DataValue::Null, &arena).unwrap();
        assert_eq!(result.as_str().unwrap(), "Count: 42, Boolean: true");
        
        // Test with empty args
        let args = &[];
        let result = eval_cat(args, &DataValue::Null, &arena).unwrap();
        assert_eq!(result.as_str().unwrap(), "");
    }
    
    #[test]
    fn test_substr() {
        let arena = DataArena::new();
        let test_string = "jsonlogic";
        
        // Test with positive start, no length
        let args = &[
            Token::literal(DataValue::String(arena.alloc_str(test_string))),
            Token::literal(DataValue::integer(4)),
        ];
        
        let result = eval_substr(args, &DataValue::Null, &arena).unwrap();
        assert_eq!(result.as_str().unwrap(), "logic");
        
        // Test with negative start, no length
        let args = &[
            Token::literal(DataValue::String(arena.alloc_str(test_string))),
            Token::literal(DataValue::integer(-5)),
        ];
        
        let result = eval_substr(args, &DataValue::Null, &arena).unwrap();
        assert_eq!(result.as_str().unwrap(), "logic");
        
        // Test with positive start and positive length
        let args = &[
            Token::literal(DataValue::String(arena.alloc_str(test_string))),
            Token::literal(DataValue::integer(1)),
            Token::literal(DataValue::integer(3)),
        ];
        
        let result = eval_substr(args, &DataValue::Null, &arena).unwrap();
        assert_eq!(result.as_str().unwrap(), "son");
        
        // Test with positive start and negative length
        let args = &[
            Token::literal(DataValue::String(arena.alloc_str(test_string))),
            Token::literal(DataValue::integer(4)),
            Token::literal(DataValue::integer(-2)),
        ];
        
        let result = eval_substr(args, &DataValue::Null, &arena).unwrap();
        assert_eq!(result.as_str().unwrap(), "log");
    }
} 