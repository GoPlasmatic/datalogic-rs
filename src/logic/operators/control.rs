//! Logical operators for logic expressions.
//!
//! This module provides implementations for logical operators
//! such as and, or, not, etc.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Enumeration of logical operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlOp {
    /// If operator
    If,
    /// Logical AND
    And,
    /// Logical OR
    Or,
    /// Logical NOT
    Not,
    /// Logical Double Negation
    DoubleNegation,
}

/// Evaluates an if operation.
pub fn eval_if<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.is_empty() {
        return Ok(arena.null_value());
    }

    // Process arguments in pairs (condition, value)
    let mut i = 0;
    while i + 1 < args.len() {
        // Evaluate the condition
        let condition = evaluate(args[i], arena)?;

        // If the condition is true, return the value
        if condition.coerce_to_bool() {
            return evaluate(args[i + 1], arena);
        }

        // Move to the next pair
        i += 2;
    }

    // If there's an odd number of arguments, the last one is the "else" value
    if i < args.len() {
        return evaluate(args[i], arena);
    }

    // No conditions matched and no else value
    Ok(arena.null_value())
}

/// Evaluates an AND operation.
pub fn eval_and<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Ok(arena.null_value());
    }

    // Fast path for single argument
    if args.len() == 1 {
        return evaluate(args[0], arena);
    }

    // Evaluate each argument with short-circuit evaluation
    let mut last_value = arena.null_value();

    for arg in args {
        let value = evaluate(arg, arena)?;
        last_value = value;

        // If any argument is false, short-circuit and return that value
        if !value.coerce_to_bool() {
            return Ok(value);
        }
    }

    // All arguments are true, return the last value
    Ok(last_value)
}

/// Evaluates an OR operation.
pub fn eval_or<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Ok(arena.false_value());
    }

    // Fast path for single argument
    if args.len() == 1 {
        return evaluate(args[0], arena);
    }

    // Evaluate each argument with short-circuit evaluation
    let mut last_value = arena.false_value();

    for arg in args {
        let value = evaluate(arg, arena)?;
        last_value = value;

        // If any argument is true, short-circuit and return that value
        if value.coerce_to_bool() {
            return Ok(value);
        }
    }

    // All arguments are false, return the last value
    Ok(last_value)
}

/// Evaluates a logical NOT operation.
pub fn eval_not<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() != 1 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let value = evaluate(args[0], arena)?;
    Ok(arena.alloc(DataValue::Bool(!value.coerce_to_bool())))
}

/// Evaluates a logical double negation (!!).
pub fn eval_double_negation<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 1 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let value = evaluate(args[0], arena)?;
    Ok(arena.alloc(DataValue::Bool(value.coerce_to_bool())))
}

#[cfg(test)]
mod tests {
    use crate::logic::datalogic_core::DataLogicCore;
    use serde_json::json;

    #[test]
    fn test_and() {
        // Create JSONLogic instance with arena
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for true AND true
        let rule = builder.control().and_op().bool(true).bool(true).build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for true AND false
        let rule = builder.control().and_op().bool(true).bool(false).build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with variables
        let rule = builder.control().and_op().var("a").var("b").build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(0));

        // Test with multiple values - should return last truthy value or first falsy
        let rule = builder.control().and_op().int(1).int(2).int(3).build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn test_or() {
        // Create JSONLogic instance with arena
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for true OR true
        let rule = builder.control().or_op().bool(true).bool(true).build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for true OR false
        let rule = builder.control().or_op().bool(true).bool(false).build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for false OR true
        let rule = builder.control().or_op().bool(false).bool(true).build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for false OR false
        let rule = builder.control().or_op().bool(false).bool(false).build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with variables
        let rule = builder.control().or_op().var("a").var("b").build();
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn test_not() {
        // Create JSONLogic instance with arena
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for NOT true
        let rule = builder.control().not_op(builder.bool(true));
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test for NOT false
        let rule = builder.control().not_op(builder.bool(false));
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test with variables
        let rule = builder.control().not_op(builder.var("a").build());
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with falsy variable
        let rule = builder.control().not_op(builder.var("b").build());
        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));
    }
}
