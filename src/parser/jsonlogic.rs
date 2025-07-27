//! JSONLogic parser implementation
//!
//! This module provides the parser for JSONLogic expressions.

use std::str::FromStr;

use crate::arena::DataArena;
use crate::logic::{LogicError, OperatorType, Result, Token};
use crate::parser::ExpressionParser;
use crate::value::{DataValue, FromJson};
use serde_json::{Map as JsonMap, Value as JsonValue};

/// Parser for JSONLogic expressions
pub struct JsonLogicParser;

impl ExpressionParser for JsonLogicParser {
    fn parse<'a>(&self, input: &str, arena: &'a DataArena) -> Result<&'a Token<'a>> {
        // Parse the input string as JSON
        let json: JsonValue = serde_json::from_str(input).map_err(|e| LogicError::ParseError {
            reason: format!("Invalid JSON: {e}"),
        })?;

        // Use the JSONLogic parsing logic
        parse_json(&json, arena)
    }

    fn parse_json<'a>(&self, input: &JsonValue, arena: &'a DataArena) -> Result<&'a Token<'a>> {
        parse_json(input, arena)
    }

    fn parse_with_preserve<'a>(
        &self,
        input: &str,
        arena: &'a DataArena,
        preserve_structure: bool,
    ) -> Result<&'a Token<'a>> {
        // Parse the input string as JSON
        let json: JsonValue = serde_json::from_str(input).map_err(|e| LogicError::ParseError {
            reason: format!("Invalid JSON: {e}"),
        })?;

        // Use the JSONLogic parsing logic with preserve structure option
        parse_json_with_preserve(&json, arena, preserve_structure)
    }

    fn parse_json_with_preserve<'a>(
        &self,
        input: &JsonValue,
        arena: &'a DataArena,
        preserve_structure: bool,
    ) -> Result<&'a Token<'a>> {
        parse_json_with_preserve(input, arena, preserve_structure)
    }

    fn format_name(&self) -> &'static str {
        "jsonlogic"
    }
}

/// Checks if a JSON value is a literal.
fn is_json_literal(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => true,
        JsonValue::Array(arr) => {
            // Nested arrays are allowed if they only contain literals
            arr.iter().all(is_json_literal)
        }
        JsonValue::Object(_) => false,
    }
}

/// Parses a logic expression from a JSON value.
pub fn parse_json<'a>(json: &JsonValue, arena: &'a DataArena) -> Result<&'a Token<'a>> {
    let token = parse_json_internal(json, arena, false)?;
    Ok(arena.alloc(token))
}

/// Parses a logic expression from a JSON value with structure preservation option.
pub fn parse_json_with_preserve<'a>(
    json: &JsonValue,
    arena: &'a DataArena,
    preserve_structure: bool,
) -> Result<&'a Token<'a>> {
    let token = parse_json_internal(json, arena, preserve_structure)?;
    Ok(arena.alloc(token))
}

/// Internal function for parsing a JSON value into a token.
fn parse_json_internal<'a>(
    json: &JsonValue,
    arena: &'a DataArena,
    preserve_structure: bool,
) -> Result<Token<'a>> {
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
                    reason: format!("Invalid number: {n}"),
                })
            }
        }
        JsonValue::String(s) => Ok(Token::literal(DataValue::string(arena, s))),

        // Arrays could be literal arrays or token arrays
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
                let values_slice = arena.vec_into_slice(values);
                Ok(Token::literal(DataValue::Array(values_slice)))
            } else {
                // Otherwise, create an array of tokens and allocate them in the arena
                let mut tokens = Vec::with_capacity(arr.len());
                for item in arr {
                    let token = parse_json_internal(item, arena, preserve_structure)?;
                    let token_ref = arena.alloc(token);
                    tokens.push(token_ref);
                }
                Ok(Token::ArrayLiteral(tokens))
            }
        }

        // Objects could be operators or literal objects
        JsonValue::Object(obj) => parse_object(obj, arena, preserve_structure),
    }
}

