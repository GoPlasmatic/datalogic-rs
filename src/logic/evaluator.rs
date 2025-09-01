//! Evaluator for logic expressions.
//!
//! This module provides functions for evaluating logic expressions.

use super::error::{LogicError, Result};
use super::operators::{
    arithmetic, array, comparison, control, datetime, missing, string, throw, r#try, type_op, val,
    variable,
};
use super::token::{OperatorType, Token};
use crate::arena::DataArena;
use crate::context::EvalContext;
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
pub fn evaluate<'a>(
    token: &'a Token<'a>,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match token {
        // Fast path for literals - most common case
        Token::Literal(value) => Ok(value),

        // Fast path for variables - second most common case
        Token::Variable { path, default } => {
            variable::evaluate_variable(path, default, context, arena)
        }

        // Dynamic variables evaluate the path expression first
        Token::DynamicVariable { path_expr, default } => {
            evaluate_dynamic_variable(path_expr, default, context, arena)
        }

        // Array literals evaluate each element
        Token::ArrayLiteral(items) => evaluate_array_literal(items, context, arena),

        // Operators apply a function to their arguments
        Token::Operator { op_type, args } => evaluate_operator(*op_type, args, context, arena),

        // Custom operators are looked up in a registry
        Token::CustomOperator { name, args } => {
            let data_values = evaluate_arguments(args, context, arena)?;
            evaluate_custom_operator(name, data_values, context, arena)
        }

        // Structured objects evaluate each field value while preserving keys
        Token::StructuredObject { fields } => evaluate_structured_object(fields, context, arena),
    }
}

/// Evaluates a dynamic variable access
#[inline]
fn evaluate_dynamic_variable<'a>(
    path_expr: &'a Token<'a>,
    default: &Option<&'a Token<'a>>,
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Evaluate the path expression
    let path_value = evaluate(path_expr, context, arena)?;

    // Convert the path value to a string
    let path_str = convert_to_path_string(path_value, arena)?;

    // Evaluate the variable with the computed path
    variable::evaluate_variable(path_str, default, context, arena)
}

/// Converts a data value to a string for use as a variable path
#[inline]
fn convert_to_path_string<'a>(
    path_value: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a str> {
    match path_value {
        // Fast path for strings - no allocation needed
        DataValue::String(s) => Ok(s),

        // For null, use the preallocated empty string
        DataValue::Null => Ok(arena.empty_string()),

        // For other types, convert to string
        DataValue::Number(n) => Ok(arena.alloc_str(&n.to_string())),
        DataValue::Bool(b) => {
            if *b {
                Ok("true")
            } else {
                Ok("false")
            }
        }
        _ => Err(LogicError::VariableError {
            path: format!("{path_value:?}"),
            reason: format!(
                "Dynamic variable path must evaluate to a scalar value, got: {path_value:?}"
            ),
        }),
    }
}

/// Evaluates an array literal
#[inline]
fn evaluate_array_literal<'a>(
    items: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Get a vector from the arena's pool
    let mut values = arena.get_data_value_vec_with_capacity(items.len());

    // Evaluate each item in the array
    for item in items {
        let value = evaluate(item, context, arena)?;
        values.push(value.clone());
    }

    // Create the array DataValue and allocate it
    let array_slice = arena.bump_vec_into_slice(values);
    let result = DataValue::Array(array_slice);
    Ok(arena.alloc(result))
}

/// Evaluates a custom operator application.
fn evaluate_custom_operator<'a>(
    name: &'a str,
    args: &'a [DataValue<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Use the arena's evaluate_custom_operator method
    arena.evaluate_custom_operator(name, args, context)
}

