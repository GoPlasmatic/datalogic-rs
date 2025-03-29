//! Array operators for logic expressions.
//!
//! This module provides implementations for array operators
//! such as map, filter, reduce, etc.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::operators::arithmetic::ArithmeticOp;
use crate::logic::token::OperatorType;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Enumeration of array operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayOp {
    /// Map operator
    Map,
    /// Filter operator
    Filter,
    /// Reduce operator
    Reduce,
    /// All operator
    All,
    /// Some operator
    Some,
    /// None operator
    None,
    /// Merge operator
    Merge,
    /// In operator
    In,
}

/// Evaluates an all operation.
pub fn eval_all<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the array
    let array = evaluate(args[0], arena)?;
    if let Token::Variable { path, .. } = args[0] {
        let key = DataValue::String(path);
        arena.push_path_key(arena.alloc(key));
    }

    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.false_value()),
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    // If the array is empty, return false (vacuously false)
    if items.is_empty() {
        return Ok(arena.false_value());
    }

    // Cache the condition token
    let condition = args[1];

    // Check if all items satisfy the condition
    for (index, item) in items.iter().enumerate() {
        // Store the current path chain length to preserve parent contexts
        let current_chain_len = arena.path_chain_len();

        let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
        arena.set_current_context(item, arena.alloc(key));

        // Evaluate the function with the item as context
        if !evaluate(condition, arena)?.coerce_to_bool() {
            return Ok(arena.false_value());
        }

        // Restore the path chain to its original state
        while arena.path_chain_len() > current_chain_len {
            arena.pop_path_component();
        }
    }

    // If all items satisfy the condition, return true
    Ok(arena.true_value())
}

/// Evaluates a some operation.
pub fn eval_some<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the array
    let array = evaluate(args[0], arena)?;
    if let Token::Variable { path, .. } = args[0] {
        let key = DataValue::String(path);
        arena.push_path_key(arena.alloc(key));
    }

    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.false_value()),
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    // If the array is empty, return false (vacuously false)
    if items.is_empty() {
        return Ok(arena.false_value());
    }

    // Cache the condition token
    let condition = args[1];

    // Check if any item satisfies the condition
    for (index, item) in items.iter().enumerate() {
        // Store the current path chain length to preserve parent contexts
        let current_chain_len = arena.path_chain_len();

        let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
        arena.set_current_context(item, arena.alloc(key));

        // Evaluate the function with the item as context
        if evaluate(condition, arena)?.coerce_to_bool() {
            return Ok(arena.true_value());
        }

        // Restore the path chain to its original state
        while arena.path_chain_len() > current_chain_len {
            arena.pop_path_component();
        }
    }

    // If no items satisfy the condition, return false
    Ok(arena.false_value())
}

/// Evaluates a none operation.
pub fn eval_none<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the array
    let array = evaluate(args[0], arena)?;
    if let Token::Variable { path, .. } = args[0] {
        let key = DataValue::String(path);
        arena.push_path_key(arena.alloc(key));
    }

    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.true_value()),
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    // If the array is empty, return true (vacuously true)
    if items.is_empty() {
        return Ok(arena.true_value());
    }

    // Cache the condition token
    let condition = args[1];

    // Check if no items satisfy the condition
    for (index, item) in items.iter().enumerate() {
        // Store the current path chain length to preserve parent contexts
        let current_chain_len = arena.path_chain_len();

        let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
        arena.set_current_context(item, arena.alloc(key));

        // Evaluate the function with the item as context
        if evaluate(condition, arena)?.coerce_to_bool() {
            return Ok(arena.false_value());
        }

        // Restore the path chain to its original state
        while arena.path_chain_len() > current_chain_len {
            arena.pop_path_component();
        }
    }

    // If no items satisfy the condition, return true
    Ok(arena.true_value())
}

/// Evaluates a map operation.
///
/// The map operator applies a function to each element of an array and returns
/// a new array with the results.
///
/// Arguments:
/// - First argument: The array to map over
/// - Second argument: The function to apply to each element
///
/// Example:
/// ```json
/// {"map": [{"var": "integers"}, {"*": [{"var": ""}, 2]}]}
/// ```
pub fn eval_map<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the array
    let array = evaluate(args[0], arena)?;
    if let Token::Variable { path, .. } = args[0] {
        let key = DataValue::String(path);
        arena.push_path_key(arena.alloc(key));
    }

    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.empty_array_value()),
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    // Fast path for empty array
    if items.is_empty() {
        return Ok(arena.empty_array_value());
    }

    // Cache the function token
    let function = args[1];

    // Get a vector from the arena's pool
    let mut result_values = arena.get_data_value_vec();
    result_values.reserve(items.len()); // Pre-allocate for expected size

    // Apply the function to each item
    for (index, item) in items.iter().enumerate() {
        // Store the current path chain length to preserve parent contexts
        let current_chain_len = arena.path_chain_len();

        // Set the current item as context
        // Use the index as the key for the current context
        let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
        arena.set_current_context(item, arena.alloc(key));

        // Evaluate the function with the item as context
        let result = evaluate(function, arena)?;
        result_values.push(result.clone());

        // Restore the path chain to its original state
        while arena.path_chain_len() > current_chain_len {
            arena.pop_path_component();
        }
    }

    // Create the result array
    let result = DataValue::Array(arena.bump_vec_into_slice(result_values));

    // Return the result array
    Ok(arena.alloc(result))
}

