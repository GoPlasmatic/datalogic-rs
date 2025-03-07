//! Evaluator for logic expressions.
//!
//! This module provides functions for evaluating logic expressions.

use crate::arena::DataArena;
use crate::value::DataValue;
use super::token::{Token, OperatorType};
use super::error::Result;
use super::operators::{comparison, arithmetic, logical, string, missing, array, conditional, log, r#in, variable};

/// Evaluates a logic expression.
pub fn evaluate<'a>(
    token: &'a Token<'a>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    match token {
        // Literals are returned as-is
        Token::Literal(value) => Ok(value.clone()),
        
        // Variables are looked up in the data
        Token::Variable { path, default } => {
            variable::evaluate_variable(path, default, data, arena)
        },
        
        // Dynamic variables evaluate the path expression first
        Token::DynamicVariable { path_expr, default } => {
            // Evaluate the path expression
            let path_value = evaluate(path_expr, data, arena)?;
            
            // Convert the path value to a string
            let path_str = match path_value {
                DataValue::String(s) => s,
                DataValue::Number(n) => arena.alloc_str(&n.to_string()),
                DataValue::Bool(b) => arena.alloc_str(&b.to_string()),
                DataValue::Null => arena.alloc_str(""),
                _ => return Err(super::error::LogicError::VariableError {
                    path: format!("{:?}", path_value),
                    reason: format!("Dynamic variable path must evaluate to a scalar value, got: {:?}", path_value),
                }),
            };
            
            // Evaluate the variable with the computed path
            variable::evaluate_variable(path_str, default, data, arena)
        },
        
        // Operators apply a function to their arguments
        Token::Operator { op_type, args } => {
            evaluate_operator(*op_type, args, data, arena)
        },
        
        // Custom operators are looked up in a registry
        Token::CustomOperator { name, args } => {
            evaluate_custom_operator(name, args, data, arena)
        },
    }
}

/// Evaluates an operator application.
fn evaluate_operator<'a>(
    op_type: OperatorType,
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    match op_type {
        // Comparison operators
        OperatorType::Comparison(comp_op) => match comp_op {
            comparison::ComparisonOp::Equal => comparison::eval_equal(args, data, arena),
            comparison::ComparisonOp::StrictEqual => comparison::eval_strict_equal(args, data, arena),
            comparison::ComparisonOp::NotEqual => comparison::eval_not_equal(args, data, arena),
            comparison::ComparisonOp::StrictNotEqual => comparison::eval_strict_not_equal(args, data, arena),
            comparison::ComparisonOp::GreaterThan => comparison::eval_greater_than(args, data, arena),
            comparison::ComparisonOp::GreaterThanOrEqual => comparison::eval_greater_than_or_equal(args, data, arena),
            comparison::ComparisonOp::LessThan => comparison::eval_less_than(args, data, arena),
            comparison::ComparisonOp::LessThanOrEqual => comparison::eval_less_than_or_equal(args, data, arena),
        },
        
        // Arithmetic operators
        OperatorType::Arithmetic(arith_op) => match arith_op {
            arithmetic::ArithmeticOp::Add => arithmetic::eval_add(args, data, arena),
            arithmetic::ArithmeticOp::Subtract => arithmetic::eval_subtract(args, data, arena),
            arithmetic::ArithmeticOp::Multiply => arithmetic::eval_multiply(args, data, arena),
            arithmetic::ArithmeticOp::Divide => arithmetic::eval_divide(args, data, arena),
            arithmetic::ArithmeticOp::Modulo => arithmetic::eval_modulo(args, data, arena),
            arithmetic::ArithmeticOp::Min => arithmetic::eval_min(args, data, arena),
            arithmetic::ArithmeticOp::Max => arithmetic::eval_max(args, data, arena),
        },
        
        // Logical operators
        OperatorType::Logical(logic_op) => match logic_op {
            logical::LogicalOp::And => logical::eval_and(args, data, arena),
            logical::LogicalOp::Or => logical::eval_or(args, data, arena),
            logical::LogicalOp::Not => logical::eval_not(args, data, arena),
        },
        
        // String operators
        OperatorType::String(string_op) => match string_op {
            string::StringOp::Cat => string::eval_cat(args, data, arena),
            string::StringOp::Substr => string::eval_substr(args, data, arena),
        },
        
        // Array operators
        OperatorType::Array(array_op) => match array_op {
            array::ArrayOp::Map => array::eval_map(args, data, arena),
            array::ArrayOp::Filter => array::eval_filter(args, data, arena),
            array::ArrayOp::Reduce => array::eval_reduce(args, data, arena),
            array::ArrayOp::All => array::eval_all(args, data, arena),
            array::ArrayOp::Some => array::eval_some(args, data, arena),
            array::ArrayOp::None => array::eval_none(args, data, arena),
            array::ArrayOp::Merge => array::eval_merge(args, data, arena),
        },
        
        // Conditional operators
        OperatorType::Conditional(cond_op) => match cond_op {
            conditional::ConditionalOp::If => conditional::eval_if(args, data, arena),
            conditional::ConditionalOp::Ternary => conditional::eval_ternary(args, data, arena),
        },
        
        // Special operators
        OperatorType::Log => log::eval_log(args, data, arena),
        OperatorType::In => r#in::eval_in(args, data, arena),
        OperatorType::Missing => missing::eval_missing(args, data, arena),
        OperatorType::MissingSome => missing::eval_missing_some(args, data, arena),
        
        // Array literal operator (evaluates each element and returns an array)
        OperatorType::ArrayLiteral => {
            // Get a pre-allocated vector from the pool
            let mut values = arena.get_data_value_vec();
            
            // Evaluate each element
            for arg in args {
                let value = evaluate(arg, data, arena)?;
                values.push(value);
            }
            
            // Create the array
            let values_slice = arena.alloc_slice_clone(&values);
            
            // Release the vector back to the pool
            arena.release_data_value_vec(values);
            
            Ok(DataValue::Array(values_slice))
        },
    }
}

/// Evaluates a custom operator application.
fn evaluate_custom_operator<'a>(
    name: &str,
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    match name {
        // Double negation operator (!!) - converts a value to boolean
        "!!" => {
            // Check that we have exactly 1 argument
            if args.len() != 1 {
                return Err(super::error::LogicError::OperatorError {
                    operator: "!!".to_string(),
                    reason: format!("Expected 1 argument, got {}", args.len()),
                });
            }
            
            // Evaluate the argument
            let value = evaluate(&args[0], data, arena)?;
            
            // Convert to boolean
            Ok(DataValue::Bool(value.coerce_to_bool()))
        },
        
        // Other custom operators are not yet implemented
        _ => Err(super::error::LogicError::OperatorError {
            operator: name.to_string(),
            reason: "Custom operators are not yet implemented".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;
    
    #[test]
    fn test_evaluate_literal() {
        let arena = DataArena::new();
        let token = Token::literal(DataValue::integer(42));
        let data = DataValue::null();
        
        let result = evaluate(&token, &data, &arena).unwrap();
        assert_eq!(result.as_i64(), Some(42));
    }
    
    #[test]
    fn test_evaluate_comparison() {
        let arena = DataArena::new();
        let data_json = json!({
            "a": 10,
            "b": 20
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Parse and evaluate a comparison
        let token = parse_str(r#"{"<": [{"var": "a"}, {"var": "b"}]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
        
        // Test equality
        let token = parse_str(r#"{"==": [{"var": "a"}, 10]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }
} 