/// Parses a JSON object into a token.
fn parse_object<'a>(
    obj: &JsonMap<String, JsonValue>,
    arena: &'a DataArena,
    preserve_structure: bool,
) -> Result<Token<'a>> {
    // If the object has exactly one key, it might be an operator
    if obj.len() == 1 {
        let (key, value) = obj.iter().next().unwrap();

        match key.as_str() {
            "var" => parse_variable(value, arena, preserve_structure),
            "val" => {
                let token = parse_json_internal(value, arena, preserve_structure)?;
                let args_token = arena.alloc(token);
                Ok(Token::operator(OperatorType::Val, args_token))
            }
            "exists" => parse_exists_operator(value, arena, preserve_structure),
            "preserve" => {
                // The preserve operator returns its argument as-is without parsing it as an operator
                let preserved_value = DataValue::from_json(value, arena);
                Ok(Token::literal(preserved_value))
            }
            _ => {
                // Check if it's a standard operator
                if let Ok(op_type) = OperatorType::from_str(key) {
                    return parse_operator(op_type, value, arena, preserve_structure);
                }

                // Check if this is a registered custom operator
                if arena.has_custom_operator(key) {
                    // Always treat registered custom operators as operators, regardless of preserve_structure
                    parse_custom_operator(key, value, arena, preserve_structure)
                } else if preserve_structure {
                    // Create a structured object with this single field
                    let value_token = parse_json_internal(value, arena, preserve_structure)?;
                    let value_token_ref = arena.alloc(value_token);
                    let key_str = arena.intern_str(key);
                    let fields = vec![(key_str, value_token_ref)];
                    let fields_slice = arena.vec_into_slice(fields);
                    Ok(Token::structured_object(fields_slice))
                } else {
                    // Otherwise, treat it as a custom operator
                    parse_custom_operator(key, value, arena, preserve_structure)
                }
            }
        }
    } else if obj.is_empty() {
        // Empty object literal
        Ok(Token::literal(DataValue::Object(
            arena.vec_into_slice(vec![]),
        )))
    } else {
        // Multi-key objects
        if preserve_structure {
            // When structure preservation is enabled, create a structured object
            let mut fields = Vec::with_capacity(obj.len());
            for (key, value) in obj {
                let value_token = parse_json_internal(value, arena, preserve_structure)?;
                let value_token_ref = arena.alloc(value_token);
                let key_str = arena.intern_str(key);
                fields.push((key_str, value_token_ref));
            }
            let fields_slice = arena.vec_into_slice(fields);
            Ok(Token::structured_object(fields_slice))
        } else {
            // Original behavior: treat the first key as an unknown operator
            // This matches the JSONLogic behavior where multi-key objects should
            // fail as unknown operators rather than parse errors
            let (key, _) = obj.iter().next().unwrap();

            // Return an OperatorNotFoundError instead of a ParseError
            Err(LogicError::OperatorNotFoundError {
                operator: key.clone(),
            })
        }
    }
}

