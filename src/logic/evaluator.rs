//! Evaluator for logic expressions.
//!
//! This module provides functions for evaluating logic expressions.

use super::error::Result;
use super::operators::{
    arithmetic, array, comparison, control, datetime, missing, string, throw, r#try, val, variable,
};
use super::token::{OperatorType, Token};
use crate::arena::DataArena;
use crate::value::DataValue;

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
        _ => arena.alloc_slice_copy(&[args]),
    }
}

/// Evaluates a logic expression.
#[inline]
pub fn evaluate<'a>(token: &'a Token<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for literals - most common case
    if let Token::Literal(value) = token {
        return Ok(value);
    }

    // Fast path for variables - second most common case
    if let Token::Variable { path, default } = token {
        return variable::evaluate_variable(path, default, arena);
    }

    // Handle other token types
    match token {
        // Already handled above
        Token::Literal(_) | Token::Variable { .. } => unreachable!(),

        // Dynamic variables evaluate the path expression first
        Token::DynamicVariable { path_expr, default } => {
            // Evaluate the path expression
            let path_value = evaluate(path_expr, arena)?;

            // Convert the path value to a string
            let path_str = match path_value {
                // Fast path for strings - no allocation needed
                DataValue::String(s) => s,

                // For null, use the preallocated empty string
                DataValue::Null => arena.empty_string(),

                // For other types, convert to string
                DataValue::Number(n) => arena.alloc_str(&n.to_string()),
                DataValue::Bool(b) => {
                    if *b {
                        "true"
                    } else {
                        "false"
                    }
                }
                _ => {
                    return Err(super::error::LogicError::VariableError {
                        path: format!("{:?}", path_value),
                        reason: format!(
                            "Dynamic variable path must evaluate to a scalar value, got: {:?}",
                            path_value
                        ),
                    });
                }
            };

            // Evaluate the variable with the computed path
            variable::evaluate_variable(path_str, default, arena)
        }

        // Array literals evaluate each element
        Token::ArrayLiteral(items) => {
            // Get a vector from the arena's pool
            let mut values = arena.get_data_value_vec_with_capacity(items.len());

            // Evaluate each item in the array
            for item in items {
                let value = evaluate(item, arena)?;
                values.push(value.clone());
            }

            // Create the array DataValue and allocate it
            let array_slice = arena.bump_vec_into_slice(values);
            let result = DataValue::Array(array_slice);
            Ok(arena.alloc(result))
        }

        // Operators apply a function to their arguments
        Token::Operator { op_type, args } => evaluate_operator(*op_type, args, arena),

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
            evaluate_custom_operator(name, tokens_refs, arena)
        }
    }
}

/// Evaluates a custom operator application.
fn evaluate_custom_operator<'a>(
    name: &'a str,
    _args: &'a [&'a Token<'a>],
    _arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    Err(super::error::LogicError::OperatorNotFoundError {
        operator: name.to_string(),
    })
}

