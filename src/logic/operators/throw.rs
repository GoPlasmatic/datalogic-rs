//! Throw operator implementation.
//!
//! This module provides the implementation of the throw operator.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Validate that at least one argument is provided for throw
fn validate_throw_args(args: &[&Token]) -> Result<()> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }
    Ok(())
}

/// Extract error message from a value
fn extract_error_message<'a>(error_value: &'a DataValue<'a>) -> String {
    // For string values, use them directly as the error type
    if let Some(error_str) = error_value.as_str() {
        return error_str.to_string();
    }

    // Handle object values with a "type" field
    if let Some(obj) = error_value.as_object() {
        for (key, value) in obj {
            if *key == "type" {
                if let Some(type_str) = value.as_str() {
                    return type_str.to_string();
                }
            }
        }
    }

    // For other values, convert to string
    if let Some(i) = error_value.as_i64() {
        i.to_string()
    } else if let Some(f) = error_value.as_f64() {
        f.to_string()
    } else if let Some(b) = error_value.as_bool() {
        b.to_string()
    } else if error_value.is_null() {
        "null".to_string()
    } else {
        "Unknown error".to_string()
    }
}

/// Evaluates a throw operation.
/// The throw operator throws an error with the provided value.
#[inline]
pub fn eval_throw<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    validate_throw_args(args)?;

    // Evaluate the first argument to get the error value/type
    let error_value = evaluate(args[0], arena)?;
    let error_message = extract_error_message(error_value);

    Err(LogicError::thrown_error(error_message))
}

#[cfg(test)]
mod tests {
    use crate::logic::datalogic_core::DataLogicCore;
    use crate::logic::error::LogicError;
    use crate::logic::token::{OperatorType, Token};
    use crate::logic::Logic;
    use crate::value::DataValue;
    use serde_json::json;

    #[test]
    fn test_evaluate_throw_string() {
        // Create DataLogic instance with arena
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!(null);

        // Create {"throw": "hello"}
        let hello_token = Token::literal(DataValue::string(arena, "hello"));
        let hello_ref = arena.alloc(hello_token);

        let throw_token = Token::operator(OperatorType::Throw, hello_ref);
        let throw_ref = arena.alloc(throw_token);

        let rule = Logic::new(throw_ref, arena);

        let result = core.apply(&rule, &data_json);
        assert!(result.is_err());
        if let Err(LogicError::ThrownError { r#type: error_type }) = result {
            assert_eq!(error_type, "hello");
        } else {
            panic!("Expected ThrownError, got: {:?}", result);
        }
    }

    #[test]
    fn test_evaluate_throw_object() {
        // Create DataLogic instance with arena
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({
            "x": {"type": "Some error"}
        });

        // Create {"throw": {"var": "x"}}
        let x_var_token = Token::variable("x", None);
        let x_var_ref = arena.alloc(x_var_token);

        let throw_token = Token::operator(OperatorType::Throw, x_var_ref);
        let throw_ref = arena.alloc(throw_token);

        let rule = Logic::new(throw_ref, arena);

        let result = core.apply(&rule, &data_json);
        assert!(result.is_err());
        if let Err(LogicError::ThrownError { r#type: error_type }) = result {
            assert_eq!(error_type, "Some error");
        } else {
            panic!("Expected ThrownError, got: {:?}", result);
        }
    }
}
