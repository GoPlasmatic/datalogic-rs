//! Try operator implementation.
//!
//! This module provides the implementation of the try operator for error handling.

use crate::arena::DataArena;
use crate::context::EvalContext;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Validate that at least one argument is provided
fn validate_try_args(args: &[&Token]) -> Result<()> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }
    Ok(())
}

/// Create an error context object from a LogicError
fn create_error_context<'a>(error: &LogicError, arena: &'a DataArena) -> &'a DataValue<'a> {
    match error {
        LogicError::ThrownError { r#type: error_type } => {
            // Create a context with the error type
            let entries = arena.vec_into_slice(vec![(
                arena.intern_str("type"),
                DataValue::string(arena, error_type),
            )]);
            arena.alloc(DataValue::Object(entries))
        }
        LogicError::NaNError => {
            // Create a context for NaN errors
            let entries = arena.vec_into_slice(vec![(
                arena.intern_str("type"),
                DataValue::string(arena, "NaN"),
            )]);
            arena.alloc(DataValue::Object(entries))
        }
        err => {
            // For other errors, just include a generic error message
            let entries = arena.vec_into_slice(vec![(
                arena.intern_str("type"),
                DataValue::string(arena, &err.to_string()),
            )]);
            arena.alloc(DataValue::Object(entries))
        }
    }
}

/// Try to evaluate a single expression, returning the result or the error
fn try_evaluate_expression<'a>(
    expr: &'a Token<'a>,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> std::result::Result<&'a DataValue<'a>, LogicError> {
    evaluate(expr, context, arena)
}

/// Create error context for next evaluation by pushing onto existing context
fn create_error_context_for_eval<'a>(
    error: &LogicError,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> EvalContext<'a> {
    // Create error context
    let error_context = create_error_context(error, arena);

    // Push error context onto existing context stack
    context.push(error_context)
}

/// Evaluates a try operation.
/// The try operator attempts to evaluate a sequence of expressions, returning
/// the result of the first one that succeeds without an error.
/// If all expressions fail, the last error is propagated.
///
/// When an error occurs, subsequent expressions are evaluated with the error
/// as the context, allowing them to examine the error's properties.
#[inline]
pub fn eval_try<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    validate_try_args(args)?;

    // Special case for a single argument - just evaluate it
    if args.len() == 1 {
        return evaluate(args[0], context, arena);
    }

    // Try each expression in sequence
    let mut last_error = None;

    for (i, arg) in args.iter().enumerate() {
        // For the first expression, use the original data context
        if i == 0 {
            match try_evaluate_expression(arg, context, arena) {
                Ok(result) => return Ok(result),
                Err(e) => last_error = Some(e),
            }
        } else if let Some(ref error) = last_error {
            // Create error context for this evaluation
            let error_context = create_error_context_for_eval(error, context, arena);

            // Evaluate with the error context
            match try_evaluate_expression(arg, &error_context, arena) {
                Ok(result) => return Ok(result),
                Err(e) => last_error = Some(e),
            }
        }
    }

    // If we get here, all expressions failed; propagate the last error
    Err(last_error.unwrap_or(LogicError::InvalidArgumentsError))
}

#[cfg(test)]
mod tests {
    use crate::logic::Logic;
    use crate::logic::datalogic_core::DataLogicCore;
    use crate::logic::error::LogicError;
    use crate::logic::token::{OperatorType, Token};
    use crate::value::DataValue;
    use serde_json::json;

    #[test]
    pub fn test_try_coalesce_error() {
        // Create DataLogic instance with arena
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!(null);

        // Create {"try": [{"throw": "Some error"}, 1]}

        // First, create the throw token
        let error_str_token = Token::literal(DataValue::string(arena, "Some error"));
        let error_str_ref = arena.alloc(error_str_token);

        let throw_token = Token::operator(OperatorType::Throw, error_str_ref);
        let throw_ref = arena.alloc(throw_token);

        // Create the literal 1
        let one_token = Token::literal(DataValue::integer(1));
        let one_ref = arena.alloc(one_token);

        // Create the try with both arguments
        let try_args = vec![throw_ref, one_ref];
        let try_array_token = Token::ArrayLiteral(try_args);
        let try_array_ref = arena.alloc(try_array_token);

        let try_token = Token::operator(OperatorType::Try, try_array_ref);
        let try_ref = arena.alloc(try_token);

        let rule = Logic::new(try_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result.as_i64(), Some(1));
    }

    #[test]
    pub fn test_try_propagate_error() {
        // Create DataLogic instance with arena
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!(null);

        // Create {"try": [{"throw": "Some error"}, {"throw": "Another error"}]}

        // First, create the first throw token
        let error1_str_token = Token::literal(DataValue::string(arena, "Some error"));
        let error1_str_ref = arena.alloc(error1_str_token);

        let throw1_token = Token::operator(OperatorType::Throw, error1_str_ref);
        let throw1_ref = arena.alloc(throw1_token);

        // Create the second throw token
        let error2_str_token = Token::literal(DataValue::string(arena, "Another error"));
        let error2_str_ref = arena.alloc(error2_str_token);

        let throw2_token = Token::operator(OperatorType::Throw, error2_str_ref);
        let throw2_ref = arena.alloc(throw2_token);

        // Create the try with both throw arguments
        let try_args = vec![throw1_ref, throw2_ref];
        let try_array_token = Token::ArrayLiteral(try_args);
        let try_array_ref = arena.alloc(try_array_token);

        let try_token = Token::operator(OperatorType::Try, try_array_ref);
        let try_ref = arena.alloc(try_token);

        let rule = Logic::new(try_ref, arena);

        let result = core.apply(&rule, &data_json);
        assert!(result.is_err());
        if let Err(LogicError::ThrownError { r#type: error_type }) = result {
            assert_eq!(error_type, "Another error");
        } else {
            panic!("Expected ThrownError, got: {result:?}");
        }
    }

    #[test]
    pub fn test_try_error_context() {
        // Create DataLogic instance with arena
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!(null);

        // Create {"try": [{"throw": "Some error"}, {"var": "type"}]}

        // First, create the throw token
        let error_str_token = Token::literal(DataValue::string(arena, "Some error"));
        let error_str_ref = arena.alloc(error_str_token);

        let throw_token = Token::operator(OperatorType::Throw, error_str_ref);
        let throw_ref = arena.alloc(throw_token);

        // Create the variable access
        let type_var_token = Token::variable("type", None);
        let type_var_ref = arena.alloc(type_var_token);

        // Create the try with both arguments
        let try_args = vec![throw_ref, type_var_ref];
        let try_array_token = Token::ArrayLiteral(try_args);
        let try_array_ref = arena.alloc(try_array_token);

        let try_token = Token::operator(OperatorType::Try, try_array_ref);
        let try_ref = arena.alloc(try_token);

        let rule = Logic::new(try_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result.as_str(), Some("Some error"));
    }
}
