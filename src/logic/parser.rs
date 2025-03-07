//! Parser for logic expressions.
//!
//! This module provides functions for parsing logic expressions from JSON.

use serde_json::{Value as JsonValue, Map as JsonMap};
use crate::arena::DataArena;
use crate::value::{DataValue, FromJson};
use super::token::{Token, OperatorType};
use super::error::{LogicError, Result};

/// Checks if a JSON value is a literal.
fn is_json_literal(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => true,
        JsonValue::Array(arr) => {
            // Nested arrays are allowed if they only contain literals
            arr.iter().all(is_json_literal)
        },
        JsonValue::Object(_) => false,
    }
}

/// Parses a logic expression from a JSON value.
pub fn parse_json<'a>(json: &JsonValue, arena: &'a DataArena) -> Result<&'a Token<'a>> {
    let token = parse_json_internal(json, arena)?;
    Ok(arena.alloc(token))
}

/// Parses a logic expression from a JSON string.
pub fn parse_str<'a>(json_str: &str, arena: &'a DataArena) -> Result<&'a Token<'a>> {
    let json: JsonValue = serde_json::from_str(json_str)
        .map_err(|e| LogicError::ParseError {
            reason: format!("Invalid JSON: {}", e),
        })?;
    parse_json(&json, arena)
}

/// Internal function for parsing a JSON value into a token.
fn parse_json_internal<'a>(json: &JsonValue, arena: &'a DataArena) -> Result<Token<'a>> {
    match json {
        // Simple literals
        JsonValue::Null => Ok(Token::literal(DataValue::null())),
        JsonValue::Bool(b) => Ok(Token::literal(DataValue::bool(*b))),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Token::literal(DataValue::integer(i)))
            } else if let Some(f) = n.as_f64() {
                Ok(Token::literal(DataValue::float(f)))
            } else {
                Err(LogicError::ParseError {
                    reason: format!("Invalid number: {}", n),
                })
            }
        },
        JsonValue::String(s) => Ok(Token::literal(DataValue::string(arena, s))),
        
        // Arrays could be literal arrays or arrays of tokens
        JsonValue::Array(arr) => {
            // Check if all elements are literals
            let mut all_literals = true;
            for item in arr {
                if !is_json_literal(item) {
                    all_literals = false;
                    break;
                }
            }
            
            // If all elements are literals, create a literal array
            if all_literals {
                let mut values = Vec::with_capacity(arr.len());
                for item in arr {
                    let value = DataValue::from_json(item, arena);
                    values.push(value);
                }
                let values_slice = arena.alloc_slice_clone(&values);
                Ok(Token::literal(DataValue::Array(values_slice)))
            } else {
                // Otherwise, create an array operator
                let mut tokens = Vec::with_capacity(arr.len());
                for item in arr {
                    let token = parse_json_internal(item, arena)?;
                    tokens.push(token);
                }
                let tokens_slice = arena.alloc_slice_clone(&tokens);
                Ok(Token::operator(OperatorType::ArrayLiteral, tokens_slice))
            }
        },
        
        // Objects could be operators or literal objects
        JsonValue::Object(obj) => parse_object(obj, arena),
    }
}

/// Parses a JSON object into a token.
fn parse_object<'a>(obj: &JsonMap<String, JsonValue>, arena: &'a DataArena) -> Result<Token<'a>> {
    // If the object has exactly one key, it might be an operator
    if obj.len() == 1 {
        let (key, value) = obj.iter().next().unwrap();
        
        // Check if it's a variable reference
        if key == "var" {
            return parse_variable(value, arena);
        }
        
        // Check if it's a variable reference using the "val" operator (compatibility with test files)
        if key == "val" {
            return parse_variable(value, arena);
        }
        
        // Check if it's a standard operator
        if let Some(op_type) = OperatorType::from_str(key) {
            return parse_operator(op_type, value, arena);
        }
        
        // If it's not a standard operator, treat it as a custom operator
        return parse_custom_operator(key, value, arena);
    }
    
    // If it has more than one key, it's a literal object
    let mut entries = Vec::with_capacity(obj.len());
    for (key, value) in obj {
        let token = parse_json_internal(value, arena)?;
        if let Token::Literal(value) = token {
            entries.push((arena.intern_str(key), value));
        } else {
            // If it's not a literal, we need to convert it to a data value
            return Err(LogicError::ParseError {
                reason: format!("Object values must be literals, found: {:?}", token),
            });
        }
    }
    let entries_slice = arena.alloc_slice_clone(&entries);
    Ok(Token::literal(DataValue::Object(entries_slice)))
}