/// Parses a variable reference.
fn parse_variable<'a>(
    var_json: &JsonValue,
    arena: &'a DataArena,
    preserve_structure: bool,
) -> Result<Token<'a>> {
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
        }

        // Variable reference with default value
        JsonValue::Array(arr) => {
            // Handle empty array - treat it as a reference to the data itself
            if arr.is_empty() {
                return Ok(Token::variable(arena.intern_str(""), None));
            }

            // For complex expressions in the path, we need to create a special token
            // that will evaluate the path at runtime
            if !arr[0].is_string()
                && !arr[0].is_number()
                && !arr[0].is_boolean()
                && !arr[0].is_null()
            {
                // Parse the path expression
                let path_expr = parse_json_internal(&arr[0], arena, preserve_structure)?;
                let path_token = arena.alloc(path_expr);

                // If there's a default value, parse it
                let default = if arr.len() >= 2 {
                    let default_token = parse_json_internal(&arr[1], arena, preserve_structure)?;
                    Some(arena.alloc(default_token))
                } else {
                    None
                };

                // Create a special token for dynamic variable paths
                return Ok(Token::dynamic_variable(path_token, default));
            }

            // If we have exactly two elements, it's likely a path with a default value
            if arr.len() == 2
                && (arr[0].is_string()
                    || arr[0].is_number()
                    || arr[0].is_boolean()
                    || arr[0].is_null())
            {
                let path = match &arr[0] {
                    JsonValue::String(s) => arena.intern_str(s),
                    JsonValue::Number(n) => arena.intern_str(&n.to_string()),
                    JsonValue::Bool(b) => arena.intern_str(&b.to_string()),
                    JsonValue::Null => arena.intern_str(""),
                    _ => unreachable!(),
                };

                // Parse the default value
                let default_token = parse_json_internal(&arr[1], arena, preserve_structure)?;
                let default = arena.alloc(default_token);

                return Ok(Token::variable(path, Some(default)));
            }

            // Handle array of strings as a path with dots
            // For example: ["person", "name", "first"] -> "person.name.first"
            if arr.iter().all(|item| {
                item.is_string() || item.is_number() || item.is_boolean() || item.is_null()
            }) {
                // Convert all elements to strings and join with dots
                let mut path_parts = Vec::with_capacity(arr.len());
                for item in arr {
                    let part = match item {
                        JsonValue::String(s) => s.clone(),
                        JsonValue::Number(n) => n.to_string(),
                        JsonValue::Bool(b) => b.to_string(),
                        JsonValue::Null => "".to_string(),
                        _ => {
                            return Err(LogicError::ParseError {
                                reason: format!(
                                    "Variable path component must be a scalar value, found: {item:?}"
                                ),
                            });
                        }
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
                _ => {
                    return Err(LogicError::ParseError {
                        reason: format!(
                            "Variable path must be a scalar value, found: {:?}",
                            arr[0]
                        ),
                    });
                }
            };

            // If there's only one element, there's no default
            if arr.len() == 1 {
                return Ok(Token::variable(path, None));
            }

            // If there are two or more elements, the second is the default
            // Parse the default value
            let default_token = parse_json_internal(&arr[1], arena, preserve_structure)?;
            let default = arena.alloc(default_token);

            Ok(Token::variable(path, Some(default)))
        }

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
        }

        // Handle null variable reference (reference to the data itself)
        JsonValue::Null => Ok(Token::variable(arena.intern_str(""), None)),

        // Handle object as variable path (e.g., {"cat": ["te", "st"]})
        JsonValue::Object(_) => {
            // Parse the object as a regular expression
            let path_expr = parse_json_internal(var_json, arena, preserve_structure)?;
            let path_token = arena.alloc(path_expr);

            // Create a dynamic variable reference where the path will be evaluated at runtime
            Ok(Token::dynamic_variable(path_token, None))
        }

        // Invalid variable reference
        _ => Err(LogicError::ParseError {
            reason: format!("Invalid variable reference: {var_json:?}"),
        }),
    }
}

/// Parses an operator application.
fn parse_operator<'a>(
    op_type: OperatorType,
    args_json: &JsonValue,
    arena: &'a DataArena,
    preserve_structure: bool,
) -> Result<Token<'a>> {
    // Parse the arguments
    let args = parse_arguments(args_json, arena, preserve_structure)?;

    // Create the operator token
    Ok(Token::operator(op_type, args))
}

/// Parses a custom operator application.
fn parse_custom_operator<'a>(
    name: &str,
    args_json: &JsonValue,
    arena: &'a DataArena,
    preserve_structure: bool,
) -> Result<Token<'a>> {
    // Parse the arguments
    let args = parse_arguments(args_json, arena, preserve_structure)?;

    // Create the custom operator token
    Ok(Token::custom_operator(arena.intern_str(name), args))
}

/// Parses the arguments for an operator.
fn parse_arguments<'a>(
    args_json: &JsonValue,
    arena: &'a DataArena,
    preserve_structure: bool,
) -> Result<&'a Token<'a>> {
    match args_json {
        // Single argument that's not an array - no need for ArrayLiteral
        _ if !args_json.is_array() => {
            let arg = parse_json_internal(args_json, arena, preserve_structure)?;
            Ok(arena.alloc(arg))
        }

        // Empty array - create an empty ArrayLiteral
        JsonValue::Array(arr) if arr.is_empty() => {
            let empty_array_token = Token::ArrayLiteral(Vec::new());
            Ok(arena.alloc(empty_array_token))
        }

        // Multiple arguments as array
        JsonValue::Array(arr) => {
            let mut tokens = Vec::with_capacity(arr.len());

            // Parse each argument
            for arg_json in arr {
                let arg = parse_json_internal(arg_json, arena, preserve_structure)?;
                let arg_ref = arena.alloc(arg);
                tokens.push(arg_ref);
            }

            // Create an array literal token
            let array_token = Token::ArrayLiteral(tokens);
            Ok(arena.alloc(array_token))
        }

        // Should never reach here due to the first match arm
        _ => unreachable!(),
    }
}

