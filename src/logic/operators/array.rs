//! Array operators for logic expressions.
//!
//! This module provides implementations for array operators
//! such as map, filter, reduce, etc.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
use crate::logic::token::OperatorType;
use crate::logic::operators::arithmetic::ArithmeticOp;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;

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
pub fn eval_all<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(args[0], data, arena)?;
    
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
    for item in items.iter() {
        // Evaluate the function with the item as context
        if !evaluate(condition, item, arena)?.coerce_to_bool() {
            return Ok(arena.false_value());
        }
    }
    
    // If all items satisfy the condition, return true
    Ok(arena.true_value())
}

/// Evaluates a some operation.
pub fn eval_some<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(args[0], data, arena)?;
    
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
    for item in items.iter() {
        // Evaluate the function with the item as context
        if evaluate(condition, item, arena)?.coerce_to_bool() {
            return Ok(arena.true_value());
        }
    }
    
    // If no items satisfy the condition, return false
    Ok(arena.false_value())
}

/// Evaluates a none operation.
pub fn eval_none<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(args[0], data, arena)?;
    
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
    for item in items.iter() {
        // Evaluate the function with the item as context
        if evaluate(condition, item, arena)?.coerce_to_bool() {
            return Ok(arena.false_value());
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
pub fn eval_map<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(args[0], data, arena)?;
    
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
    
    // Get a vector from the arena's pool to avoid allocation
    let mut result_values = arena.get_data_value_vec();
    result_values.reserve(items.len()); // Pre-allocate for expected size
    
    // Apply the function to each item
    for item in items.iter() {
        // Evaluate the function with the item as context
        let result = evaluate(function, item, arena)?;
        result_values.push(result.clone());
    }
    
    // Create the result array
    let result = DataValue::Array(arena.alloc_slice_clone(&result_values));
    
    // Return the vector to the pool
    arena.release_data_value_vec(result_values);
    
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
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(args[0], data, arena)?;
    
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
    
    // Optimization: Pre-scan to estimate result size
    // For small arrays (<=16 items), just use the array length as capacity
    // For larger arrays, sample a few items to estimate selectivity
    let estimated_capacity = if items.len() <= 16 {
        items.len()
    } else {
        // Sample up to 8 items to estimate selectivity
        let sample_size = std::cmp::min(8, items.len());
        let mut sample_count = 0;
        
        for i in 0..sample_size {
            // Use evenly distributed indices for better sampling
            let idx = (i * items.len()) / sample_size;
            let item = &items[idx];
            
            // Evaluate the condition with the item as context
            if evaluate(condition, item, arena)?.coerce_to_bool() {
                sample_count += 1;
            }
        }
        
        // Estimate capacity based on sample selectivity
        let selectivity = sample_count as f64 / sample_size as f64;
        std::cmp::max(4, (items.len() as f64 * selectivity).ceil() as usize)
    };
    
    // Get a vector from the arena's pool with the estimated capacity
    let mut results = arena.get_data_value_vec();
    results.reserve(estimated_capacity);
    
    // Filter the array
    for item in items.iter() {
        // Evaluate the condition with the item as context
        if evaluate(condition, item, arena)?.coerce_to_bool() {
            results.push(item.clone());
        }
    }
    
    // Create the result array
    let result = DataValue::Array(arena.alloc_slice_clone(&results));
    
    // Return the vector to the pool
    arena.release_data_value_vec(results);
    
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
    let initial_val = initial.coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
    let mut sum = initial_val;
    
    for i in start_idx..items.len() {
        sum += items[i].coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
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
    let initial_val = initial.coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
    let mut product = initial_val;
    
    for i in start_idx..items.len() {
        product *= items[i].coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
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
    let initial_val = initial.coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
    let mut result = initial_val;
    
    for i in start_idx..items.len() {
        result -= items[i].coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
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
    let initial_val = initial.coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
    let mut result = initial_val;
    
    for i in start_idx..items.len() {
        let divisor = items[i].coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
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
    let initial_val = initial.coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
    let mut result = initial_val;
    
    for i in start_idx..items.len() {
        let divisor = items[i].coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
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
    let initial_val = initial.coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
    let mut min_val = initial_val;
    
    for i in start_idx..items.len() {
        let val = items[i].coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
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
    let initial_val = initial.coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
    let mut max_val = initial_val;
    
    for i in start_idx..items.len() {
        let val = items[i].coerce_to_number().ok_or(LogicError::NaNError)?.as_f64();
        max_val = max_val.max(val);
    }
    
    Ok(arena.alloc(DataValue::float(max_val)))
}

/// Checks if an operator token matches the expected pattern for optimized arithmetic operations
fn is_arithmetic_reduce_pattern<'a>(
    function: &'a Token<'a>,
) -> Option<ArithmeticOp> {
    if let Token::Operator { op_type, args: fn_args } = function {
        if let OperatorType::Arithmetic(arith_op) = op_type {
            if let Token::ArrayLiteral(fn_args_tokens) = fn_args {
                if fn_args_tokens.len() == 2 {
                    let is_var_current = is_var_with_path(fn_args_tokens[0], "current");
                    let is_var_acc = is_var_with_path(fn_args_tokens[1], "accumulator");
                    
                    if is_var_current && is_var_acc {
                        return Some(*arith_op);
                    }
                }
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
pub fn eval_reduce<'a>(args: &'a [&'a Token<'a>], data: &'a DataValue<'a>, arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() < 2 || args.len() > 3 {
        return Err(LogicError::InvalidArgumentsError);
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => {
            // If we have an initial value, return it
            if args.len() == 3 {
                return evaluate(args[2], data, arena);
            }
            return Err(LogicError::InvalidArgumentsError);
        },
        _ => return Err(LogicError::InvalidArgumentsError),
    };
    
    // Fast path for empty array
    if items.is_empty() {
        // If we have an initial value, return it
        if args.len() == 3 {
            return evaluate(args[2], data, arena);
        }
        return Err(LogicError::InvalidArgumentsError);
    }
    
    // Get the initial value
    let initial = if args.len() == 3 {
        evaluate(args[2], data, arena)?
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
    for i in start_idx..items.len() {
        // Create object entries with references to the values
        let entries = [
            (curr_key, items[i].clone()),
            (acc_key, acc.clone()),
        ];
        
        // Allocate the entries in the arena
        let context_entries = arena.alloc_slice_clone(&entries);
        
        // Create the context object
        let context = arena.alloc(DataValue::Object(context_entries));
        
        // Evaluate the function with the context
        acc = evaluate(function, context, arena)?;
    }
    
    Ok(acc)
}

/// Evaluates a merge operation.
pub fn eval_merge<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for no arguments
    if args.is_empty() {
        return Ok(arena.alloc(DataValue::Array(&[])));
    }
    
    // Evaluate all arguments and collect arrays
    let mut result = arena.get_data_value_vec();
    
    for arg in args {
        let value = evaluate(arg, data, arena)?;
        
        match value {
            DataValue::Array(items) => {
                // Add all items from the array
                for item in items.iter() {
                    result.push(item.clone());
                }
            },
            _ => {
                // Add non-array values directly
                result.push(value.clone());
            }
        }
    }
    
    // Create the result array
    let result_array = DataValue::Array(arena.alloc_slice_clone(&result));
    arena.release_data_value_vec(result);
    
    Ok(arena.alloc(result_array))
}

/// Evaluates an "in" operation.
pub fn eval_in<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    let needle = evaluate(args[0], data, arena)?;
    let haystack = evaluate(args[1], data, arena)?;

    let result = match haystack {
        DataValue::String(s) => {
            let needle_str = match needle {
                DataValue::String(ns) => *ns,
                _ => arena.alloc_str(&needle.to_string()),
            };
            s.contains(needle_str)
        }
        DataValue::Array(arr) => {
            arr.iter().any(|item| {
                match (item, needle) {
                    (DataValue::Number(a), DataValue::Number(b)) => a == b,
                    (DataValue::String(a), DataValue::String(b)) => a == b,
                    (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
                    _ => false,
                }
            })
        }
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
    use crate::JsonLogic;
    use serde_json::json;

    #[test]
    fn test_map_with_op_syntax() {
        // Create JSONLogic instance
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });
        
        // Test mapping an array to double each value
        let rule = builder.array()
            .mapOp()
            .array(builder.var("numbers").build())
            .mapper(
                builder.arithmetic()
                    .multiplyOp()
                    .var("")
                    .int(2)
                    .build()
            )
            .build();
        
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!([2, 4, 6, 8]));
        
        // Test with empty array
        let data_json = json!({
            "numbers": []
        });
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_filter_with_op_syntax() {
        // Create JSONLogic instance
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({
            "numbers": [1, 2, 3, 4, 5, 6, 7, 8]
        });
        
        // Test filtering for even numbers
        let rule = builder.array()
            .filterOp()
            .array(builder.var("numbers").build())
            .condition(
                builder.compare()
                    .equalOp()
                    .var("numbers")
                    .operand(builder.arithmetic()
                        .moduloOp()
                        .var("")
                        .int(2)
                        .build()
                    )
                    .int(0)
                    .build()
            )
            .build();
        
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!([2, 4, 6, 8]));
        
        // Test with empty array
        let data_json = json!({
            "numbers": []
        });
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_reduce_with_op_syntax() {
        // Create JSONLogic instance
        let logic = JsonLogic::new();
        let builder = logic.builder();
        
        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });
        
        // Test reducing an array to sum its values
        let rule = builder.array()
            .reduceOp()
            .array(builder.var("numbers").build())
            .reducer(
                builder.arithmetic()
                    .addOp()
                    .var("current")
                    .var("accumulator")
                    .build()
            )
            .initial(builder.int(0))
            .build();
        
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(10)); // 1 + 2 + 3 + 4 = 10
        
        // Test with empty array - should return initial value
        let data_json = json!({
            "numbers": []
        });
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(0));
        
        // Test with different initial value
        let rule = builder.array()
            .reduceOp()
            .array(builder.var("numbers").build())
            .reducer(
                builder.arithmetic()
                    .addOp()
                    .var("current")
                    .var("accumulator")
                    .build()
            )
            .initial(builder.int(10))
            .build();
        
        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });
        let result = logic.apply_logic(&rule, &data_json).unwrap();
        assert_eq!(result, json!(20)); // 10 + 1 + 2 + 3 + 4 = 20
    }
} 