/// Evaluates arguments and returns them as a slice of DataValues
/// This function is optimized to avoid unnecessary allocations
#[inline]
fn evaluate_arguments<'a>(
    args: &'a Token<'a>,
    context: &EvalContext<'a>,
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
                let value = evaluate(item, context, arena)?;
                values.push(value.clone());
            }

            // Create the array slice
            Ok(arena.bump_vec_into_slice(values))
        }

        // For other token types, evaluate to a single value and wrap in a slice
        _ => {
            let value = evaluate(args, context, arena)?;
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
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Get token references for lazy evaluation
    let token_refs = convert_to_token_refs(args, arena);

    match op_type {
        OperatorType::Comparison(comp_op) => {
            evaluate_comparison_operator(comp_op, token_refs, context, arena)
        }
        OperatorType::Array(array_op) => {
            evaluate_array_operator(array_op, token_refs, context, arena)
        }
        OperatorType::Arithmetic(arith_op) => {
            // Evaluate arguments once and pass to the appropriate function
            let args_result = evaluate_arguments(args, context, arena)?;
            evaluate_arithmetic_operator(arith_op, args_result, context, arena)
        }
        OperatorType::Control(control_op) => {
            evaluate_control_operator(control_op, args, token_refs, context, arena)
        }
        OperatorType::String(string_op) => {
            evaluate_string_operator(string_op, token_refs, context, arena)
        }
        OperatorType::DateTime(datetime_op) => {
            // Evaluate arguments once and pass to the appropriate function
            let args_result = evaluate_arguments(args, context, arena)?;
            evaluate_datetime_operator(datetime_op, args_result, context, arena)
        }
        OperatorType::Missing => missing::eval_missing(token_refs, context, arena),
        OperatorType::MissingSome => missing::eval_missing_some(token_refs, context, arena),
        OperatorType::Exists => {
            let args_result = evaluate_arguments(args, context, arena)?;
            val::eval_exists(args_result, context, arena)
        }
        OperatorType::Coalesce => eval_coalesce(token_refs, context, arena),
        OperatorType::Throw => throw::eval_throw(token_refs, context, arena),
        OperatorType::Try => r#try::eval_try(token_refs, context, arena),
        OperatorType::Val => val::eval_val(token_refs, context, arena),
        OperatorType::Type => type_op::eval_type(token_refs, context, arena),
        OperatorType::ArrayLiteral => evaluate_array_literal_operator(token_refs, context, arena),
    }
}

/// Evaluates a comparison operator
#[inline]
fn evaluate_comparison_operator<'a>(
    comp_op: comparison::ComparisonOp,
    token_refs: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match comp_op {
        comparison::ComparisonOp::Equal => comparison::eval_equal(token_refs, context, arena),
        comparison::ComparisonOp::StrictEqual => {
            comparison::eval_strict_equal(token_refs, context, arena)
        }
        comparison::ComparisonOp::NotEqual => {
            comparison::eval_not_equal(token_refs, context, arena)
        }
        comparison::ComparisonOp::StrictNotEqual => {
            comparison::eval_strict_not_equal(token_refs, context, arena)
        }
        comparison::ComparisonOp::GreaterThan => {
            comparison::eval_greater_than(token_refs, context, arena)
        }
        comparison::ComparisonOp::GreaterThanOrEqual => {
            comparison::eval_greater_than_or_equal(token_refs, context, arena)
        }
        comparison::ComparisonOp::LessThan => {
            comparison::eval_less_than(token_refs, context, arena)
        }
        comparison::ComparisonOp::LessThanOrEqual => {
            comparison::eval_less_than_or_equal(token_refs, context, arena)
        }
    }
}

/// Evaluates an array operator
#[inline]
fn evaluate_array_operator<'a>(
    array_op: array::ArrayOp,
    token_refs: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match array_op {
        array::ArrayOp::Map => array::eval_map(token_refs, context, arena),
        array::ArrayOp::Filter => array::eval_filter(token_refs, context, arena),
        array::ArrayOp::Reduce => array::eval_reduce(token_refs, context, arena),
        array::ArrayOp::All => array::eval_all(token_refs, context, arena),
        array::ArrayOp::Some => array::eval_some(token_refs, context, arena),
        array::ArrayOp::None => array::eval_none(token_refs, context, arena),
        array::ArrayOp::Merge => array::eval_merge(token_refs, context, arena),
        array::ArrayOp::In => array::eval_in(token_refs, context, arena),
        array::ArrayOp::Length => array::eval_length(token_refs, context, arena),
        array::ArrayOp::Slice => array::eval_slice(token_refs, context, arena),
        array::ArrayOp::Sort => array::eval_sort(token_refs, context, arena),
    }
}

/// Evaluates an arithmetic operator
#[inline]
fn evaluate_arithmetic_operator<'a>(
    arith_op: arithmetic::ArithmeticOp,
    args_result: &'a [DataValue<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match arith_op {
        arithmetic::ArithmeticOp::Add => arithmetic::eval_add(args_result, context, arena),
        arithmetic::ArithmeticOp::Subtract => arithmetic::eval_sub(args_result, context, arena),
        arithmetic::ArithmeticOp::Multiply => arithmetic::eval_mul(args_result, context, arena),
        arithmetic::ArithmeticOp::Divide => arithmetic::eval_div(args_result, context, arena),
        arithmetic::ArithmeticOp::Modulo => arithmetic::eval_mod(args_result, context, arena),
        arithmetic::ArithmeticOp::Min => arithmetic::eval_min(args_result, context),
        arithmetic::ArithmeticOp::Max => arithmetic::eval_max(args_result, context),
        arithmetic::ArithmeticOp::Abs => arithmetic::eval_abs(args_result, context, arena),
        arithmetic::ArithmeticOp::Ceil => arithmetic::eval_ceil(args_result, context, arena),
        arithmetic::ArithmeticOp::Floor => arithmetic::eval_floor(args_result, context, arena),
    }
}

