//! Optimizer for logic expressions.
//!
//! This module provides functions for optimizing logic expressions by
//! precomputing static parts of the expression at compile time.

use super::error::Result;
use super::token::{OperatorType, Token};
use crate::arena::DataArena;
use crate::logic::evaluator::evaluate;
use crate::value::DataValue;

/// Optimizes a token by evaluating static parts of the expression.
pub fn optimize<'a>(token: &'a Token<'a>, arena: &'a DataArena) -> Result<&'a Token<'a>> {
    match token {
        // Literals are already optimized
        Token::Literal(_) => Ok(token),

        // Variables can't be optimized without data
        Token::Variable { .. } => Ok(token),

        // Dynamic variables can't be optimized without data
        Token::DynamicVariable { .. } => Ok(token),

        // For now, just return the original token for array literals
        // This needs to be fixed with a proper lifetime-respecting implementation
        Token::ArrayLiteral(_) => Ok(token),

        // Operators might be optimizable if their arguments are static
        Token::Operator { op_type, args } => {
            // Special case: missing and missing_some operators always need data
            if *op_type == OperatorType::Missing
                || *op_type == OperatorType::MissingSome
                || *op_type == OperatorType::Exists
                || *op_type == OperatorType::Val
            {
                // Just optimize the arguments
                let optimized_args = optimize(args, arena)?;
                return Ok(arena.alloc(Token::operator(*op_type, optimized_args)));
            }

            // Optimize the arguments
            let optimized_args = optimize(args, arena)?;

            // Check if all arguments are literals or static expressions
            let is_static = match optimized_args {
                Token::ArrayLiteral(items) => {
                    items.iter().all(|item| matches!(item, Token::Literal(_)))
                }
                Token::Literal(_) => true,
                _ => false,
            };

            // If all arguments are static, evaluate the expression
            if is_static {
                // Create a dummy data value for evaluation
                let dummy_data = arena.alloc(DataValue::Null);
                arena.set_current_context(dummy_data, &DataValue::String("$"));

                // Create the operator token in the arena
                let op_token = arena.alloc(Token::operator(*op_type, optimized_args));

                // Try to evaluate the expression
                match evaluate(op_token, arena) {
                    Ok(result) => {
                        // Return the result as a literal
                        return Ok(arena.alloc(Token::literal(result.clone())));
                    }
                    Err(_) => {
                        // If evaluation fails, just return the optimized operator
                        return Ok(op_token);
                    }
                }
            }

            // If not all arguments are static, check if we can optimize nested expressions
            if let Token::ArrayLiteral(items) = optimized_args {
                let mut all_optimized_items = Vec::with_capacity(items.len());
                let mut any_changed = false;

                // Try to optimize each item
                for item in items.iter() {
                    if let Token::Operator {
                        op_type: _nested_op_type,
                        args: _nested_args,
                    } = *item
                    {
                        // Recursively optimize the nested operator
                        let optimized_item = optimize(item, arena)?;
                        all_optimized_items.push(optimized_item);

                        // Check if the item was optimized
                        if !std::ptr::eq(optimized_item, *item) {
                            any_changed = true;
                        }
                    } else {
                        // Keep non-operator items as is
                        all_optimized_items.push(*item);
                    }
                }

                // If any items were optimized, create a new array literal
                if any_changed {
                    // Check if all items are literals
                    let all_literals = all_optimized_items
                        .iter()
                        .all(|item| matches!(item, Token::Literal(_)));

                    // Create a new array literal
                    let new_array_literal = Token::ArrayLiteral(all_optimized_items);
                    let new_array_token = arena.alloc(new_array_literal);

                    if all_literals {
                        // Create a dummy data value for evaluation
                        let dummy_data = arena.alloc(DataValue::Null);
                        arena.set_current_context(dummy_data, &DataValue::String("$"));
                        // Create the operator token in the arena
                        let op_token = arena.alloc(Token::operator(*op_type, new_array_token));

                        // Try to evaluate the expression
                        match evaluate(op_token, arena) {
                            Ok(result) => {
                                // Return the result as a literal
                                return Ok(arena.alloc(Token::literal(result.clone())));
                            }
                            Err(_) => {
                                // If evaluation fails, just return the optimized operator
                                return Ok(op_token);
                            }
                        }
                    }

                    return Ok(arena.alloc(Token::operator(*op_type, new_array_token)));
                }
            }

            // If nothing was optimized, just return the optimized operator
            Ok(arena.alloc(Token::operator(*op_type, optimized_args)))
        }

        // Custom operators can't be optimized, but their arguments can
        Token::CustomOperator { name, args } => {
            // Optimize the arguments
            let optimized_args = optimize(args, arena)?;

            // Return the optimized custom operator
            Ok(arena.alloc(Token::custom_operator(name, optimized_args)))
        }

        // Structured objects can optimize their field values
        Token::StructuredObject { fields } => {
            // Optimize each field value
            let mut optimized_fields = Vec::with_capacity(fields.len());
            let mut any_changed = false;

            for (key, value_token) in fields.iter() {
                let optimized_value = optimize(value_token, arena)?;
                optimized_fields.push((*key, optimized_value));

                // Check if this field was optimized
                if !std::ptr::eq(
                    *value_token as *const Token<'_>,
                    optimized_value as *const Token<'_>,
                ) {
                    any_changed = true;
                }
            }

            // If any field was optimized, create a new structured object
            if any_changed {
                let fields_slice = arena.vec_into_slice(optimized_fields);
                Ok(arena.alloc(Token::structured_object(fields_slice)))
            } else {
                // If nothing changed, return the original token
                Ok(token)
            }
        }
    }
}
