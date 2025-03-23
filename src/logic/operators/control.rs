//! Logical operators for logic expressions.
//!
//! This module provides implementations for logical operators
//! such as and, or, not, etc.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;

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
pub fn eval_if<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.is_empty() {
        return Ok(arena.null_value());
    }
    
    // Process arguments in pairs (condition, value)
    let mut i = 0;
    while i + 1 < args.len() {
        // Evaluate the condition
        let condition = evaluate(args[i], data, arena)?;
        
        // If the condition is true, return the value
        if condition.coerce_to_bool() {
            return evaluate(args[i + 1], data, arena);
        }
        
        // Move to the next pair
        i += 2;
    }
    
    // If there's an odd number of arguments, the last one is the "else" value
    if i < args.len() {
        return evaluate(args[i], data, arena);
    }
    
    // No conditions matched and no else value
    Ok(arena.null_value())
}


/// Evaluates an AND operation.
pub fn eval_and<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Ok(arena.null_value());
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        return evaluate(args[0], data, arena);
    }
    
    // Evaluate each argument with short-circuit evaluation
    let mut last_value = arena.null_value();
    
    for arg in args {
        let value = evaluate(arg, data, arena)?;
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
pub fn eval_or<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for empty arguments
    if args.is_empty() {
        return Ok(arena.false_value());
    }
    
    // Fast path for single argument
    if args.len() == 1 {
        return evaluate(args[0], data, arena);
    }
    
    // Evaluate each argument with short-circuit evaluation
    let mut last_value = arena.false_value();
    
    for arg in args {
        let value = evaluate(arg, data, arena)?;
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
pub fn eval_not<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 1 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let value = evaluate(args[0], data, arena)?;
    Ok(arena.alloc(DataValue::Bool(!value.coerce_to_bool())))
}

/// Evaluates a logical double negation (!!).
pub fn eval_double_negation<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 1 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let value = evaluate(args[0], data, arena)?;
    Ok(arena.alloc(DataValue::Bool(value.coerce_to_bool())))
}

#[cfg(test)]
mod tests {
    use crate::JsonLogic;
    use serde_json::json;

    #[test]
    fn test_and() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for true AND true
        let rule = builder.control()
            .andOp()
            .operand(builder.bool(true))
            .operand(builder.bool(true))
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for true AND false
        let rule = builder.control()
            .andOp()
            .operand(builder.bool(true))
            .operand(builder.bool(false))
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with variables
        let rule = builder.control()
            .andOp()
            .var("a")
            .var("b")
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(0));

        // Test with multiple values - should return last truthy value or first falsy
        let rule = builder.control()
            .andOp()
            .operand(builder.int(1))
            .operand(builder.int(2))
            .operand(builder.int(3))
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn test_or() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for true OR true
        let rule = builder.control()
            .orOp()
            .operand(builder.bool(true))
            .operand(builder.bool(true))
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for true OR false
        let rule = builder.control()
            .orOp()
            .operand(builder.bool(true))
            .operand(builder.bool(false))
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for false OR true
        let rule = builder.control()
            .orOp()
            .operand(builder.bool(false))
            .operand(builder.bool(true))
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test for false OR false
        let rule = builder.control()
            .orOp()
            .operand(builder.bool(false))
            .operand(builder.bool(false))
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with variables
        let rule = builder.control()
            .orOp()
            .var("a")
            .var("b")
            .build();
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn test_not() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let builder = logic.builder();

        let data = json!({
            "a": 1,
            "b": 0
        });

        // Test for NOT true
        let rule = builder.control()
            .notOp(builder.bool(true));
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test for NOT false
        let rule = builder.control()
            .notOp(builder.bool(false));
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(true));

        // Test with variables
        let var_a = builder.var("a").build();
        let rule = builder.control()
            .notOp(var_a);
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(false));

        // Test with falsy variable
        let var_b = builder.var("b").build();
        let rule = builder.control()
            .notOp(var_b);
        let result = logic.apply_logic(&rule, &data).unwrap();
        assert_eq!(result, json!(true));
    }
} 