/// Evaluates a filter operation.
///
/// The filter operator filters an array based on a condition and returns
/// a new array with the elements that satisfy the condition.
///
/// Arguments:
/// - First argument: The array to filter
/// - Second argument: The condition to apply to each element
///
/// Example:
/// ```json
/// {"filter": [{"var": "integers"}, {">": [{"var": ""}, 2]}]}
/// ```
pub fn eval_filter<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the array
    let array = evaluate(args[0], arena)?;
    if let Token::Variable { path, .. } = args[0] {
        let key = DataValue::String(path);
        arena.push_path_key(arena.alloc(key));
    }

    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.empty_array_value()),
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    // Fast path for empty array
    if items.is_empty() {
        return Ok(arena.empty_array_value());
    }

    // Cache the condition token
    let condition = args[1];

    // Get a vector from the arena's pool with the estimated capacity
    let mut results = arena.get_data_value_vec();
    results.reserve(items.len());

    // Filter the array
    for (index, item) in items.iter().enumerate() {
        // Store the current path chain length to preserve parent contexts
        let current_chain_len = arena.path_chain_len();

        let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
        arena.set_current_context(item, arena.alloc(key));

        // Evaluate the condition with the item as context
        if evaluate(condition, arena)?.coerce_to_bool() {
            results.push(item.clone());
        }

        // Restore the path chain to its original state
        while arena.path_chain_len() > current_chain_len {
            arena.pop_path_component();
        }
    }

    // Create the result array
    let result = DataValue::Array(arena.bump_vec_into_slice(results));

    // Return the result array
    Ok(arena.alloc(result))
}

/// Helper function to check if a token is a variable with a specific path
fn is_var_with_path(token: &Token, path: &str) -> bool {
    match token {
        Token::Variable { path: var_path, .. } => *var_path == path,
        _ => false,
    }
}

/// Performs a reduce operation with addition
fn reduce_add<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let initial_val = initial
        .coerce_to_number()
        .ok_or(LogicError::NaNError)?
        .as_f64();
    let mut sum = initial_val;

    for item in items.iter().skip(start_idx) {
        sum += item
            .coerce_to_number()
            .ok_or(LogicError::NaNError)?
            .as_f64();
    }

    Ok(arena.alloc(DataValue::float(sum)))
}

/// Performs a reduce operation with multiplication
fn reduce_multiply<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let initial_val = initial
        .coerce_to_number()
        .ok_or(LogicError::NaNError)?
        .as_f64();
    let mut product = initial_val;

    for item in items.iter().skip(start_idx) {
        product *= item
            .coerce_to_number()
            .ok_or(LogicError::NaNError)?
            .as_f64();
    }

    Ok(arena.alloc(DataValue::float(product)))
}

/// Performs a reduce operation with subtraction
fn reduce_subtract<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let initial_val = initial
        .coerce_to_number()
        .ok_or(LogicError::NaNError)?
        .as_f64();
    let mut result = initial_val;

    for item in items.iter().skip(start_idx) {
        result -= item
            .coerce_to_number()
            .ok_or(LogicError::NaNError)?
            .as_f64();
    }

    Ok(arena.alloc(DataValue::float(result)))
}

/// Performs a reduce operation with division
fn reduce_divide<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let initial_val = initial
        .coerce_to_number()
        .ok_or(LogicError::NaNError)?
        .as_f64();
    let mut result = initial_val;

    for item in items.iter().skip(start_idx) {
        let divisor = item
            .coerce_to_number()
            .ok_or(LogicError::NaNError)?
            .as_f64();
        if divisor == 0.0 {
            return Err(LogicError::NaNError);
        }
        result /= divisor;
    }

    Ok(arena.alloc(DataValue::float(result)))
}