/// Evaluates arguments and returns them as a slice of DataValues
/// This function is optimized to avoid unnecessary allocations
#[inline]
fn evaluate_arguments<'a>(
    args: &'a Token<'a>,
    arena: &'a DataArena,
) -> Result<&'a [DataValue<'a>]> {
    match args {
        // Fast path for array literals - evaluate each item
        Token::ArrayLiteral(items) => {
            // Fast path for empty arrays
            if items.is_empty() {
                return Ok(arena.empty_array());
            }

            // Get a vector from the arena's pool
            let mut values = arena.get_data_value_vec_with_capacity(items.len());

            // Evaluate each item in the array
            for item in items {
                let value = evaluate(item, arena)?;
                values.push(value.clone());
            }

            // Create the array slice
            Ok(arena.bump_vec_into_slice(values))
        }

        // For other token types, evaluate to a single value and wrap in a slice
        _ => {
            let value = evaluate(args, arena)?;
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
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Get token references for lazy evaluation
    let token_refs = convert_to_token_refs(args, arena);

    match op_type {
        // Comparison operators
        OperatorType::Comparison(comp_op) => match comp_op {
            comparison::ComparisonOp::Equal => comparison::eval_equal(token_refs, arena),
            comparison::ComparisonOp::StrictEqual => {
                comparison::eval_strict_equal(token_refs, arena)
            }
            comparison::ComparisonOp::NotEqual => comparison::eval_not_equal(token_refs, arena),
            comparison::ComparisonOp::StrictNotEqual => {
                comparison::eval_strict_not_equal(token_refs, arena)
            }
            comparison::ComparisonOp::GreaterThan => {
                comparison::eval_greater_than(token_refs, arena)
            }
            comparison::ComparisonOp::GreaterThanOrEqual => {
                comparison::eval_greater_than_or_equal(token_refs, arena)
            }
            comparison::ComparisonOp::LessThan => comparison::eval_less_than(token_refs, arena),
            comparison::ComparisonOp::LessThanOrEqual => {
                comparison::eval_less_than_or_equal(token_refs, arena)
            }
        },

        // Array operators
        OperatorType::Array(array_op) => match array_op {
            array::ArrayOp::Map => array::eval_map(token_refs, arena),
            array::ArrayOp::Filter => array::eval_filter(token_refs, arena),
            array::ArrayOp::Reduce => array::eval_reduce(token_refs, arena),
            array::ArrayOp::All => array::eval_all(token_refs, arena),
            array::ArrayOp::Some => array::eval_some(token_refs, arena),
            array::ArrayOp::None => array::eval_none(token_refs, arena),
            array::ArrayOp::Merge => array::eval_merge(token_refs, arena),
            array::ArrayOp::In => array::eval_in(token_refs, arena),
            array::ArrayOp::Length => array::eval_length(token_refs, arena),
            array::ArrayOp::Slice => array::eval_slice(token_refs, arena),
            array::ArrayOp::Sort => array::eval_sort(token_refs, arena),
        },

        // Arithmetic operators
        OperatorType::Arithmetic(arith_op) => {
            // Evaluate arguments once and pass to the appropriate function
            let args_result = evaluate_arguments(args, arena)?;
            match arith_op {
                arithmetic::ArithmeticOp::Add => arithmetic::eval_add(args_result, arena),
                arithmetic::ArithmeticOp::Subtract => arithmetic::eval_sub(args_result, arena),
                arithmetic::ArithmeticOp::Multiply => arithmetic::eval_mul(args_result, arena),
                arithmetic::ArithmeticOp::Divide => arithmetic::eval_div(args_result, arena),
                arithmetic::ArithmeticOp::Modulo => arithmetic::eval_mod(args_result, arena),
                arithmetic::ArithmeticOp::Min => arithmetic::eval_min(args_result),
                arithmetic::ArithmeticOp::Max => arithmetic::eval_max(args_result),
            }
        }

        // Logical operators
        OperatorType::Control(control_op) => match control_op {
            control::ControlOp::If => {
                if !args.is_array_literal() {
                    return Err(super::error::LogicError::InvalidArgumentsError);
                }
                control::eval_if(token_refs, arena)
            }
            control::ControlOp::And => {
                if !args.is_array_literal() {
                    return Err(super::error::LogicError::InvalidArgumentsError);
                }
                control::eval_and(token_refs, arena)
            }
            control::ControlOp::Or => {
                if !args.is_array_literal() {
                    return Err(super::error::LogicError::InvalidArgumentsError);
                }
                control::eval_or(token_refs, arena)
            }
            control::ControlOp::Not => control::eval_not(token_refs, arena),
            control::ControlOp::DoubleNegation => control::eval_double_negation(token_refs, arena),
        },

        // String operators
        OperatorType::String(string_op) => match string_op {
            string::StringOp::Cat => string::eval_cat(token_refs, arena),
            string::StringOp::Substr => string::eval_substr(token_refs, arena),
        },

        OperatorType::Missing => missing::eval_missing(token_refs, arena),

        OperatorType::MissingSome => missing::eval_missing_some(token_refs, arena),

        OperatorType::Exists => {
            let args_result = evaluate_arguments(args, arena)?;
            val::eval_exists(args_result, arena)
        }

        OperatorType::Coalesce => eval_coalesce(token_refs, arena),

        // Throw operator
        OperatorType::Throw => throw::eval_throw(token_refs, arena),

        // Try operator
        OperatorType::Try => r#try::eval_try(token_refs, arena),

        // Val operator
        OperatorType::Val => val::eval_val(token_refs, arena),

        // Array literal operator
        OperatorType::ArrayLiteral => {
            // Just evaluate all elements as an array
            let mut values = arena.get_data_value_vec();

            for token in token_refs {
                let value = evaluate(token, arena)?;
                values.push(value.clone());
            }

            let array_slice = arena.bump_vec_into_slice(values);
            let result = DataValue::Array(array_slice);
            Ok(arena.alloc(result))
        }

        // DateTime operators
        OperatorType::DateTime(datetime_op) => {
            // Evaluate arguments once and pass to the appropriate function
            let args_result = evaluate_arguments(args, arena)?;
            match datetime_op {
                datetime::DateTimeOp::DateTime => {
                    datetime::eval_datetime_operator(args_result, arena)
                }
                datetime::DateTimeOp::Timestamp => {
                    datetime::eval_timestamp_operator(args_result, arena)
                }
                datetime::DateTimeOp::Now => datetime::eval_now(arena),
                datetime::DateTimeOp::ParseDate => datetime::eval_parse_date(args_result, arena),
                datetime::DateTimeOp::FormatDate => datetime::eval_format_date(args_result, arena),
                datetime::DateTimeOp::DateDiff => datetime::eval_date_diff(args_result, arena),
            }
        }
    }
}

/// Evaluates a coalesce operation, which returns the first non-null value.
fn eval_coalesce<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // If no arguments, return null
    if args.is_empty() {
        return Ok(arena.null_value());
    }

    // Return the first non-null value
    for arg in args {
        let value = evaluate(arg, arena)?;

        // Check if the value is null
        if !value.is_null() {
            return Ok(value);
        }
    }

    // If all values are null, return null
    Ok(arena.null_value())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::RuleBuilder;
    use crate::builder::RuleFactory;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_evaluate_literal() {
        let arena = DataArena::new();
        let builder = RuleBuilder::new(&arena);

        // Null
        let token = builder.null();
        let result = evaluate(token.root(), &arena).unwrap();
        assert!(result.is_null());

        // Boolean
        let token = builder.bool(true);
        let result = evaluate(token.root(), &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn test_evaluate_comparison() {
        let arena = DataArena::new();
        let data_json = json!({"foo": 42, "bar": "hello"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        arena.set_current_context(&data, &DataValue::String("$"));
        arena.set_root_context(&data);
        let builder = RuleBuilder::new(&arena);

        // Equal
        let token = builder.compare().equal_op().var("foo").int(42).build();
        let result = evaluate(token.root(), &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn test_evaluate_coalesce() {
        let arena = DataArena::new();
        let data_json = json!({"person": {"name": "John"}, "name": "Jane"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        arena.set_current_context(&data, &DataValue::String("$"));
        arena.set_root_context(&data);
        let factory = RuleFactory::new(&arena);

        // Simple coalesce with one value
        let token = factory.coalesce(vec!["name"]);
        let result = evaluate(token.root(), &arena).unwrap();
        assert_eq!(result.as_str(), Some("Jane"));
    }

    #[test]
    fn test_evaluate_val() {
        use super::evaluate;
        use crate::arena::DataArena;
        use crate::logic::token::{OperatorType, Token};
        use crate::value::DataValue;

        let arena = DataArena::new();

        // Create test data: { "hello": 0, "nested": { "world": 1 } }
        let world_entries =
            arena.vec_into_slice(vec![(arena.intern_str("world"), DataValue::integer(1))]);
        let nested_obj = DataValue::Object(world_entries);

        let entries = arena.vec_into_slice(vec![
            (arena.intern_str("hello"), DataValue::integer(0)),
            (arena.intern_str("nested"), nested_obj),
        ]);
        let data = DataValue::Object(entries);
        arena.set_current_context(&data, &DataValue::String("$"));
        arena.set_root_context(&data);

        // Test simple val: { "val": "hello" }
        let val_arg = Token::literal(DataValue::string(&arena, "hello"));
        let val_token = Token::operator(OperatorType::Val, arena.alloc(val_arg));

        let result = evaluate(arena.alloc(val_token), &arena).unwrap();
        assert_eq!(*result, DataValue::integer(0));

        // Test nested val: { "val": ["nested", "world"] }
        let nested_args = arena.vec_into_slice(vec![
            DataValue::string(&arena, "nested"),
            DataValue::string(&arena, "world"),
        ]);
        let nested_array = DataValue::Array(nested_args);
        let nested_val_arg = Token::literal(nested_array);
        let nested_val_token = Token::operator(OperatorType::Val, arena.alloc(nested_val_arg));

        let result = evaluate(arena.alloc(nested_val_token), &arena).unwrap();
        assert_eq!(*result, DataValue::integer(1));

        // Test val with empty array (should return the entire data)
        let empty_array = DataValue::Array(arena.vec_into_slice(vec![]));
        let empty_val_arg = Token::literal(empty_array);
        let empty_val_token = Token::operator(OperatorType::Val, arena.alloc(empty_val_arg));

        let result = evaluate(arena.alloc(empty_val_token), &arena).unwrap();
        assert_eq!(*result, data);
    }

    #[test]
    fn test_evaluate_datetime() {
        use super::evaluate;
        use crate::arena::DataArena;
        use crate::logic::operators::DateTimeOp;
        use crate::logic::token::{OperatorType, Token};
        use crate::value::DataValue;

        let arena = DataArena::new();

        // Test simple datetime conversion: { "datetime": "2022-07-06T13:20:06Z" }
        let dt_arg = Token::literal(DataValue::string(&arena, "2022-07-06T13:20:06Z"));
        let dt_token = Token::operator(
            OperatorType::DateTime(DateTimeOp::DateTime),
            arena.alloc(dt_arg),
        );

        let result = evaluate(arena.alloc(dt_token), &arena).unwrap();

        // Check if it's an object with a datetime key
        if let DataValue::Object(entries) = result {
            let datetime_entry = entries
                .iter()
                .find(|(key, _)| *key == arena.intern_str("datetime"));
            assert!(datetime_entry.is_some());

            let (_, dt_val) = datetime_entry.unwrap();
            assert!(dt_val.is_datetime());

            // Now proceed with format test using the datetime from the object
            // Since we can't easily compare the exact datetime directly, verify that
            // it converts back to the expected string format using format_date
            let format_arg = arena.vec_into_slice(vec![
                result.clone(),
                DataValue::string(&arena, "yyyy-MM-ddTHH:mm:ssZ"),
            ]);
            let format_array = DataValue::Array(format_arg);
            let format_token = Token::operator(
                OperatorType::DateTime(DateTimeOp::FormatDate),
                arena.alloc(Token::literal(format_array)),
            );

            let formatted = evaluate(arena.alloc(format_token), &arena).unwrap();
            assert_eq!(formatted.as_str().unwrap(), "2022-07-06T13:20:06Z");
        } else {
            panic!("Expected object but got: {:?}", result);
        }
    }
}
