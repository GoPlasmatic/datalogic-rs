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
    use crate::logic::operators::control::ControlOp;
    use crate::logic::token::{OperatorType, Token};
    use crate::logic::Logic;
    use crate::value::DataValue;
    use serde_json::json;

    #[test]
    fn test_and() {
        // Create DataLogic instance with arena
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for true AND true
        // Create {"and": [true, true]}
        let true_token1 = Token::literal(DataValue::Bool(true));
        let true_ref1 = arena.alloc(true_token1);

        let true_token2 = Token::literal(DataValue::Bool(true));
        let true_ref2 = arena.alloc(true_token2);

        let and_args = vec![true_ref1, true_ref2];
        let and_array_token = Token::ArrayLiteral(and_args);
        let and_array_ref = arena.alloc(and_array_token);

        let and_token = Token::operator(OperatorType::Control(ControlOp::And), and_array_ref);
        let and_ref = arena.alloc(and_token);

        let rule = Logic::new(and_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for true AND false
        // Create {"and": [true, false]}
        let true_token = Token::literal(DataValue::Bool(true));
        let true_ref = arena.alloc(true_token);

        let false_token = Token::literal(DataValue::Bool(false));
        let false_ref = arena.alloc(false_token);

        let and_args = vec![true_ref, false_ref];
        let and_array_token = Token::ArrayLiteral(and_args);
        let and_array_ref = arena.alloc(and_array_token);

        let and_token = Token::operator(OperatorType::Control(ControlOp::And), and_array_ref);
        let and_ref = arena.alloc(and_token);

        let rule = Logic::new(and_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with variables
        // Create {"and": [{"var": "a"}, {"var": "b"}]}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let b_var_token = Token::variable("b", None);
        let b_var_ref = arena.alloc(b_var_token);

        let and_args = vec![a_var_ref, b_var_ref];
        let and_array_token = Token::ArrayLiteral(and_args);
        let and_array_ref = arena.alloc(and_array_token);

        let and_token = Token::operator(OperatorType::Control(ControlOp::And), and_array_ref);
        let and_ref = arena.alloc(and_token);

        let rule = Logic::new(and_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(0));

        // Test with multiple values - should return last truthy value or first falsy
        // Create {"and": [1, 2, 3]}
        let one_token = Token::literal(DataValue::integer(1));
        let one_ref = arena.alloc(one_token);

        let two_token = Token::literal(DataValue::integer(2));
        let two_ref = arena.alloc(two_token);

        let three_token = Token::literal(DataValue::integer(3));
        let three_ref = arena.alloc(three_token);

        let and_args = vec![one_ref, two_ref, three_ref];
        let and_array_token = Token::ArrayLiteral(and_args);
        let and_array_ref = arena.alloc(and_array_token);

        let and_token = Token::operator(OperatorType::Control(ControlOp::And), and_array_ref);
        let and_ref = arena.alloc(and_token);

        let rule = Logic::new(and_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn test_or() {
        // Create DataLogic instance with arena
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for true OR true
        // Create {"or": [true, true]}
        let true_token1 = Token::literal(DataValue::Bool(true));
        let true_ref1 = arena.alloc(true_token1);

        let true_token2 = Token::literal(DataValue::Bool(true));
        let true_ref2 = arena.alloc(true_token2);

        let or_args = vec![true_ref1, true_ref2];
        let or_array_token = Token::ArrayLiteral(or_args);
        let or_array_ref = arena.alloc(or_array_token);

        let or_token = Token::operator(OperatorType::Control(ControlOp::Or), or_array_ref);
        let or_ref = arena.alloc(or_token);

        let rule = Logic::new(or_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for true OR false
        // Create {"or": [true, false]}
        let true_token = Token::literal(DataValue::Bool(true));
        let true_ref = arena.alloc(true_token);

        let false_token = Token::literal(DataValue::Bool(false));
        let false_ref = arena.alloc(false_token);

        let or_args = vec![true_ref, false_ref];
        let or_array_token = Token::ArrayLiteral(or_args);
        let or_array_ref = arena.alloc(or_array_token);

        let or_token = Token::operator(OperatorType::Control(ControlOp::Or), or_array_ref);
        let or_ref = arena.alloc(or_token);

        let rule = Logic::new(or_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for false OR true
        // Create {"or": [false, true]}
        let false_token = Token::literal(DataValue::Bool(false));
        let false_ref = arena.alloc(false_token);

        let true_token = Token::literal(DataValue::Bool(true));
        let true_ref = arena.alloc(true_token);

        let or_args = vec![false_ref, true_ref];
        let or_array_token = Token::ArrayLiteral(or_args);
        let or_array_ref = arena.alloc(or_array_token);

        let or_token = Token::operator(OperatorType::Control(ControlOp::Or), or_array_ref);
        let or_ref = arena.alloc(or_token);

        let rule = Logic::new(or_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for false OR false
        // Create {"or": [false, false]}
        let false_token1 = Token::literal(DataValue::Bool(false));
        let false_ref1 = arena.alloc(false_token1);

        let false_token2 = Token::literal(DataValue::Bool(false));
        let false_ref2 = arena.alloc(false_token2);

        let or_args = vec![false_ref1, false_ref2];
        let or_array_token = Token::ArrayLiteral(or_args);
        let or_array_ref = arena.alloc(or_array_token);

        let or_token = Token::operator(OperatorType::Control(ControlOp::Or), or_array_ref);
        let or_ref = arena.alloc(or_token);

        let rule = Logic::new(or_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with variables
        // Create {"or": [{"var": "a"}, {"var": "b"}]}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let b_var_token = Token::variable("b", None);
        let b_var_ref = arena.alloc(b_var_token);

        let or_args = vec![a_var_ref, b_var_ref];
        let or_array_token = Token::ArrayLiteral(or_args);
        let or_array_ref = arena.alloc(or_array_token);

        let or_token = Token::operator(OperatorType::Control(ControlOp::Or), or_array_ref);
        let or_ref = arena.alloc(or_token);

        let rule = Logic::new(or_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn test_not() {
        // Create DataLogic instance with arena
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for NOT true
        // Create {"!": [true]}
        let true_token = Token::literal(DataValue::Bool(true));
        let true_ref = arena.alloc(true_token);

        let not_args = vec![true_ref];
        let not_array_token = Token::ArrayLiteral(not_args);
        let not_array_ref = arena.alloc(not_array_token);

        let not_token = Token::operator(OperatorType::Control(ControlOp::Not), not_array_ref);
        let not_ref = arena.alloc(not_token);

        let rule = Logic::new(not_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test for NOT false
        // Create {"!": [false]}
        let false_token = Token::literal(DataValue::Bool(false));
        let false_ref = arena.alloc(false_token);

        let not_args = vec![false_ref];
        let not_array_token = Token::ArrayLiteral(not_args);
        let not_array_ref = arena.alloc(not_array_token);

        let not_token = Token::operator(OperatorType::Control(ControlOp::Not), not_array_ref);
        let not_ref = arena.alloc(not_token);

        let rule = Logic::new(not_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test with variables
        // Create {"!": [{"var": "a"}]}
        let a_var_token = Token::variable("a", None);
        let a_var_ref = arena.alloc(a_var_token);

        let not_args = vec![a_var_ref];
        let not_array_token = Token::ArrayLiteral(not_args);
        let not_array_ref = arena.alloc(not_array_token);

        let not_token = Token::operator(OperatorType::Control(ControlOp::Not), not_array_ref);
        let not_ref = arena.alloc(not_token);

        let rule = Logic::new(not_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with falsy variable
        // Create {"!": [{"var": "b"}]}
        let b_var_token = Token::variable("b", None);
        let b_var_ref = arena.alloc(b_var_token);

        let not_args = vec![b_var_ref];
        let not_array_token = Token::ArrayLiteral(not_args);
        let not_array_ref = arena.alloc(not_array_token);

        let not_token = Token::operator(OperatorType::Control(ControlOp::Not), not_array_ref);
        let not_ref = arena.alloc(not_token);

        let rule = Logic::new(not_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert_eq!(result, json!(true));
    }
}