/// Performs a reduce operation with modulo
fn reduce_modulo<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let initial_val = initial
        .coerce_to_number()
        .ok_or(LogicError::NaNError)?
        .as_f64();
    let mut result = initial_val;

    for item in items.iter().skip(start_idx) {
        let divisor = item
            .coerce_to_number()
            .ok_or(LogicError::NaNError)?
            .as_f64();
        if divisor == 0.0 {
            return Err(LogicError::NaNError);
        }
        result %= divisor;
    }

    Ok(arena.alloc(DataValue::float(result)))
}

/// Performs a reduce operation to find the minimum value
fn reduce_min<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let initial_val = initial
        .coerce_to_number()
        .ok_or(LogicError::NaNError)?
        .as_f64();
    let mut min_val = initial_val;

    for item in items.iter().skip(start_idx) {
        let val = item
            .coerce_to_number()
            .ok_or(LogicError::NaNError)?
            .as_f64();
        min_val = min_val.min(val);
    }

    Ok(arena.alloc(DataValue::float(min_val)))
}

/// Performs a reduce operation to find the maximum value
fn reduce_max<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let initial_val = initial
        .coerce_to_number()
        .ok_or(LogicError::NaNError)?
        .as_f64();
    let mut max_val = initial_val;

    for item in items.iter().skip(start_idx) {
        let val = item
            .coerce_to_number()
            .ok_or(LogicError::NaNError)?
            .as_f64();
        max_val = max_val.max(val);
    }

    Ok(arena.alloc(DataValue::float(max_val)))
}

/// Checks if an operator token matches the expected pattern for optimized arithmetic operations
fn is_arithmetic_reduce_pattern<'a>(function: &'a Token<'a>) -> Option<ArithmeticOp> {
    if let Token::Operator {
        op_type: OperatorType::Arithmetic(arith_op),
        args: Token::ArrayLiteral(fn_args_tokens),
    } = function
    {
        if fn_args_tokens.len() == 2 {
            let is_var_current = is_var_with_path(fn_args_tokens[0], "current");
            let is_var_acc = is_var_with_path(fn_args_tokens[1], "accumulator");

            if is_var_current && is_var_acc {
                return Some(*arith_op);
            }
        }
    }
    None
}

/// Evaluates a reduce operation.
///
/// The reduce operator applies a function to each element of an array and an accumulator,
/// and returns the final accumulated value.
///
/// Arguments:
/// - First argument: The array to reduce
/// - Second argument: The function to apply to each element and the accumulator
/// - Third argument: The initial value for the accumulator
///
/// Example:
/// ```json
/// {"reduce": [{"var": "integers"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}
/// ```
pub fn eval_reduce<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() < 2 || args.len() > 3 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the array
    let array = evaluate(args[0], arena)?;
    if let Token::Variable { path, .. } = args[0] {
        let key = DataValue::String(path);
        arena.push_path_key(arena.alloc(key));
    }

    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => {
            // If we have an initial value, return it
            if args.len() == 3 {
                return evaluate(args[2], arena);
            }
            return Err(LogicError::InvalidArgumentsError);
        }
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    // Fast path for empty array
    if items.is_empty() {
        // If we have an initial value, return it
        if args.len() == 3 {
            return evaluate(args[2], arena);
        }
        return Err(LogicError::InvalidArgumentsError);
    }

    // Get the initial value
    let initial = if args.len() == 3 {
        evaluate(args[2], arena)?
    } else {
        // If no initial value is provided, use the first item
        &items[0]
    };

    // Cache the function token
    let function = args[1];

    // Start from the first item if no initial value was provided
    let start_idx = if args.len() == 3 { 0 } else { 1 };

    // Optimization for arithmetic operators - desugar reduce to direct arithmetic operation
    if let Some(arith_op) = is_arithmetic_reduce_pattern(function) {
        // Use our specialized helper functions for each arithmetic operation
        return match arith_op {
            ArithmeticOp::Add => reduce_add(items, initial, start_idx, arena),
            ArithmeticOp::Multiply => reduce_multiply(items, initial, start_idx, arena),
            ArithmeticOp::Subtract => reduce_subtract(items, initial, start_idx, arena),
            ArithmeticOp::Divide => reduce_divide(items, initial, start_idx, arena),
            ArithmeticOp::Modulo => reduce_modulo(items, initial, start_idx, arena),
            ArithmeticOp::Min => reduce_min(items, initial, start_idx, arena),
            ArithmeticOp::Max => reduce_max(items, initial, start_idx, arena),
        };
    }

    // NOTE: For the generic case, we still need to create a context object with the current item and accumulator

    // Initialize static keys only once - these are interned and reused
    let curr_key = arena.intern_str("current");
    let acc_key = arena.intern_str("accumulator");

    // The accumulator will be updated on each iteration
    let mut acc = initial;

    // Reduce the array using the generic approach
    for (index, item) in items.iter().enumerate() {
        // Store the current path chain length to preserve parent contexts
        let current_chain_len = arena.path_chain_len();

        let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
        // Create object entries with references to the values
        let entries = vec![(curr_key, item.clone()), (acc_key, acc.clone())];

        // Allocate the entries in the arena
        let context_entries = arena.vec_into_slice(entries);

        // Create the context object
        let context = arena.alloc(DataValue::Object(context_entries));
        arena.set_current_context(context, &key);

        // Evaluate the function with the context
        acc = evaluate(function, arena)?;

        // Restore the path chain to its original state
        while arena.path_chain_len() > current_chain_len {
            arena.pop_path_component();
        }
    }

    Ok(acc)
}

