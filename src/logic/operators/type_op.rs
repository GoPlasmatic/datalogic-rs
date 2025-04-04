//! Type operator implementation.
//!
//! This module provides the implementation of the "type" operator,
//! which returns the type of a value as a string.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Evaluates the 'type' operator, which returns the type of a value.
///
/// The operator takes a single argument and returns a string representing its type.
///
/// Examples:
/// ```json
/// {"type": 42} => "number"
/// {"type": "hello"} => "string"
/// {"type": [1, 2, 3]} => "array"
/// {"type": {"a": 1}} => "object"
/// {"type": null} => "null"
/// {"type": true} => "boolean"
/// ```
#[inline]
pub fn eval_type<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Validate arguments: we need exactly one argument
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the argument to get its value
    let value = evaluate(args[0], arena)?;

    // Get the type name as a string
    let type_name = value.type_name();

    // Create a string DataValue with the type name
    let result = DataValue::string(arena, type_name);

    // Return the result
    Ok(arena.alloc(result))
}

#[cfg(test)]
mod tests {
    use crate::logic::datalogic_core::DataLogicCore;
    use serde_json::json;

    #[test]
    fn test_type_operator() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        // Test with different types
        let rule = builder.type_op().int(42).build();
        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("number"));

        let rule = builder.type_op().string("hello").build();
        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("string"));

        let rule = builder.type_op().array().build();
        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("array"));

        let rule = builder.type_op().object().build();
        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("object"));

        let rule = builder.type_op().null().build();
        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("null"));

        let rule = builder.type_op().bool(true).build();
        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("boolean"));
    }

    #[test]
    fn test_type_with_variables() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data = json!({
            "number_val": 42,
            "string_val": "hello",
            "array_val": [1, 2, 3],
            "object_val": {"a": 1},
            "null_val": null,
            "bool_val": true
        });

        let rule = builder.type_op().var("number_val").build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("number"));

        let rule = builder.type_op().var("string_val").build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("string"));

        let rule = builder.type_op().var("array_val").build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("array"));

        let rule = builder.type_op().var("object_val").build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("object"));

        let rule = builder.type_op().var("null_val").build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("null"));

        let rule = builder.type_op().var("bool_val").build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("boolean"));
    }
}