/// Evaluates a control flow operator
#[inline]
fn evaluate_control_operator<'a>(
    control_op: control::ControlOp,
    args: &'a Token<'a>,
    token_refs: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Validate array literals for certain control operations
    if matches!(
        control_op,
        control::ControlOp::If | control::ControlOp::And | control::ControlOp::Or
    ) && !args.is_array_literal()
    {
        return Err(LogicError::InvalidArgumentsError);
    }

    match control_op {
        control::ControlOp::If => control::eval_if(token_refs, context, arena),
        control::ControlOp::And => control::eval_and(token_refs, context, arena),
        control::ControlOp::Or => control::eval_or(token_refs, context, arena),
        control::ControlOp::Not => control::eval_not(token_refs, context, arena),
        control::ControlOp::DoubleNegation => {
            control::eval_double_negation(token_refs, context, arena)
        }
    }
}

/// Evaluates a string operator
#[inline]
fn evaluate_string_operator<'a>(
    string_op: string::StringOp,
    token_refs: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match string_op {
        string::StringOp::Cat => string::eval_cat(token_refs, context, arena),
        string::StringOp::Substr => string::eval_substr(token_refs, context, arena),
        string::StringOp::StartsWith => string::eval_starts_with(token_refs, context, arena),
        string::StringOp::EndsWith => string::eval_ends_with(token_refs, context, arena),
        string::StringOp::Upper => string::eval_upper(token_refs, context, arena),
        string::StringOp::Lower => string::eval_lower(token_refs, context, arena),
        string::StringOp::Trim => string::eval_trim(token_refs, context, arena),
        string::StringOp::Replace => string::eval_replace(token_refs, context, arena),
        string::StringOp::Split => string::eval_split(token_refs, context, arena),
    }
}

/// Evaluates a datetime operator
#[inline]
fn evaluate_datetime_operator<'a>(
    datetime_op: datetime::DateTimeOp,
    args_result: &'a [DataValue<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    match datetime_op {
        datetime::DateTimeOp::DateTime => {
            datetime::eval_datetime_operator(args_result, context, arena)
        }
        datetime::DateTimeOp::Timestamp => {
            datetime::eval_timestamp_operator(args_result, context, arena)
        }
        datetime::DateTimeOp::Now => datetime::eval_now(context, arena),
        datetime::DateTimeOp::ParseDate => datetime::eval_parse_date(args_result, context, arena),
        datetime::DateTimeOp::FormatDate => datetime::eval_format_date(args_result, context, arena),
        datetime::DateTimeOp::DateDiff => datetime::eval_date_diff(args_result, context, arena),
    }
}

/// Evaluates an array literal operator
#[inline]
fn evaluate_array_literal_operator<'a>(
    token_refs: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Just evaluate all elements as an array
    let mut values = arena.get_data_value_vec();

    for token in token_refs {
        let value = evaluate(token, context, arena)?;
        values.push(value.clone());
    }

    let array_slice = arena.bump_vec_into_slice(values);
    let result = DataValue::Array(array_slice);
    Ok(arena.alloc(result))
}

/// Evaluates a coalesce operation, which returns the first non-null value.
fn eval_coalesce<'a>(
    args: &'a [&'a Token<'a>],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // If no arguments, return null
    if args.is_empty() {
        return Ok(arena.null_value());
    }

    // Return the first non-null value
    for arg in args {
        let value = evaluate(arg, context, arena)?;

        // Check if the value is null
        if !value.is_null() {
            return Ok(value);
        }
    }

    // If all values are null, return null
    Ok(arena.null_value())
}