/// Evaluates a merge operation.
pub fn eval_merge<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for no arguments
    if args.is_empty() {
        return Ok(arena.empty_array_value());
    }

    // Evaluate all arguments and collect arrays
    let mut result = Vec::with_capacity(args.len());

    for arg in args {
        let value = evaluate(arg, arena)?;

        match value {
            DataValue::Array(items) => {
                // Add all items from the array
                for item in items.iter() {
                    result.push(item.clone());
                }
            }
            _ => {
                // Add non-array values directly
                result.push(value.clone());
            }
        }
    }

    // Create the result array
    let result_array = DataValue::Array(arena.vec_into_slice(result));

    Ok(arena.alloc(result_array))
}

/// Evaluates an "in" operation.
pub fn eval_in<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let needle = evaluate(args[0], arena)?;
    let haystack = evaluate(args[1], arena)?;

    let result = match haystack {
        DataValue::String(s) => {
            let needle_str = match needle {
                DataValue::String(ns) => *ns,
                _ => arena.alloc_str(&needle.to_string()),
            };
            s.contains(needle_str)
        }
        DataValue::Array(arr) => arr.iter().any(|item| match (item, needle) {
            (DataValue::Number(a), DataValue::Number(b)) => a == b,
            (DataValue::String(a), DataValue::String(b)) => a == b,
            (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
            _ => false,
        }),
        DataValue::Object(obj) => {
            // For objects, check if needle is a key in the object
            if let DataValue::String(key) = needle {
                obj.iter().any(|(k, _)| *k == *key)
            } else {
                // If needle is not a string, convert it to a string and check
                let key_str = needle.to_string();
                obj.iter().any(|(k, _)| *k == key_str)
            }
        }
        _ => false,
    };

    if result {
        Ok(arena.true_value())
    } else {
        Ok(arena.false_value())
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::datalogic_core::DataLogicCore;
    use serde_json::json;

    #[test]
    fn test_map_with_op_syntax() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });

        // Test mapping an array to double each value
        let rule = builder
            .array()
            .map_op()
            .array(builder.var("numbers").build())
            .mapper(builder.arithmetic().multiply_op().var("").int(2).build())
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!([2, 4, 6, 8]));

        // Test with empty array
        let data_json = json!({
            "numbers": []
        });
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_filter_with_op_syntax() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({
            "numbers": [1, 2, 3, 4, 5, 6, 7, 8]
        });

        // Test filtering for even numbers
        let rule = builder
            .array()
            .filter_op()
            .array(builder.var("numbers").build())
            .condition(
                builder
                    .compare()
                    .equal_op()
                    .operand(builder.arithmetic().modulo_op().var("").int(2).build())
                    .operand(builder.int(0))
                    .build(),
            )
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!([2, 4, 6, 8]));

        // Test with empty array
        let data_json = json!({
            "numbers": []
        });
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_reduce_with_op_syntax() {
        let core = DataLogicCore::new();
        let builder = core.builder();

        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });

        // Test reducing an array to sum its values
        let rule = builder
            .array()
            .reduce_op()
            .array(builder.var("numbers").build())
            .reducer(
                builder
                    .arithmetic()
                    .add_op()
                    .var("current")
                    .var("accumulator")
                    .build(),
            )
            .initial(builder.int(0))
            .build();

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(10)); // 1 + 2 + 3 + 4 = 10

        // Test with empty array - should return initial value
        let data_json = json!({
            "numbers": []
        });
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(0));

        // Test with different initial value
        let rule = builder
            .array()
            .reduce_op()
            .array(builder.var("numbers").build())
            .reducer(
                builder
                    .arithmetic()
                    .add_op()
                    .var("current")
                    .var("accumulator")
                    .build(),
            )
            .initial(builder.int(10))
            .build();

        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(20)); // 10 + 1 + 2 + 3 + 4 = 20
    }
}
