//! Try operator implementation.
//!
//! This module provides the implementation of the try operator for error handling.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

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
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Check if we have arguments
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Special case for a single argument - just evaluate it
    if args.len() == 1 {
        return evaluate(args[0], arena);
    }

    // Try each expression in sequence
    let mut last_error = None;

    for (i, arg) in args.iter().enumerate() {
        // For the first expression, use the original data context
        if i == 0 {
            match evaluate(arg, arena) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        } else {
            // For subsequent expressions, we need to create an error context
            // that includes the error details from the previous attempt
            let error_context = match &last_error {
                Some(LogicError::ThrownError { r#type: error_type }) => {
                    // Create a context with the error type
                    let entries = arena.vec_into_slice(vec![(
                        arena.intern_str("type"),
                        DataValue::string(arena, error_type),
                    )]);
                    arena.alloc(DataValue::Object(entries))
                }
                Some(LogicError::NaNError) => {
                    // Create a context for NaN errors
                    let entries = arena.vec_into_slice(vec![(
                        arena.intern_str("type"),
                        DataValue::string(arena, "NaN"),
                    )]);
                    arena.alloc(DataValue::Object(entries))
                }
                Some(err) => {
                    // For other errors, just include a generic error message
                    let entries = arena.vec_into_slice(vec![(
                        arena.intern_str("type"),
                        DataValue::string(arena, &err.to_string()),
                    )]);
                    arena.alloc(DataValue::Object(entries))
                }
                None => {
                    // This shouldn't happen, but just in case
                    arena.alloc(DataValue::null())
                }
            };
            arena.set_current_context(&error_context);

            // Evaluate with the error context
            match evaluate(arg, arena) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }
    }

    // If we get here, all expressions failed; propagate the last error
    Err(last_error.unwrap_or(LogicError::InvalidArgumentsError))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::datalogic_core::DataLogicCore;
    use serde_json::json;

    #[test]
    pub fn test_try_coalesce_error() {
        // Create JSONLogic instance with arena
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!(null);

        // Test with builder API
        let rule = builder.try_op([
            builder.throw_op(builder.string_value("Some error")),
            builder.int(1),
        ]);
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result.as_i64(), Some(1));
    }

    #[test]
    pub fn test_try_propagate_error() {
        // Create JSONLogic instance with arena
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!(null);

        // Test with builder API
        let rule = builder.try_op([
            builder.throw_op(builder.string_value("Some error")),
            builder.throw_op(builder.string_value("Another error")),
        ]);
        let result = core.apply(&rule, &data_json);
        assert!(result.is_err());
        if let Err(LogicError::ThrownError { r#type: error_type }) = result {
            assert_eq!(error_type, "Another error");
        } else {
            panic!("Expected ThrownError, got: {:?}", result);
        }
    }

    #[test]
    pub fn test_try_error_context() {
        // Create JSONLogic instance with arena
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!(null);

        // Test with builder API
        let rule = builder.try_op([
            builder.throw_op(builder.string_value("Some error")),
            builder.val_str("type"),
        ]);
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result.as_str(), Some("Some error"));
    }
}
