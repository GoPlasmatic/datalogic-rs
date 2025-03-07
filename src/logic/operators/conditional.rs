//! Conditional operator implementations.
//!
//! This module provides implementations for conditional operators such as if and ternary.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Enumeration of conditional operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalOp {
    /// If operator
    If,
    /// Ternary operator
    Ternary,
}

/// Evaluates an if operation.
pub fn eval_if<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // If no arguments, return null
    if args.is_empty() {
        return Ok(DataValue::null());
    }
    
    // If only one argument, evaluate it and return the result
    if args.len() == 1 {
        return evaluate(&args[0], data, arena);
    }
    
    // If only two arguments (condition and result), evaluate condition
    // and return result if truthy, otherwise return null
    if args.len() == 2 {
        let condition = evaluate(&args[0], data, arena)?;
        if condition.coerce_to_bool() {
            return evaluate(&args[1], data, arena);
        }
        return Ok(DataValue::null());
    }
    
    // Process condition-result pairs
    let mut i = 0;
    while i < args.len() - 1 {
        // Evaluate the condition
        let condition = evaluate(&args[i], data, arena)?;
        
        // If the condition is truthy, return the result
        if condition.coerce_to_bool() {
            return evaluate(&args[i + 1], data, arena);
        }
        
        // Move to the next condition-result pair
        i += 2;
    }
    
    // If we have an odd number of arguments, the last one is the default result
    if args.len() % 2 == 1 {
        return evaluate(&args[args.len() - 1], data, arena);
    }
    
    // If no condition was truthy and there's no default, return null
    Ok(DataValue::null())
}

/// Evaluates a ternary operation (?:).
pub fn eval_ternary<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Check that we have exactly 3 arguments
    if args.len() != 3 {
        return Err(LogicError::OperatorError {
            operator: "?:".to_string(),
            reason: format!("Expected 3 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the condition
    let condition = evaluate(&args[0], data, arena)?;
    
    // If the condition is truthy, return the first result, otherwise return the second
    if condition.coerce_to_bool() {
        evaluate(&args[1], data, arena)
    } else {
        evaluate(&args[2], data, arena)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_evaluate_if() {
        let arena = DataArena::new();
        let data_json = json!({
            "temp": 75
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Parse and evaluate an if expression
        let token = parse_str(r#"{
            "if": [
                {">": [{"var": "temp"}, 80]},
                "hot",
                {"<": [{"var": "temp"}, 70]},
                "cold",
                "pleasant"
            ]
        }"#, &arena).unwrap();
        
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("pleasant"));
    }

    #[test]
    fn test_evaluate_ternary() {
        let arena = DataArena::new();
        let data_json = json!({
            "age": 25
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Create tokens for the ternary operation
        let condition = parse_str(r#"{">": [{"var": "age"}, 21]}"#, &arena).unwrap();
        let true_result = parse_str(r#""adult""#, &arena).unwrap();
        let false_result = parse_str(r#""minor""#, &arena).unwrap();
        
        // Manually evaluate the ternary operation
        let condition_value = evaluate(&condition, &data, &arena).unwrap();
        let result = if condition_value.coerce_to_bool() {
            evaluate(&true_result, &data, &arena).unwrap()
        } else {
            evaluate(&false_result, &data, &arena).unwrap()
        };
        
        assert_eq!(result.as_str(), Some("adult"));
    }
} 