/// Parses the exists operator application.
fn parse_exists_operator<'a>(
    value: &JsonValue,
    arena: &'a DataArena,
    preserve_structure: bool,
) -> Result<Token<'a>> {
    // Parse the arguments for exists operator
    let args = parse_arguments(value, arena, preserve_structure)?;

    // Create the exists operator token
    Ok(Token::operator(OperatorType::Exists, args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;
    use crate::logic::{ArithmeticOp, ComparisonOp, ControlOp};
    use serde_json::json;

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
        let token = parse_json(&json!(3.5), &arena).unwrap();
        assert!(token.is_literal());
        assert_eq!(token.as_literal().unwrap().as_f64(), Some(3.5));

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
        assert_eq!(
            default_token.as_literal().unwrap().as_str(),
            Some("Anonymous")
        );
    }

    #[test]
    fn test_parse_operator() {
        let arena = DataArena::new();

        // Parse comparison operator
        let token = parse_json(&json!({"==": [1, 2]}), &arena).unwrap();
        assert!(token.is_operator());

        let (op_type, _args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));

        // Parse arithmetic operator
        let token = parse_json(&json!({"+": [1, 2, 3]}), &arena).unwrap();
        assert!(token.is_operator());

        let (op_type, _args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Arithmetic(ArithmeticOp::Add));

        // Parse logical operator
        let token = parse_json(&json!({"and": [true, false]}), &arena).unwrap();
        assert!(token.is_operator());

        let (op_type, _args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Control(ControlOp::And));
    }

    #[test]
    fn test_parse_custom_operator() {
        let arena = DataArena::new();

        // Parse custom operator
        let token = parse_json(&json!({"my_op": [1, 2, 3]}), &arena).unwrap();
        assert!(token.is_custom_operator());

        let (name, _args) = token.as_custom_operator().unwrap();
        assert_eq!(name, "my_op");
    }

    #[test]
    fn test_parse_complex_expression() {
        let arena = DataArena::new();

        // Parse complex expression
        let json = json!({
            "if": [
                {"<": [{"var": "temp"}, 0]}, "freezing",
                {"<": [{"var": "temp"}, 20]}, "cold",
                {"<": [{"var": "temp"}, 30]}, "warm",
                "hot"
            ]
        });

        let token = parse_json(&json, &arena).unwrap();
        assert!(token.is_operator());

        let (op_type, _args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Control(ControlOp::If));
    }

    #[test]
    fn test_parser_interface() {
        let arena = DataArena::new();
        let parser = JsonLogicParser;

        // Test the parser interface
        let json_str = r#"{"==": [{"var": "a"}, 42]}"#;
        let token = parser.parse(json_str, &arena).unwrap();

        // Verify the token
        assert!(token.is_operator());
        let (op_type, _args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));

        // Check the format name
        assert_eq!(parser.format_name(), "jsonlogic");
    }

    #[test]
    fn test_parse_preserve_operator() {
        let arena = DataArena::new();

        // Test preserve with a literal
        let json = json!({"preserve": 42});
        let token = parse_json(&json, &arena).unwrap();
        if let Token::Literal(value) = token {
            assert_eq!(value.as_i64(), Some(42));
        } else {
            panic!("Expected literal token, got: {token:?}");
        }

        // Test preserve with an array
        let json = json!({"preserve": [1, 2, 3]});
        let token = parse_json(&json, &arena).unwrap();
        if let Token::Literal(value) = token {
            assert!(value.is_array());
            let arr = value.as_array().unwrap();
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0].as_i64(), Some(1));
            assert_eq!(arr[1].as_i64(), Some(2));
            assert_eq!(arr[2].as_i64(), Some(3));
        } else {
            panic!("Expected literal token, got: {token:?}");
        }

        // Test preserve with an object
        let json = json!({"preserve": {"a": 1, "b": 2}});
        let token = parse_json(&json, &arena).unwrap();
        if let Token::Literal(value) = token {
            assert!(value.is_object());
            let obj = value.as_object().unwrap();
            assert_eq!(obj.len(), 2);
        } else {
            panic!("Expected literal token, got: {token:?}");
        }
    }

    #[test]
    fn test_parse_val_operator() {
        let arena = DataArena::new();

        // Test simple val operator: {"val": "hello"}
        let json_str = r#"{"val": "hello"}"#;
        let token = parse_json(&serde_json::from_str(json_str).unwrap(), &arena).unwrap();

        // Check that it's a Val operator
        let (op_type, _args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Val);

        // Test nested val operator: {"val": ["hello", "world"]}
        let json_str = r#"{"val": ["hello", "world"]}"#;
        let token = parse_json(&serde_json::from_str(json_str).unwrap(), &arena).unwrap();

        // Check that it's a Val operator
        let (op_type, _args) = token.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Val);
    }
}