/// Parses a variable reference.
fn parse_variable<'a>(var_json: &JsonValue, arena: &'a DataArena) -> Result<Token<'a>> {
    match var_json {
        // Simple variable reference
        JsonValue::String(path) => {
            // For compatibility with the test suite, if the path contains dots,
            // we need to split it and handle it as a multi-level path
            if path.contains('.') {
                let parts: Vec<&str> = path.split('.').collect();
                let mut path_parts = Vec::with_capacity(parts.len());
                for part in parts {
                    path_parts.push(part.to_string());
                }
                
                let path = path_parts.join(".");
                return Ok(Token::variable(arena.intern_str(&path), None));
            }
            
            Ok(Token::variable(arena.intern_str(path), None))
        },
        
        // Variable reference with default value
        JsonValue::Array(arr) => {
            // Handle empty array - treat it as a reference to the data itself
            if arr.is_empty() {
                return Ok(Token::variable(arena.intern_str(""), None));
            }
            
            // For complex expressions in the path, we need to create a special token
            // that will evaluate the path at runtime
            if !arr[0].is_string() && !arr[0].is_number() && !arr[0].is_boolean() && !arr[0].is_null() {
                // Parse the path expression
                let path_expr = parse_json_internal(&arr[0], arena)?;
                let path_token = arena.alloc(path_expr);
                
                // If there's a default value, parse it
                let default = if arr.len() >= 2 {
                    let default_token = parse_json_internal(&arr[1], arena)?;
                    Some(arena.alloc(default_token))
                } else {
                    None
                };
                
                // Create a special token for dynamic variable paths
                return Ok(Token::dynamic_variable(path_token, default));
            }
            
            // Handle multi-level paths specified as arrays
            // For example: ["a", "b", "c"] -> "a.b.c"
            if arr.len() > 2 && arr.iter().all(|item| item.is_string() || item.is_number() || item.is_boolean() || item.is_null()) {
                // Convert all elements to strings and join with dots
                let mut path_parts = Vec::with_capacity(arr.len());
                for item in arr {
                    let part = match item {
                        JsonValue::String(s) => s.clone(),
                        JsonValue::Number(n) => n.to_string(),
                        JsonValue::Bool(b) => b.to_string(),
                        JsonValue::Null => "".to_string(),
                        _ => return Err(LogicError::ParseError {
                            reason: format!("Variable path component must be a scalar value, found: {:?}", item),
                        }),
                    };
                    path_parts.push(part);
                }
                
                let path = path_parts.join(".");
                return Ok(Token::variable(arena.intern_str(&path), None));
            }
            
            // Parse the path
            let path = match &arr[0] {
                JsonValue::String(s) => arena.intern_str(s),
                JsonValue::Number(n) => arena.intern_str(&n.to_string()),
                JsonValue::Bool(b) => arena.intern_str(&b.to_string()),
                JsonValue::Null => arena.intern_str(""),
                _ => return Err(LogicError::ParseError {
                    reason: format!("Variable path must be a scalar value, found: {:?}", arr[0]),
                }),
            };
            
            // If there's only one element, there's no default
            if arr.len() == 1 {
                return Ok(Token::variable(path, None));
            }
            
            // If there are two or more elements, the second is the default
            // Parse the default value
            let default_token = parse_json_internal(&arr[1], arena)?;
            let default = arena.alloc(default_token);
            
            return Ok(Token::variable(path, Some(default)));
        },
        
        // Handle numeric variable references (convert to string)
        JsonValue::Number(n) => {
            // For compatibility with the test suite, if the number contains a decimal point,
            // we need to split it and handle it as a multi-level path
            let n_str = n.to_string();
            if n_str.contains('.') {
                let parts: Vec<&str> = n_str.split('.').collect();
                let mut path_parts = Vec::with_capacity(parts.len());
                for part in parts {
                    path_parts.push(part.to_string());
                }
                
                let path = path_parts.join(".");
                return Ok(Token::variable(arena.intern_str(&path), None));
            }
            
            Ok(Token::variable(arena.intern_str(&n_str), None))
        },
        
        // Handle null variable reference (reference to the data itself)
        JsonValue::Null => {
            Ok(Token::variable(arena.intern_str(""), None))
        },
        
        // Invalid variable reference
        _ => Err(LogicError::ParseError {
            reason: format!("Invalid variable reference: {:?}", var_json),
        }),
    }
}

