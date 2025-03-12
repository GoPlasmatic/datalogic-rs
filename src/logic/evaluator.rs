//! Evaluator for logic expressions.
//!
//! This module provides functions for evaluating logic expressions.

use crate::arena::DataArena;
use crate::value::DataValue;
use super::token::{Token, OperatorType};
use super::error::Result;
use super::operators::{comparison, arithmetic, logical, string, missing, array, conditional, log, r#in, variable};

/// Evaluates a logic expression.
#[inline]
pub fn evaluate<'a>(
    token: &'a Token<'a>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for literals - most common case
    if let Token::Literal(value) = token {
        return Ok(value);
    }
    
    // Fast path for variables - second most common case
    if let Token::Variable { path, default } = token {
        return variable::evaluate_variable(path, default, data, arena);
    }
    
    // Handle other token types
    match token {
        // Dynamic variables evaluate the path expression first
        Token::DynamicVariable { path_expr, default } => {
            // Evaluate the path expression
            let path_value = evaluate(path_expr, data, arena)?;
            
            // Convert the path value to a string
            let path_str = match path_value {
                // Fast path for strings - no allocation needed
                DataValue::String(s) => s,
                
                // For null, use the preallocated empty string
                DataValue::Null => arena.empty_string(),
                
                // For other types, convert to string
                DataValue::Number(n) => arena.alloc_str(&n.to_string()),
                DataValue::Bool(b) => if *b { "true" } else { "false" },
                _ => return Err(super::error::LogicError::VariableError {
                    path: format!("{:?}", path_value),
                    reason: format!("Dynamic variable path must evaluate to a scalar value, got: {:?}", path_value),
                }),
            };
            
            // Evaluate the variable with the computed path
            variable::evaluate_variable(path_str, default, data, arena)
        },
        
        // Array literals evaluate each element
        Token::ArrayLiteral(items) => {
            // Get a vector from the arena's pool
            let mut values = arena.get_data_value_vec();
            
            // Evaluate each item in the array
            for item in items {
                let value = evaluate(item, data, arena)?;
                values.push(value.clone());
            }
            
            // Create the array DataValue
            let result = DataValue::Array(arena.alloc_slice_clone(&values));
            
            // Return the vector to the pool
            arena.release_data_value_vec(values);
            
            // Return the array DataValue
            Ok(arena.alloc(result))
        },
        
        // Operators apply a function to their arguments
        Token::Operator { op_type, args } => {
            evaluate_operator(*op_type, args, data, arena)
        },
        
        // Custom operators are looked up in a registry
        Token::CustomOperator { name, args } => {
            // Convert args to a vector of token references
            let tokens_refs = if let Token::ArrayLiteral(items) = args {
                // For ArrayLiteral, we can use the items directly
                items.as_slice()
            } else {
                // For single argument (not in an ArrayLiteral), create a one-element slice
                std::slice::from_ref(args)
            };
            
            // Pass the tokens to the custom operator evaluator
            evaluate_custom_operator(name, tokens_refs, data, arena)
        },

        // Already handled above
        Token::Literal(_) => unreachable!(),
        Token::Variable { .. } => unreachable!(),
    }
}

/// Evaluates a custom operator application.
fn evaluate_custom_operator<'a>(
    name: &'a str,
    _args: &'a [&'a Token<'a>],
    _data: &'a DataValue<'a>,
    _arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match name {
        _ => {
            // Custom operators are not yet implemented
            Err(super::error::LogicError::OperatorError {
                operator: format!("Custom operator '{}'", name),
                reason: "Custom operators are not yet supported".to_string(),
            })
        }
    }
}

fn evaluate_arguments<'a>(
    args: &'a Token<'a>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> &'a [DataValue<'a>] {
    let arg = evaluate(args, data, arena).unwrap();
    let args = match arg {
        DataValue::Array(items) => items,
        _ => arena.alloc_slice_clone(&[arg.clone()]),
    };
    return args;
}

/// Helper function to convert a token to a slice of tokens
fn convert_to_token_slice<'a>(args: &'a Token<'a>, arena: &'a DataArena) -> &'a [Token<'a>] {
    match args {
        // Fast path for ArrayLiteral with 0 items
        Token::ArrayLiteral(items) if items.is_empty() => {
            &[]
        },
        // For ArrayLiteral with items, clone the tokens
        Token::ArrayLiteral(items) => {
            // Pre-allocate capacity to avoid reallocations
            let mut token_vec = Vec::with_capacity(items.len());
            
            // Clone the tokens efficiently - collect them all at once
            token_vec.extend(items.iter().map(|&item| (*item).clone()));
            
            arena.alloc_slice_clone(&token_vec)
        },
        // Fast path for the single argument case
        _ => {
            arena.alloc_slice_clone(&[args.clone()])
        }
    }
}

