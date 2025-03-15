//! Evaluator for logic expressions.
//!
//! This module provides functions for evaluating logic expressions.

use crate::arena::DataArena;
use crate::value::DataValue;
use super::token::{Token, OperatorType};
use super::error::Result;
use super::operators::{comparison, arithmetic, logical, string, missing, array, conditional, log, r#in, variable};

/// Helper function to convert a token to a TokenRefs wrapper
/// This avoids cloning tokens for lazy evaluation
#[inline]
fn convert_to_token_refs<'a>(args: &'a Token<'a>, arena: &'a DataArena) -> &'a [&'a Token<'a>] {
    match args {
        // Fast path for ArrayLiteral with 0 items
        Token::ArrayLiteral(items) if items.is_empty() => &[],
        // For ArrayLiteral with items, just use the references directly
        Token::ArrayLiteral(items) => items.as_slice(),
        // Fast path for the single argument case
        _ => arena.alloc_slice_copy(&[args])
    }
}

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
        // Already handled above
        Token::Literal(_) | Token::Variable { .. } => unreachable!(),
        
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
            Err(super::error::LogicError::InvalidArgumentsError)
        }
    }
}

/// Evaluates arguments and returns them as a slice of DataValues
/// This function is optimized to avoid unnecessary allocations
#[inline]
fn evaluate_arguments<'a>(args: &'a Token<'a>, data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a [DataValue<'a>]> {
    match args {
        // Fast path for array literals - evaluate each item
        Token::ArrayLiteral(items) => {
            // Fast path for empty arrays
            if items.is_empty() {
                return Ok(arena.empty_array());
            }
            
            // Get a vector from the arena's pool
            let mut values = arena.get_data_value_vec();
            values.reserve(items.len());
            
            // Evaluate each item in the array
            for item in items {
                let value = evaluate(item, data, arena)?;
                values.push(value.clone());
            }
            
            // Create the array slice
            let result = arena.alloc_data_value_slice(&values);
            
            // Return the vector to the pool
            arena.release_data_value_vec(values);
            
            // Return the array slice
            Ok(result)
        },
        
        // For other token types, evaluate to a single value and wrap in a slice
        _ => {
            let value = evaluate(args, data, arena)?;
            match value {
                // If the result is already an array, use it directly
                DataValue::Array(items) => Ok(items),
                
                // For single values, create a one-element slice
                _ => {
                    // For single values, use a more efficient allocation method
                    let slice = arena.alloc_slice_fill_with(1, |_| value.clone());
                    Ok(slice)
                }
            }
        }
    }
}

/// Helper function to evaluate an operator with a token argument
#[inline]
fn evaluate_operator<'a>(
    op_type: OperatorType,
    args: &'a Token<'a>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Get token references for lazy evaluation
    let token_refs = convert_to_token_refs(args, arena);
    
    match op_type {
        // Comparison operators
        OperatorType::Comparison(comp_op) => {
            match comp_op {
                comparison::ComparisonOp::Equal => comparison::eval_equal(token_refs, data, arena),
                comparison::ComparisonOp::StrictEqual => comparison::eval_strict_equal(token_refs, data, arena),
                comparison::ComparisonOp::NotEqual => comparison::eval_not_equal(token_refs, data, arena),
                comparison::ComparisonOp::StrictNotEqual => comparison::eval_strict_not_equal(token_refs, data, arena),
                comparison::ComparisonOp::GreaterThan => comparison::eval_greater_than(token_refs, data, arena),
                comparison::ComparisonOp::GreaterThanOrEqual => comparison::eval_greater_than_or_equal(token_refs, data, arena),
                comparison::ComparisonOp::LessThan => comparison::eval_less_than(token_refs, data, arena),
                comparison::ComparisonOp::LessThanOrEqual => comparison::eval_less_than_or_equal(token_refs, data, arena),
            }
        },
        
        // Array operators
        OperatorType::Array(array_op) => {
            match array_op {
                array::ArrayOp::Map => array::eval_map(token_refs, data, arena),
                array::ArrayOp::Filter => array::eval_filter(token_refs, data, arena),
                array::ArrayOp::Reduce => array::eval_reduce(token_refs, data, arena),
                array::ArrayOp::All => array::eval_all(token_refs, data, arena),
                array::ArrayOp::Some => array::eval_some(token_refs, data, arena),
                array::ArrayOp::None => array::eval_none(token_refs, data, arena),
                array::ArrayOp::Merge => array::eval_merge(token_refs, data, arena),
            }
        },
        
        // Arithmetic operators
        OperatorType::Arithmetic(arith_op) => {
            // Evaluate arguments once and pass to the appropriate function
            let args_result = evaluate_arguments(args, data, arena)?;
            match arith_op {
                arithmetic::ArithmeticOp::Add => arithmetic::eval_add(args_result, arena),
                arithmetic::ArithmeticOp::Subtract => arithmetic::eval_sub(args_result, arena),
                arithmetic::ArithmeticOp::Multiply => arithmetic::eval_mul(args_result, arena),
                arithmetic::ArithmeticOp::Divide => arithmetic::eval_div(args_result, arena),
                arithmetic::ArithmeticOp::Modulo => arithmetic::eval_mod(args_result, arena),
                arithmetic::ArithmeticOp::Min => arithmetic::eval_min(args_result),
                arithmetic::ArithmeticOp::Max => arithmetic::eval_max(args_result),
            }
        },
        
        // Logical operators
        OperatorType::Logical(logic_op) => {
            match logic_op {
                logical::LogicalOp::And => logical::eval_and(token_refs, data, arena),
                logical::LogicalOp::Or => logical::eval_or(token_refs, data, arena),
                logical::LogicalOp::Not => logical::eval_not(token_refs, data, arena),
                logical::LogicalOp::DoubleNegation => logical::eval_double_negation(token_refs, data, arena),
            }
        },
        
        // Conditional operators
        OperatorType::Conditional(cond_op) => {
            match cond_op {
                conditional::ConditionalOp::If => conditional::eval_if(token_refs, data, arena),
                conditional::ConditionalOp::Ternary => conditional::eval_ternary(token_refs, data, arena),
            }
        },
        
        // String operators
        OperatorType::String(string_op) => {
            match string_op {
                string::StringOp::Cat => string::eval_cat(token_refs, data, arena),
                string::StringOp::Substr => string::eval_substr(token_refs, data, arena),
            }
        },
        
        // Other operators
        OperatorType::Log => {
            log::eval_log(token_refs, data, arena)
        },
        
        OperatorType::In => {
            r#in::eval_in(token_refs, data, arena)
        },
        
        OperatorType::Missing => {
            missing::eval_missing(token_refs, data, arena)
        },
        
        OperatorType::MissingSome => {
            missing::eval_missing_some(token_refs, data, arena)
        },
        
        OperatorType::ArrayLiteral => {
            // This should be handled by the Token::ArrayLiteral case in evaluate()
            Err(super::error::LogicError::InvalidArgumentsError)
        }
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