/// Parses an operator application.
fn parse_operator<'a>(op_type: OperatorType, args_json: &JsonValue, arena: &'a DataArena) -> Result<Token<'a>> {
    // Parse the arguments
    let args = parse_arguments(args_json, arena)?;
    
    // Create the operator token
    Ok(Token::operator(op_type, args))
}

/// Parses a custom operator application.
fn parse_custom_operator<'a>(name: &str, args_json: &JsonValue, arena: &'a DataArena) -> Result<Token<'a>> {
    // Parse the arguments
    let args = parse_arguments(args_json, arena)?;
    
    // Create the custom operator token
    Ok(Token::custom_operator(arena.intern_str(name), args))
}

/// Parses the arguments for an operator.
fn parse_arguments<'a>(args_json: &JsonValue, arena: &'a DataArena) -> Result<&'a [Token<'a>]> {
    match args_json {
        // Single argument
        _ if !args_json.is_array() => {
            let arg = parse_json_internal(args_json, arena)?;
            let arg_slice = arena.alloc_slice_clone(&[arg]);
            Ok(arg_slice)
        },
        
        // Multiple arguments
        JsonValue::Array(arr) => {
            // Pre-allocate space for the arguments
            let mut args = Vec::with_capacity(arr.len());
            
            // Parse each argument
            for arg_json in arr {
                let arg = parse_json_internal(arg_json, arena)?;
                args.push(arg);
            }
            
            // Allocate the arguments in the arena
            let args_slice = arena.alloc_slice_clone(&args);
            Ok(args_slice)
        },
        
        // This should never happen
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::logic::operators::comparison::ComparisonOp;
    use crate::logic::operators::arithmetic::ArithmeticOp;
    use crate::logic::operators::logical::LogicalOp;
    use crate::logic::operators::conditional::ConditionalOp;
    
    #[test]
    fn test_parse_literals() {
        let arena = DataArena::new();
        
        // Parse null
        let token = parse_json(&json!(null), &arena).unwrap();
        assert!(token.is_literal());
        assert!(token.as_literal().unwrap().is_null());
        
        // Parse boolean
        let token = parse_json(&json!(true), &arena).unwrap();
        assert!(token.is_literal());
        assert_eq!(token.as_literal().unwrap().as_bool(), Some(true));
        
        // Parse integer
        let token = parse_json(&json!(42), &arena).unwrap();
        assert!(token.is_literal());
        assert_eq!(token.as_literal().unwrap().as_i64(), Some(42));
        
        // Parse float
        let token = parse_json(&json!(3.14), &arena).unwrap();
        assert!(token.is_literal());
        assert_eq!(token.as_literal().unwrap().as_f64(), Some(3.14));
        
        // Parse string
        let token = parse_json(&json!("hello"), &arena).unwrap();
        assert!(token.is_literal());
        assert_eq!(token.as_literal().unwrap().as_str(), Some("hello"));
        
        // Parse array
        let token = parse_json(&json!([1, 2, 3]), &arena).unwrap();
        assert!(token.is_literal());
        let array = token.as_literal().unwrap().as_array().unwrap();
        assert_eq!(array.len(), 3);
        assert_eq!(array[0].as_i64(), Some(1));
        assert_eq!(array[1].as_i64(), Some(2));
        assert_eq!(array[2].as_i64(), Some(3));
        
        // Parse object
        let token = parse_json(&json!({"a": 1, "b": 2}), &arena).unwrap();
        assert!(token.is_literal());
        let object = token.as_literal().unwrap().as_object().unwrap();
        assert_eq!(object.len(), 2);
        assert_eq!(object[0].0, "a");
        assert_eq!(object[0].1.as_i64(), Some(1));
        assert_eq!(object[1].0, "b");
        assert_eq!(object[1].1.as_i64(), Some(2));
    }
    
    #[test]
    fn test_parse_variable() {
        let arena = DataArena::new();
        
        // Parse simple variable
        let token = parse_json(&json!({"var": "user.name"}), &arena).unwrap();
        assert!(token.is_variable());
        let (path, default) = token.as_variable().unwrap();
        assert_eq!(path, "user.name");
        assert!(default.is_none());
        
        // Parse variable with default
        let token = parse_json(&json!({"var": ["user.name", "Anonymous"]}), &arena).unwrap();
        assert!(token.is_variable());
        let (path, default) = token.as_variable().unwrap();
        assert_eq!(path, "user.name");
        assert!(default.is_some());
        let default_token = default.unwrap();
        assert!(default_token.is_literal());
        assert_eq!(default_token.as_literal().unwrap().as_str(), Some("Anonymous"));
    }
    
    #[test]
    fn test_parse_operator() {
        let arena = DataArena::new();
        
        // Parse comparison operator
        let token = parse_json(&json!({"==": [1, 2]}), &arena).unwrap();
        assert!(token.is_operator());
        let (op_type, args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].as_literal().unwrap().as_i64(), Some(1));
        assert_eq!(args[1].as_literal().unwrap().as_i64(), Some(2));
        
        // Parse arithmetic operator
        let token = parse_json(&json!({"+": [1, 2, 3]}), &arena).unwrap();
        assert!(token.is_operator());
        let (op_type, args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Arithmetic(ArithmeticOp::Add));
        assert_eq!(args.len(), 3);
        
        // Parse logical operator
        let token = parse_json(&json!({"and": [true, false]}), &arena).unwrap();
        assert!(token.is_operator());
        let (op_type, args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Logical(LogicalOp::And));
        assert_eq!(args.len(), 2);
    }
    
    #[test]
    fn test_parse_custom_operator() {
        let arena = DataArena::new();
        
        // Parse custom operator
        let token = parse_json(&json!({"my_op": [1, 2, 3]}), &arena).unwrap();
        assert!(token.is_custom_operator());
        let (name, args) = token.as_custom_operator().unwrap();
        assert_eq!(name, "my_op");
        assert_eq!(args.len(), 3);
    }
    
    #[test]
    fn test_parse_complex_expression() {
        let arena = DataArena::new();
        
        // Parse a complex expression
        let json = json!({
            "if": [
                {"<": [{"var": "temp"}, 0]},
                "freezing",
                {"<": [{"var": "temp"}, 20]},
                "cold",
                {"<": [{"var": "temp"}, 30]},
                "warm",
                "hot"
            ]
        });
        
        let token = parse_json(&json, &arena).unwrap();
        assert!(token.is_operator());
        let (op_type, args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Conditional(ConditionalOp::If));
        assert_eq!(args.len(), 7);
        
        // Check the first condition
        let condition1 = &args[0];
        assert!(condition1.is_operator());
        let (op_type, cond_args) = condition1.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::LessThan));
        assert_eq!(cond_args.len(), 2);
        
        // Check the variable reference
        let var = &cond_args[0];
        assert!(var.is_variable());
        let (path, _) = var.as_variable().unwrap();
        assert_eq!(path, "temp");
    }
    
    #[test]
    fn test_parse_from_string() {
        let arena = DataArena::new();
        
        // Parse from a JSON string
        let token = parse_str(r#"{"==": [{"var": "a"}, 42]}"#, &arena).unwrap();
        assert!(token.is_operator());
        let (op_type, args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
        assert_eq!(args.len(), 2);
        
        // Check the variable reference
        let var = &args[0];
        assert!(var.is_variable());
        let (path, _) = var.as_variable().unwrap();
        assert_eq!(path, "a");
        
        // Check the literal
        let lit = &args[1];
        assert!(lit.is_literal());
        assert_eq!(lit.as_literal().unwrap().as_i64(), Some(42));
    }
    
    #[test]
    fn test_parse_if() {
        let arena = DataArena::new();
        let json = json!({
            "if": [
                {"<": [{"var": "temp"}, 0]},
                "freezing",
                {"<": [{"var": "temp"}, 20]},
                "cold",
                {"<": [{"var": "temp"}, 30]},
                "warm",
                "hot"
            ]
        });
        
        let token = parse_json(&json, &arena).unwrap();
        assert!(token.is_operator());
        let (op_type, args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Conditional(ConditionalOp::If));
        assert_eq!(args.len(), 7);
        
        // Check the first condition
        let condition1 = &args[0];
        assert!(condition1.is_operator());
        let (op_type, cond_args) = condition1.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::LessThan));
        assert_eq!(cond_args.len(), 2);
        
        // Check the variable reference
        let var = &cond_args[0];
        assert!(var.is_variable());
        let (path, _) = var.as_variable().unwrap();
        assert_eq!(path, "temp");
    }
} 