/// Evaluates a structured object by evaluating each field value while preserving keys
fn evaluate_structured_object<'a>(
    fields: &'a [(&'a str, &'a Token<'a>)],
    context: &EvalContext<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Create a vector to hold the evaluated field-value pairs
    let mut evaluated_fields = Vec::with_capacity(fields.len());

    // Evaluate each field value
    for (key, value_token) in fields {
        let evaluated_value = evaluate(value_token, context, arena)?;
        evaluated_fields.push((*key, evaluated_value.clone()));
    }

    // Convert to a slice and create the object
    let fields_slice = arena.vec_into_slice(evaluated_fields);
    let result = DataValue::Object(fields_slice);
    Ok(arena.alloc(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::operators::comparison::ComparisonOp;
    use crate::logic::token::{OperatorType, Token};
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_evaluate_literal() {
        let arena = DataArena::new();
        let root = arena.null_value();
        let context = EvalContext::new(root);

        // Null
        let token = Token::literal(DataValue::null());
        let token_ref = arena.alloc(token);
        let result = evaluate(token_ref, &context, &arena).unwrap();
        assert!(result.is_null());

        // Boolean
        let token = Token::literal(DataValue::bool(true));
        let token_ref = arena.alloc(token);
        let result = evaluate(token_ref, &context, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn test_evaluate_comparison() {
        let arena = DataArena::new();
        let data_json = json!({"foo": 42, "bar": "hello"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        let context = EvalContext::new(&data);

        // Equal - create token structure directly:
        // {"==": [{"var": "foo"}, 42]}
        let var_token = Token::variable("foo", None);
        let var_ref = arena.alloc(var_token);

        let literal_token = Token::literal(DataValue::integer(42));
        let literal_ref = arena.alloc(literal_token);

        let args = vec![var_ref, literal_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let equal_token = Token::operator(OperatorType::Comparison(ComparisonOp::Equal), array_ref);
        let equal_ref = arena.alloc(equal_token);

        let result = evaluate(equal_ref, &context, &arena).unwrap();
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn test_evaluate_coalesce() {
        let arena = DataArena::new();
        let data_json = json!({"person": {"name": "John"}, "name": "Jane"});
        let data = <DataValue as FromJson>::from_json(&data_json, &arena);
        let context = EvalContext::new(&data);

        // Create {"??": [{"var": "name"}]}
        // Instead of using a string literal, we need to use a variable reference
        let name_var_token = Token::variable("name", None);
        let name_var_ref = arena.alloc(name_var_token);

        let args = vec![name_var_ref];
        let array_token = Token::ArrayLiteral(args);
        let array_ref = arena.alloc(array_token);

        let coalesce_token = Token::operator(OperatorType::Coalesce, array_ref);
        let coalesce_ref = arena.alloc(coalesce_token);

        let result = evaluate(coalesce_ref, &context, &arena).unwrap();
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
        let data_ref = arena.alloc(data.clone());
        let context = EvalContext::new(data_ref);

        // Test simple val: { "val": "hello" }
        let val_arg = Token::literal(DataValue::string(&arena, "hello"));
        let val_token = Token::operator(OperatorType::Val, arena.alloc(val_arg));

        let result = evaluate(arena.alloc(val_token), &context, &arena).unwrap();
        assert_eq!(*result, DataValue::integer(0));

        // Test nested val: { "val": ["nested", "world"] }
        let nested_args = arena.vec_into_slice(vec![
            DataValue::string(&arena, "nested"),
            DataValue::string(&arena, "world"),
        ]);
        let nested_array = DataValue::Array(nested_args);
        let nested_val_arg = Token::literal(nested_array);
        let nested_val_token = Token::operator(OperatorType::Val, arena.alloc(nested_val_arg));

        let result = evaluate(arena.alloc(nested_val_token), &context, &arena).unwrap();
        assert_eq!(*result, DataValue::integer(1));

        // Test val with empty array (should return the entire data)
        let empty_array = DataValue::Array(arena.vec_into_slice(vec![]));
        let empty_val_arg = Token::literal(empty_array);
        let empty_val_token = Token::operator(OperatorType::Val, arena.alloc(empty_val_arg));

        let result = evaluate(arena.alloc(empty_val_token), &context, &arena).unwrap();
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
        let root = arena.null_value();
        let context = EvalContext::new(root);

        // Test simple datetime conversion: { "datetime": "2022-07-06T13:20:06Z" }
        let dt_arg = Token::literal(DataValue::string(&arena, "2022-07-06T13:20:06Z"));
        let dt_token = Token::operator(
            OperatorType::DateTime(DateTimeOp::DateTime),
            arena.alloc(dt_arg),
        );

        let result = evaluate(arena.alloc(dt_token), &context, &arena).unwrap();

        // Check if it's a string value (datetime operator now returns formatted strings)
        assert!(result.is_string());
        assert_eq!(result.as_str().unwrap(), "2022-07-06T13:20:06Z");
    }
}
