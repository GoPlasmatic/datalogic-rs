//! Type operator implementation.
//!
//! This module provides the implementation of the "type" operator,
//! which returns the type of a value as a string.

use crate::arena::DataArena;
use crate::context::EvalContext;
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
pub fn eval_type<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Validate arguments: we need exactly one argument
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the argument to get its value
    let value = evaluate(args[0], context, arena)?;

    // Get the type name as a string
    let type_name = value.type_name();

    // Create a string DataValue with the type name
    let result = DataValue::string(arena, type_name);

    // Return the result
    Ok(arena.alloc(result))
}

#[cfg(test)]
mod tests {
    use crate::logic::Logic;
    use crate::logic::datalogic_core::DataLogicCore;
    use crate::logic::token::{OperatorType, Token};
    use crate::value::DataValue;
    use serde_json::json;

    #[test]
    fn test_type_operator() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Test with different types
        // Create {"type": 42}
        let int_token = Token::literal(DataValue::integer(42));
        let int_ref = arena.alloc(int_token);

        let type_token = Token::operator(OperatorType::Type, int_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("number"));

        // Create {"type": "hello"}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let str_token = Token::literal(DataValue::string(arena, "hello"));
        let str_ref = arena.alloc(str_token);

        let type_token = Token::operator(OperatorType::Type, str_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("string"));

        // Create {"type": []}
        let core = DataLogicCore::new();
        let arena = core.arena();

        // First create a literal empty array value
        let empty_array_value = DataValue::Array(&[]);
        let empty_array_token = Token::literal(empty_array_value);
        let empty_array_ref = arena.alloc(empty_array_token);

        // Now use that as the argument to type
        let type_token = Token::operator(OperatorType::Type, empty_array_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("array"));

        // Create {"type": {}}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let empty_obj_token = Token::literal(DataValue::Object(&[]));
        let empty_obj_ref = arena.alloc(empty_obj_token);

        let type_token = Token::operator(OperatorType::Type, empty_obj_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("object"));

        // Create {"type": null}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let null_token = Token::literal(DataValue::null());
        let null_ref = arena.alloc(null_token);

        let type_token = Token::operator(OperatorType::Type, null_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("null"));

        // Create {"type": true}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let bool_token = Token::literal(DataValue::Bool(true));
        let bool_ref = arena.alloc(bool_token);

        let type_token = Token::operator(OperatorType::Type, bool_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &json!({})).unwrap();
        assert_eq!(result, json!("boolean"));
    }

    #[test]
    fn test_type_with_variables() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data = json!({
            "number_val": 42,
            "string_val": "hello",
            "array_val": [1, 2, 3],
            "object_val": {"a": 1},
            "null_val": null,
            "bool_val": true
        });

        // Create {"type": {"var": "number_val"}}
        let number_var_token = Token::variable("number_val", None);
        let number_var_ref = arena.alloc(number_var_token);

        let type_token = Token::operator(OperatorType::Type, number_var_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("number"));

        // Create {"type": {"var": "string_val"}}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let string_var_token = Token::variable("string_val", None);
        let string_var_ref = arena.alloc(string_var_token);

        let type_token = Token::operator(OperatorType::Type, string_var_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("string"));

        // Create {"type": {"var": "array_val"}}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let array_var_token = Token::variable("array_val", None);
        let array_var_ref = arena.alloc(array_var_token);

        let type_token = Token::operator(OperatorType::Type, array_var_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("array"));

        // Create {"type": {"var": "object_val"}}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let object_var_token = Token::variable("object_val", None);
        let object_var_ref = arena.alloc(object_var_token);

        let type_token = Token::operator(OperatorType::Type, object_var_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("object"));

        // Create {"type": {"var": "null_val"}}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let null_var_token = Token::variable("null_val", None);
        let null_var_ref = arena.alloc(null_var_token);

        let type_token = Token::operator(OperatorType::Type, null_var_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("null"));

        // Create {"type": {"var": "bool_val"}}
        let core = DataLogicCore::new();
        let arena = core.arena();

        let bool_var_token = Token::variable("bool_val", None);
        let bool_var_ref = arena.alloc(bool_var_token);

        let type_token = Token::operator(OperatorType::Type, bool_var_ref);
        let type_ref = arena.alloc(type_token);

        let rule = Logic::new(type_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!("boolean"));
    }
}