/// Helper function to evaluate an operator with a token argument
fn evaluate_operator<'a>(
    op_type: OperatorType,
    args: &'a Token<'a>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match op_type {
        // Comparison operators
        OperatorType::Comparison(comp_op) => {
            let tokens = convert_to_token_slice(args, arena);
            match comp_op {
                comparison::ComparisonOp::Equal => comparison::eval_equal(tokens, data, arena),
                comparison::ComparisonOp::StrictEqual => comparison::eval_strict_equal(tokens, data, arena),
                comparison::ComparisonOp::NotEqual => comparison::eval_not_equal(tokens, data, arena),
                comparison::ComparisonOp::StrictNotEqual => comparison::eval_strict_not_equal(tokens, data, arena),
                comparison::ComparisonOp::GreaterThan => comparison::eval_greater_than(tokens, data, arena),
                comparison::ComparisonOp::GreaterThanOrEqual => comparison::eval_greater_than_or_equal(tokens, data, arena),
                comparison::ComparisonOp::LessThan => comparison::eval_less_than(tokens, data, arena),
                comparison::ComparisonOp::LessThanOrEqual => comparison::eval_less_than_or_equal(tokens, data, arena),
            }
        },
        
        // Array operators
        OperatorType::Array(array_op) => {
            let tokens = convert_to_token_slice(args, arena);
            match array_op {
                array::ArrayOp::Map => array::eval_map(tokens, data, arena),
                array::ArrayOp::Filter => array::eval_filter(tokens, data, arena),
                array::ArrayOp::Reduce => array::eval_reduce(tokens, data, arena),
                array::ArrayOp::All => array::eval_all(tokens, data, arena),
                array::ArrayOp::Some => array::eval_some(tokens, data, arena),
                array::ArrayOp::None => array::eval_none(tokens, data, arena),
                array::ArrayOp::Merge => array::eval_merge(tokens, data, arena),
            }
        },
        
        // Arithmetic operators
        OperatorType::Arithmetic(arith_op) => match arith_op {
            arithmetic::ArithmeticOp::Add => arithmetic::eval_add(evaluate_arguments(args, data, arena), arena),
            arithmetic::ArithmeticOp::Subtract => arithmetic::eval_sub(evaluate_arguments(args, data, arena), arena),
            arithmetic::ArithmeticOp::Multiply => arithmetic::eval_mul(evaluate_arguments(args, data, arena), arena),
            arithmetic::ArithmeticOp::Divide => arithmetic::eval_div(evaluate_arguments(args, data, arena), arena),
            arithmetic::ArithmeticOp::Modulo => arithmetic::eval_mod(evaluate_arguments(args, data, arena), arena),
            arithmetic::ArithmeticOp::Min => arithmetic::eval_min(evaluate_arguments(args, data, arena)),
            arithmetic::ArithmeticOp::Max => arithmetic::eval_max(evaluate_arguments(args, data, arena)),
        },
        
        // Logical operators
        OperatorType::Logical(logic_op) => {
            let tokens = convert_to_token_slice(args, arena);
            match logic_op {
                logical::LogicalOp::And => logical::eval_and(tokens, data, arena),
                logical::LogicalOp::Or => logical::eval_or(tokens, data, arena),
                logical::LogicalOp::Not => logical::eval_not(tokens, data, arena),
                logical::LogicalOp::DoubleNegation => logical::eval_double_negation(tokens, data, arena),
            }
        },
        
        // Conditional operators
        OperatorType::Conditional(cond_op) => {
            let tokens = convert_to_token_slice(args, arena);
            match cond_op {
                conditional::ConditionalOp::If => conditional::eval_if(tokens, data, arena),
                conditional::ConditionalOp::Ternary => conditional::eval_ternary(tokens, data, arena),
            }
        },
        
        // String operators
        OperatorType::String(string_op) => {
            let tokens = convert_to_token_slice(args, arena);
            match string_op {
                string::StringOp::Cat => string::eval_cat(tokens, data, arena),
                string::StringOp::Substr => string::eval_substr(tokens, data, arena),
            }
        },
        
        // Other operators
        OperatorType::Log => {
            let tokens = convert_to_token_slice(args, arena);
            log::eval_log(tokens, data, arena)
        },
        
        OperatorType::In => {
            let tokens = convert_to_token_slice(args, arena);
            r#in::eval_in(tokens, data, arena)
        },
        
        OperatorType::Missing => {
            let tokens = convert_to_token_slice(args, arena);
            missing::eval_missing(tokens, data, arena)
        },
        
        OperatorType::MissingSome => {
            let tokens = convert_to_token_slice(args, arena);
            missing::eval_missing_some(tokens, data, arena)
        },
        
        OperatorType::ArrayLiteral => {
            // This should be handled by the Token::ArrayLiteral case in evaluate()
            Err(super::error::LogicError::OperatorError {
                operator: "ArrayLiteral".to_string(),
                reason: "ArrayLiteral operator should be handled by Token::ArrayLiteral".to_string(),
            })
        },
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
        let null = DataValue::null();
        
        // Null
        let token = parse_str("null", &arena).unwrap();
        let result = evaluate(token, &null, &arena).unwrap();
        assert!(result.is_null());
        
        // Boolean
        let token = parse_str("true", &arena).unwrap();
        let result = evaluate(token, &null, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }
    
    #[test]
    fn test_evaluate_comparison() {
        let arena = DataArena::new();
        let data_json = json!({"foo": 42, "bar": "hello"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        
        // Equal
        let token = parse_str(r#"{"==": [{"var": "foo"}, 42]}"#, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }
}