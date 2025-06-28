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
    /// Length operator
    Length,
    /// Slice operator
    Slice,
    /// Sort operator
    Sort,
}

/// Enumeration of array predicate operations (all, some, none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PredicateOp {
    /// All items match the condition (AND)
    All,
    /// At least one item matches the condition (OR)
    Some,
    /// No items match the condition (NOR)
    None,
}

/// Helper function for evaluating array predicate operations (all, some, none).
///
/// This centralizes the common logic for the three predicate operations:
/// - All: Returns true if all items satisfy the condition
/// - Some: Returns true if at least one item satisfies the condition
/// - None: Returns true if no items satisfy the condition
fn eval_predicate<'a>(
    args: &'a [&'a Token<'a>],
    op_type: PredicateOp,
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
        DataValue::Null => {
            // For empty arrays:
            // - all() returns false (vacuously false)
            // - some() returns false (no matching items)
            // - none() returns true (no items that match condition)
            return match op_type {
                PredicateOp::All | PredicateOp::Some => Ok(arena.false_value()),
                PredicateOp::None => Ok(arena.true_value()),
            };
        }
        _ => return Err(LogicError::InvalidArgumentsError),
    };

    // If the array is empty, apply the same logic as null
    if items.is_empty() {
        return match op_type {
            PredicateOp::All | PredicateOp::Some => Ok(arena.false_value()),
            PredicateOp::None => Ok(arena.true_value()),
        };
    }

    // Cache the condition token
    let condition = args[1];

    // Set initial result based on operation type
    // - all() starts with true (assume all match until finding a non-match)
    // - some() starts with false (assume none match until finding a match)
    // - none() starts with true (assume none match until finding a match)
    let mut result = match op_type {
        PredicateOp::All => true,
        PredicateOp::Some => false,
        PredicateOp::None => true,
    };

    // Evaluate the items
    for (index, item) in items.iter().enumerate() {
        // Store the current path chain length to preserve parent contexts
        let current_chain_len = arena.path_chain_len();

        let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
        arena.set_current_context(item, arena.alloc(key));

        // Evaluate the condition with the item as context
        let item_matches = evaluate(condition, arena)?.coerce_to_bool();

        // Restore the path chain to its original state
        while arena.path_chain_len() > current_chain_len {
            arena.pop_path_component();
        }

        // Early return optimization based on operation type
        match op_type {
            PredicateOp::All => {
                if !item_matches {
                    // If any item doesn't match, all() is false
                    result = false;
                    break;
                }
            }
            PredicateOp::Some => {
                if item_matches {
                    // If any item matches, some() is true
                    result = true;
                    break;
                }
            }
            PredicateOp::None => {
                if item_matches {
                    // If any item matches, none() is false
                    result = false;
                    break;
                }
            }
        }
    }

    // Return result based on the final boolean
    if result {
        Ok(arena.true_value())
    } else {
        Ok(arena.false_value())
    }
}

/// Evaluates an all operation.
pub fn eval_all<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    eval_predicate(args, PredicateOp::All, arena)
}

/// Evaluates a some operation.
pub fn eval_some<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    eval_predicate(args, PredicateOp::Some, arena)
}

/// Evaluates a none operation.
pub fn eval_none<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    eval_predicate(args, PredicateOp::None, arena)
}

/// Helper function to safely evaluate the first argument as an array and handle common edge cases.
/// Returns the array items or appropriate defaults for null/empty arrays.
fn get_array_items<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<Option<&'a [DataValue<'a>]>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the array
    let array = evaluate(args[0], arena)?;

    // Add path key if this is a variable path
    if let Token::Variable { path, .. } = args[0] {
        let key = DataValue::String(path);
        arena.push_path_key(arena.alloc(key));
    }

    // Check that the first argument is an array
    match array {
        DataValue::Array(items) => {
            if items.is_empty() {
                Ok(Some(&[])) // Return empty slice for empty arrays
            } else {
                Ok(Some(items))
            }
        }
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => Ok(None),
        _ => Err(LogicError::InvalidArgumentsError),
    }
}

/// Helper function to evaluate a function with an array item as context
/// and properly manage the path chain state.
fn with_array_item_context<'a, F, T>(
    item: &'a DataValue<'a>,
    index: usize,
    arena: &'a DataArena,
    callback: F,
) -> T
where
    F: FnOnce() -> T,
{
    // Store the current path chain length to preserve parent contexts
    let current_chain_len = arena.path_chain_len();

    // Set the current item as context with the index as key
    let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
    arena.set_current_context(item, arena.alloc(key));

    // Call the function with the item as context
    let result = callback();

    // Restore the path chain to its original state
    while arena.path_chain_len() > current_chain_len {
        arena.pop_path_component();
    }

    result
}

/// Evaluates a map operation.
///
/// The map operator applies a function to each element of a collection and returns
/// a new array with the results. The collection can be either an array or an object.
///
/// Arguments:
/// - First argument: The collection to map over (array or object)
/// - Second argument: The function to apply to each element
///
/// Example for array:
/// ```json
/// {"map": [{"var": "integers"}, {"*": [{"var": ""}, 2]}]}
/// ```
///
/// Example for object:
/// ```json
/// {"map": [{"var": "person"}, {"cat": [{"var": "key"}, ":", {"var": ""}]}]}
/// ```
pub fn eval_map<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument to get the collection
    let collection = evaluate(args[0], arena)?;
    if let Token::Variable { path, .. } = args[0] {
        let key = DataValue::String(path);
        arena.push_path_key(arena.alloc(key));
    }

    // Handle null case - return empty array
    if collection.is_null() {
        return Ok(arena.empty_array_value());
    }

    // Get a vector from the arena's pool for results
    let mut result_values = arena.get_data_value_vec();

    match collection {
        // Handle array case
        DataValue::Array(items) => {
            result_values.reserve(items.len());

            // Apply the function to each item
            for (index, item) in items.iter().enumerate() {
                // Store the current path chain length to preserve parent contexts
                let current_chain_len = arena.path_chain_len();

                let key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));
                arena.set_current_context(item, arena.alloc(key));

                // Evaluate the function with the item as context
                let result = evaluate(args[1], arena)?;

                result_values.push(result.clone());

                // Restore the path chain to its original state
                while arena.path_chain_len() > current_chain_len {
                    arena.pop_path_component();
                }
            }
        }

        // Handle object case
        DataValue::Object(entries) => {
            result_values.reserve(entries.len());

            // Sort keys alphabetically for consistent iteration order
            let mut entry_refs: Vec<(&str, &DataValue<'a>)> =
                entries.iter().map(|(k, v)| (*k, v)).collect();
            entry_refs.sort_by(|a, b| a.0.cmp(b.0));

            // Apply the function to each property value
            for (key, value) in entry_refs {
                // Store the current path chain length to preserve parent contexts
                let current_chain_len = arena.path_chain_len();

                let key_value = DataValue::String(key);
                arena.set_current_context(value, arena.alloc(key_value));

                // Evaluate the function with the property value as context
                let result = evaluate(args[1], arena)?;

                result_values.push(result.clone());

                // Restore the path chain to its original state
                while arena.path_chain_len() > current_chain_len {
                    arena.pop_path_component();
                }
            }
        }

        // Handle single value case - treat as single-element collection
        _ => {
            result_values.reserve(1);

            // Store the current path chain length to preserve parent contexts
            let current_chain_len = arena.path_chain_len();

            let key = DataValue::Number(crate::value::NumberValue::from_f64(0.0));
            arena.set_current_context(collection, arena.alloc(key));

            // Evaluate the function with the value as context
            let result = evaluate(args[1], arena)?;

            result_values.push(result.clone());

            // Restore the path chain to its original state
            while arena.path_chain_len() > current_chain_len {
                arena.pop_path_component();
            }
        }
    }

    // Create and return the result array
    let result = DataValue::Array(arena.bump_vec_into_slice(result_values));
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

    // Get the array items and handle empty/null arrays
    let items_opt = get_array_items(args, arena)?;

    // Handle null or empty arrays
    if items_opt.is_none() || items_opt.unwrap().is_empty() {
        return Ok(arena.empty_array_value());
    }

    let items = items_opt.unwrap();
    let condition = args[1]; // Cache the condition token

    // Get a vector from the arena's pool with the estimated capacity
    let mut results = arena.get_data_value_vec();
    results.reserve(items.len());

    // Filter the array
    for (index, item) in items.iter().enumerate() {
        // Evaluate condition with item as context
        let item_matches = with_array_item_context(item, index, arena, || {
            evaluate(condition, arena).map(|v| v.coerce_to_bool())
        })?;

        // Add the item to results if it matches the condition
        if item_matches {
            results.push(item.clone());
        }
    }

    // Create and return the result array
    let result = DataValue::Array(arena.bump_vec_into_slice(results));
    Ok(arena.alloc(result))
}

/// Helper function to check if a token is a variable with a specific path
fn is_var_with_path(token: &Token, path: &str) -> bool {
    match token {
        Token::Variable { path: var_path, .. } => *var_path == path,
        _ => false,
    }
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

/// Common functionality for arithmetic-based reduce operations
fn numeric_reduce<'a, F>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
    operation: F,
) -> Result<&'a DataValue<'a>>
where
    F: Fn(f64, f64) -> Result<f64>,
{
    // Convert initial value to number
    let initial_val = initial
        .coerce_to_number()
        .ok_or(LogicError::NaNError)?
        .as_f64();

    // Apply the operation to each item
    let mut result = initial_val;
    for item in items.iter().skip(start_idx) {
        let item_val = item
            .coerce_to_number()
            .ok_or(LogicError::NaNError)?
            .as_f64();

        result = operation(result, item_val)?;
    }

    // Return the final result as a DataValue
    Ok(arena.alloc(DataValue::float(result)))
}

/// Performs a reduce operation with addition
fn reduce_add<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    numeric_reduce(items, initial, start_idx, arena, |a, b| Ok(a + b))
}

/// Performs a reduce operation with multiplication
fn reduce_multiply<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    numeric_reduce(items, initial, start_idx, arena, |a, b| Ok(a * b))
}

/// Performs a reduce operation with subtraction
fn reduce_subtract<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    numeric_reduce(items, initial, start_idx, arena, |a, b| Ok(a - b))
}

/// Performs a reduce operation with division
fn reduce_divide<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    numeric_reduce(items, initial, start_idx, arena, |a, b| {
        if b == 0.0 {
            Err(LogicError::NaNError)
        } else {
            Ok(a / b)
        }
    })
}

/// Performs a reduce operation with modulo
fn reduce_modulo<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    numeric_reduce(items, initial, start_idx, arena, |a, b| {
        if b == 0.0 {
            Err(LogicError::NaNError)
        } else {
            Ok(a % b)
        }
    })
}

/// Performs a reduce operation to find the minimum value
fn reduce_min<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    numeric_reduce(items, initial, start_idx, arena, |a, b| Ok(a.min(b)))
}

/// Performs a reduce operation to find the maximum value
fn reduce_max<'a>(
    items: &'a [DataValue<'a>],
    initial: &'a DataValue<'a>,
    start_idx: usize,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    numeric_reduce(items, initial, start_idx, arena, |a, b| Ok(a.max(b)))
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
    // Validate argument count
    if args.len() < 2 || args.len() > 3 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Get the array items and handle empty/null arrays
    let items_opt = get_array_items(args, arena)?;

    // Handle null or empty arrays
    let items = match items_opt {
        Some(items) if !items.is_empty() => items,
        _ => {
            // For empty arrays, return the initial value if provided
            return if args.len() == 3 {
                evaluate(args[2], arena)
            } else {
                Err(LogicError::InvalidArgumentsError)
            };
        }
    };

    // Get the initial value
    let initial = if args.len() == 3 {
        evaluate(args[2], arena)?
    } else {
        // If no initial value is provided, use the first item
        &items[0]
    };

    // Start from the first item if no initial value was provided
    let start_idx = if args.len() == 3 { 0 } else { 1 };

    // Cache the function token
    let function = args[1];

    // Optimization for arithmetic operators - use specialized implementations
    if let Some(arith_op) = is_arithmetic_reduce_pattern(function) {
        return match arith_op {
            ArithmeticOp::Add => reduce_add(items, initial, start_idx, arena),
            ArithmeticOp::Multiply => reduce_multiply(items, initial, start_idx, arena),
            ArithmeticOp::Subtract => reduce_subtract(items, initial, start_idx, arena),
            ArithmeticOp::Divide => reduce_divide(items, initial, start_idx, arena),
            ArithmeticOp::Modulo => reduce_modulo(items, initial, start_idx, arena),
            ArithmeticOp::Min => reduce_min(items, initial, start_idx, arena),
            ArithmeticOp::Max => reduce_max(items, initial, start_idx, arena),
            // These operators don't really make sense in a reduction context
            ArithmeticOp::Abs | ArithmeticOp::Ceil | ArithmeticOp::Floor => {
                return Err(LogicError::InvalidArgumentsError);
            }
        };
    }

    // For the generic case, create a context object with current item and accumulator
    let curr_key = arena.intern_str("current");
    let acc_key = arena.intern_str("accumulator");
    let mut acc = initial;

    // Reduce the array using the generic approach
    for (index, item) in items.iter().enumerate().skip(start_idx) {
        // Call with context containing both current item and accumulator
        let current_chain_len = arena.path_chain_len();
        let index_key = DataValue::Number(crate::value::NumberValue::from_f64(index as f64));

        // Create context object with current item and accumulator
        let entries = vec![(curr_key, item.clone()), (acc_key, acc.clone())];
        let context_entries = arena.vec_into_slice(entries);
        let context = arena.alloc(DataValue::Object(context_entries));

        // Set context and evaluate
        arena.set_current_context(context, &index_key);
        acc = evaluate(function, arena)?;

        // Restore path chain
        while arena.path_chain_len() > current_chain_len {
            arena.pop_path_component();
        }
    }

    Ok(acc)
}

/// Evaluates a merge operation.
///
/// The merge operator combines multiple arrays into a single array.
/// Non-array values are included as single elements.
///
/// Example:
/// ```json
/// {"merge": [{"var": "array1"}, {"var": "array2"}, "single-value"]}
/// ```
pub fn eval_merge<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for no arguments
    if args.is_empty() {
        return Ok(arena.empty_array_value());
    }

    // Estimate the capacity based on the number of arguments
    // This is a heuristic - we don't know the exact size yet
    let mut result = arena.get_data_value_vec();
    result.reserve(args.len() * 2); // Assuming average array size > 1

    // Process each argument
    for arg in args {
        let value = evaluate(arg, arena)?;

        match value {
            DataValue::Array(items) => {
                // For arrays, add all items
                for item in items.iter() {
                    result.push(item.clone());
                }
            }
            DataValue::Null => {
                // Skip null values (treat as empty arrays)
                continue;
            }
            _ => {
                // For non-array values, add the value itself
                result.push(value.clone());
            }
        }
    }

    // Create and return the result array
    Ok(arena.alloc(DataValue::Array(arena.bump_vec_into_slice(result))))
}

/// Evaluates an "in" operation.
///
/// The "in" operator checks if a value exists in an array, string, or object.
/// - For strings, checks if the needle string is a substring
/// - For arrays, checks if the needle value exists in the array
/// - For objects, checks if the needle exists as a key
///
/// Arguments:
/// - First argument: The needle value to search for
/// - Second argument: The haystack to search in (string, array, or object)
///
/// Example:
/// ```json
/// {"in": ["apple", {"var": "fruits"}]}
/// ```
pub fn eval_in<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Validate arguments
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the needle and haystack
    let needle = evaluate(args[0], arena)?;
    let haystack = evaluate(args[1], arena)?;

    // Search based on haystack type
    let result = match haystack {
        // String haystack: check if needle is a substring
        DataValue::String(s) => {
            // Convert needle to string if needed
            let needle_str = match needle {
                DataValue::String(ns) => *ns,
                _ => arena.alloc_str(&needle.to_string()),
            };
            s.contains(needle_str)
        }

        // Array haystack: check if needle exists in array
        DataValue::Array(arr) => arr.iter().any(|item| {
            // Compare based on types for more accurate matching
            match (item, needle) {
                (DataValue::Number(a), DataValue::Number(b)) => a == b,
                (DataValue::String(a), DataValue::String(b)) => a == b,
                (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
                (DataValue::Null, DataValue::Null) => true,
                // For other types, use the equals method (handles coercion)
                _ => item.equals(needle),
            }
        }),

        // Object haystack: check if needle is a key
        DataValue::Object(obj) => {
            match needle {
                // If needle is a string, direct key comparison
                DataValue::String(key) => obj.iter().any(|(k, _)| *k == *key),
                // Otherwise, convert needle to string for comparison
                _ => {
                    let key_str = needle.to_string();
                    obj.iter().any(|(k, _)| *k == key_str)
                }
            }
        }

        // Other types (including null): always false
        _ => false,
    };

    // Return boolean result
    if result {
        Ok(arena.true_value())
    } else {
        Ok(arena.false_value())
    }
}

/// Evaluates a length operation.
///
/// The length operator returns the number of elements in an array or
/// the number of characters in a string.
///
/// Example:
/// ```json
/// {"length": {"var": "myArray"}}
/// ```
pub fn eval_length<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Validate arguments
    if args.len() != 1 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the argument to get the array or string
    let value = evaluate(args[0], arena)?;

    // Calculate length based on type
    match value {
        DataValue::Array(items) => {
            // For arrays, return the number of elements
            let length = items.len() as i64;
            Ok(
                arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(
                    length,
                ))),
            )
        }
        DataValue::String(s) => {
            // For strings, count Unicode characters (code points), not bytes
            let char_count = s.chars().count() as i64;
            Ok(
                arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(
                    char_count,
                ))),
            )
        }
        DataValue::Null => {
            // For null values, throw an error (following JSONLogic behavior)
            Err(LogicError::InvalidArgumentsError)
        }
        _ => Err(LogicError::InvalidArgumentsError),
    }
}

/// Helper function to parse slice indices
fn parse_slice_index(
    index_value: Option<&DataValue>,
    collection_length: usize,
    default_for_positive_step: usize,
    default_for_negative_step: usize,
) -> Result<usize> {
    match index_value {
        Some(idx) => {
            if let Some(i) = idx.as_i64() {
                if i >= 0 {
                    // Positive index - bound to collection length
                    Ok(i.min(collection_length as i64) as usize)
                } else {
                    // Negative index - count from end
                    let from_end = collection_length as i64 + i;
                    Ok(if from_end < 0 { 0 } else { from_end as usize })
                }
            } else if idx.is_null() {
                // Null index - use default value based on step direction
                Ok(if default_for_positive_step == 0 {
                    default_for_negative_step // For negative step
                } else {
                    default_for_positive_step // For positive step
                })
            } else {
                // Non-numeric, non-null index is an error
                Err(LogicError::NaNError)
            }
        }
        None => {
            // No index provided - use default value based on step direction
            Ok(if default_for_positive_step == 0 {
                default_for_negative_step // For negative step
            } else {
                default_for_positive_step // For positive step
            })
        }
    }
}

/// Evaluates a slice operation on an array
fn eval_array_slice<'a>(
    array: &'a [DataValue<'a>],
    start: Option<&'a DataValue<'a>>,
    end: Option<&'a DataValue<'a>>,
    step: Option<&'a DataValue<'a>>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    let array_len = array.len();

    // Handle empty array case
    if array_len == 0 {
        return Ok(arena.empty_array_value());
    }

    // Parse step value first since start/end defaults depend on step direction
    let step_val = match step {
        Some(s) => {
            if let Some(i) = s.as_i64() {
                if i == 0 {
                    return Err(LogicError::InvalidArgumentsError);
                }
                i as isize
            } else if s.is_null() {
                1
            } else {
                return Err(LogicError::NaNError);
            }
        }
        None => 1,
    };

    // Special case for full array reversal with step -1 (common case)
    // Check for both None and explicit null values
    let is_start_empty = start.is_none() || (start.is_some() && start.unwrap().is_null());
    let is_end_empty = end.is_none() || (end.is_some() && end.unwrap().is_null());

    if is_start_empty && is_end_empty && step_val == -1 {
        let mut result = arena.get_data_value_vec();
        result.reserve(array_len);
        for i in (0..array_len).rev() {
            result.push(array[i].clone());
        }
        return Ok(arena.alloc(DataValue::Array(arena.bump_vec_into_slice(result))));
    }

    // Set default indices based on step direction
    let default_start = if step_val < 0 {
        array_len.saturating_sub(1)
    } else {
        0
    };
    let default_end = if step_val < 0 { 0 } else { array_len };

    // Parse start and end indices
    let start_idx = parse_slice_index(start, array_len, 0, default_start)?;
    let end_idx = parse_slice_index(end, array_len, array_len, default_end)?;

    // Create the result array
    let mut result = arena.get_data_value_vec();
    result.reserve(array_len); // Over-allocate to be safe

    if step_val > 0 {
        // Positive step - go forward
        let mut i = start_idx;
        while i < end_idx && i < array_len {
            result.push(array[i].clone());

            // Handle potential overflow
            if array_len.saturating_sub(i) <= step_val as usize {
                break;
            }
            i = i.saturating_add(step_val as usize);
        }
    } else {
        // Negative step - go backward
        // Start from start_idx, but if it's beyond the array bounds, use the last element
        let mut i = if start_idx >= array_len {
            array_len.saturating_sub(1)
        } else {
            start_idx
        };

        // When going backward, we want to continue until i > end_idx (exclusive end)
        // But we need to handle the case where end_idx >= start_idx (which would be invalid direction)
        while i > end_idx && i < array_len {
            result.push(array[i].clone());

            // Handle potential underflow
            if i < step_val.unsigned_abs() {
                break;
            }
            i = i.saturating_sub(step_val.unsigned_abs());
        }
    }

    Ok(arena.alloc(DataValue::Array(arena.bump_vec_into_slice(result))))
}

/// Evaluates a slice operation on a string
fn eval_string_slice<'a>(
    string: &str,
    start: Option<&'a DataValue<'a>>,
    end: Option<&'a DataValue<'a>>,
    step: Option<&'a DataValue<'a>>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Convert string to character array for proper Unicode handling
    let str_chars: Vec<char> = string.chars().collect();
    let str_len = str_chars.len();

    // Handle empty string case
    if str_len == 0 {
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(""))));
    }

    // Parse step value first since start/end defaults depend on step direction
    let step_val = match step {
        Some(s) => {
            if let Some(i) = s.as_i64() {
                if i == 0 {
                    return Err(LogicError::InvalidArgumentsError);
                }
                i as isize
            } else if s.is_null() {
                1
            } else {
                return Err(LogicError::NaNError);
            }
        }
        None => 1,
    };

    // Special case for full string reversal with step -1 (common case)
    // Check for both None and explicit null values
    let is_start_empty = start.is_none() || (start.is_some() && start.unwrap().is_null());
    let is_end_empty = end.is_none() || (end.is_some() && end.unwrap().is_null());

    if is_start_empty && is_end_empty && step_val == -1 {
        let reversed: String = str_chars.iter().rev().collect();
        return Ok(arena.alloc(DataValue::String(arena.alloc_str(&reversed))));
    }

    // Set default indices based on step direction
    let default_start = if step_val < 0 {
        str_len.saturating_sub(1)
    } else {
        0
    };
    let default_end = if step_val < 0 { 0 } else { str_len };

    // Parse start and end indices
    let start_idx = parse_slice_index(start, str_len, 0, default_start)?;
    let end_idx = parse_slice_index(end, str_len, str_len, default_end)?;

    // Create the result string
    let mut result = String::new();
    result.reserve(str_len); // Over-allocate to be safe

    if step_val > 0 {
        // Positive step - go forward
        let mut i = start_idx;
        while i < end_idx && i < str_len {
            result.push(str_chars[i]);

            // Handle potential overflow
            if str_len.saturating_sub(i) <= step_val as usize {
                break;
            }
            i = i.saturating_add(step_val as usize);
        }
    } else {
        // Negative step - go backward
        // Start from start_idx, but if it's beyond the string bounds, use the last character
        let mut i = if start_idx >= str_len {
            str_len.saturating_sub(1)
        } else {
            start_idx
        };

        // When going backward, we want to continue until i >= end_idx (inclusive end for negative step)
        // This is different from positive step case where end is exclusive
        while i >= end_idx && i < str_len {
            result.push(str_chars[i]);

            // Handle potential underflow
            if i < step_val.unsigned_abs() {
                break;
            }
            i = i.saturating_sub(step_val.unsigned_abs());
        }
    }

    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result))))
}

/// Evaluates a slice operation.
///
/// The slice operator extracts a portion of an array or string based on
/// start, end, and optional step parameters.
///
/// Arguments:
/// - First argument: The array or string to slice
/// - Second argument (optional): The start index (default: 0)
/// - Third argument (optional): The end index (default: length)
/// - Fourth argument (optional): The step (default: 1)
///
/// Example:
/// ```json
/// {"slice": [{"var": "myArray"}, 1, 4]}
/// ```
pub fn eval_slice<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Validate arguments
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the collection (array or string)
    let collection = evaluate(args[0], arena)?;

    // Handle null case
    if collection.is_null() {
        return Ok(arena.null_value());
    }

    // Get slice parameters (start, end, step)
    let start = if args.len() > 1 {
        Some(evaluate(args[1], arena)?)
    } else {
        None
    };

    let end = if args.len() > 2 {
        Some(evaluate(args[2], arena)?)
    } else {
        None
    };

    let step = if args.len() > 3 {
        Some(evaluate(args[3], arena)?)
    } else {
        None
    };

    // Delegate to specialized functions based on collection type
    match collection {
        DataValue::Array(array) => eval_array_slice(array, start, end, step, arena),
        DataValue::String(s) => eval_string_slice(s, start, end, step, arena),
        _ => Err(LogicError::InvalidArgumentsError),
    }
}

/// Helper function to extract a field value from an item for sorting
fn extract_field_value<'a>(
    item: &'a DataValue<'a>,
    extractor: Option<&'a Token<'a>>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if let Some(extractor_token) = extractor {
        // Store current context and key
        let current_context = arena.current_context(0);
        let current_key = arena.last_path_component();

        // Set the item as the context for field extraction
        arena.set_current_context(item, &DataValue::String(""));

        // Evaluate the extractor with the item as context
        let result = evaluate(extractor_token, arena);

        // Restore original context if it exists
        if let (Some(ctx), Some(key)) = (current_context, current_key) {
            arena.set_current_context(ctx, key);
        }

        result
    } else {
        // If no extractor, use the item itself
        Ok(item)
    }
}

/// Helper function to compare values of potentially different types in a consistent order
fn compare_values<'a>(a: &'a DataValue<'a>, b: &'a DataValue<'a>) -> std::cmp::Ordering {
    // First, compare by type according to the JSONLogic specification
    // Type order: null < boolean < number < string < array < object
    let type_order = |val: &DataValue| {
        match val {
            DataValue::Null => 0,
            DataValue::Bool(false) => 1,
            DataValue::Bool(true) => 2,
            DataValue::Number(_) => 3,
            DataValue::String(_) => 4,
            DataValue::Array(_) => 5,
            DataValue::Object(_) => 6,
            DataValue::DateTime(_) => 7, // Additional types
            DataValue::Duration(_) => 8,
        }
    };

    let a_type = type_order(a);
    let b_type = type_order(b);

    // If types are different, sort by type
    if a_type != b_type {
        return a_type.cmp(&b_type);
    }

    // Same types, perform type-specific comparison
    match (a, b) {
        // For numbers, handle NaN safely
        (DataValue::Number(a_num), DataValue::Number(b_num)) => {
            let a_f64 = a_num.as_f64();
            let b_f64 = b_num.as_f64();

            if a_f64.is_nan() && b_f64.is_nan() {
                std::cmp::Ordering::Equal
            } else if a_f64.is_nan() {
                std::cmp::Ordering::Greater // NaN is greater than any number
            } else if b_f64.is_nan() {
                std::cmp::Ordering::Less
            } else {
                a_f64
                    .partial_cmp(&b_f64)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        }

        // For arrays, compare elements lexicographically
        (DataValue::Array(a_arr), DataValue::Array(b_arr)) => {
            // First compare by length
            match a_arr.len().cmp(&b_arr.len()) {
                std::cmp::Ordering::Equal => {
                    // If same length, compare elements
                    for (a_elem, b_elem) in a_arr.iter().zip(b_arr.iter()) {
                        let cmp = compare_values(a_elem, b_elem);
                        if cmp != std::cmp::Ordering::Equal {
                            return cmp;
                        }
                    }
                    std::cmp::Ordering::Equal
                }
                other => other,
            }
        }

        // For objects, compare keys then values
        (DataValue::Object(a_obj), DataValue::Object(b_obj)) => {
            // First compare by number of keys
            match a_obj.len().cmp(&b_obj.len()) {
                std::cmp::Ordering::Equal => {
                    // If same number of keys, compare key-value pairs
                    // Sort keys for consistent comparison
                    let mut a_keys: Vec<&str> = a_obj.iter().map(|(k, _)| *k).collect();
                    let mut b_keys: Vec<&str> = b_obj.iter().map(|(k, _)| *k).collect();
                    a_keys.sort();
                    b_keys.sort();

                    // Compare keys first
                    for (a_key, b_key) in a_keys.iter().zip(b_keys.iter()) {
                        match a_key.cmp(b_key) {
                            std::cmp::Ordering::Equal => continue,
                            other => return other,
                        }
                    }

                    // If keys are identical, compare values
                    for key in a_keys {
                        let a_val = a_obj
                            .iter()
                            .find(|(k, _)| *k == key)
                            .map(|(_, v)| v)
                            .unwrap();
                        let b_val = b_obj
                            .iter()
                            .find(|(k, _)| *k == key)
                            .map(|(_, v)| v)
                            .unwrap();

                        let cmp = compare_values(a_val, b_val);
                        if cmp != std::cmp::Ordering::Equal {
                            return cmp;
                        }
                    }
                    std::cmp::Ordering::Equal
                }
                other => other,
            }
        }

        // For other types, use the default implementation
        _ => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
    }
}

/// Evaluates a sort operation.
///
/// The sort operator sorts an array in ascending or descending order,
/// with optional field extraction for sorting objects.
///
/// Arguments:
/// - First argument: The array to sort
/// - Second argument (optional): Boolean or string indicating sort direction
///   (true/false, "asc"/"desc", etc.)
/// - Third argument (optional): Field extractor function
///
/// Example:
/// ```json
/// {"sort": [{"var": "myArray"}, false, {"var": "fieldName"}]}
/// ```
pub fn eval_sort<'a>(args: &'a [&'a Token<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Validate arguments
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the array
    let array_value = evaluate(args[0], arena)?;

    // Handle special cases for null/empty
    if args.len() == 1 && args[0].is_literal() && array_value.is_null() {
        return Err(LogicError::InvalidArgumentsError);
    } else if array_value.is_null() {
        return Ok(arena.null_value());
    }

    // Verify the value is an array
    if !array_value.is_array() {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Get the array elements
    let arr = array_value.as_array().unwrap();

    // Fast path for empty or single-element arrays (already sorted)
    if arr.is_empty() {
        return Ok(arena.empty_array_value());
    }
    if arr.len() == 1 {
        return Ok(array_value);
    }

    // Parse sort direction from second argument
    let mut ascending = true;
    if args.len() > 1 {
        let dir_value = evaluate(args[1], arena)?;
        if let Some(dir_bool) = dir_value.as_bool() {
            ascending = dir_bool;
        } else if let Some(dir_str) = dir_value.as_str() {
            let dir_lower = dir_str.to_lowercase();
            ascending = !(dir_lower == "desc" || dir_lower == "descending");
        }
    }

    // Get field extractor if provided as third argument
    let field_extractor = if args.len() > 2 { Some(args[2]) } else { None };

    // Clone the array to sort it
    let mut result: Vec<DataValue> = arr.to_vec();

    // Sort the array based on field extractor presence
    if let Some(extractor) = field_extractor {
        // Sort using extracted field values
        result.sort_by(|a, b| {
            // Extract field values for comparison
            let a_field = extract_field_value(a, Some(extractor), arena);
            let b_field = extract_field_value(b, Some(extractor), arena);

            match (a_field, b_field) {
                (Ok(a_val), Ok(b_val)) => {
                    if ascending {
                        compare_values(a_val, b_val)
                    } else {
                        compare_values(b_val, a_val)
                    }
                }
                // If extraction fails, treat elements as equal
                _ => std::cmp::Ordering::Equal,
            }
        });
    } else {
        // Direct item comparison without extraction
        if ascending {
            result.sort_by(|a, b| compare_values(a, b));
        } else {
            result.sort_by(|a, b| compare_values(b, a));
        }
    }

    // Create the sorted array
    Ok(arena.alloc(DataValue::Array(arena.vec_into_slice(result))))
}

#[cfg(test)]
mod tests {
    use crate::logic::datalogic_core::DataLogicCore;
    use crate::logic::operators::arithmetic::ArithmeticOp;
    use crate::logic::operators::array::ArrayOp;
    use crate::logic::token::{OperatorType, Token};
    use crate::logic::Logic;
    use crate::parser::jsonlogic::parse_json;
    use crate::value::DataValue;
    use serde_json::json;

    #[test]
    fn test_map_with_op_syntax() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });

        // Test mapping an array to double each value
        // Create: {"map": [{"var": "numbers"}, {"*": [{"var": ""}, 2]}]}

        // First create {"var": "numbers"}
        let numbers_var_token = Token::variable("numbers", None);
        let numbers_var_ref = arena.alloc(numbers_var_token);

        // Now create {"var": ""}
        let empty_var_token = Token::variable("", None);
        let empty_var_ref = arena.alloc(empty_var_token);

        // Create 2 literal
        let two_token = Token::literal(DataValue::integer(2));
        let two_ref = arena.alloc(two_token);

        // Create {"*": [{"var": ""}, 2]}
        let mul_args = vec![empty_var_ref, two_ref];
        let mul_array_token = Token::ArrayLiteral(mul_args);
        let mul_array_ref = arena.alloc(mul_array_token);

        let mul_token = Token::operator(
            OperatorType::Arithmetic(ArithmeticOp::Multiply),
            mul_array_ref,
        );
        let mul_ref = arena.alloc(mul_token);

        // Create {"map": [{"var": "numbers"}, {"*": [{"var": ""}, 2]}]}
        let map_args = vec![numbers_var_ref, mul_ref];
        let map_array_token = Token::ArrayLiteral(map_args);
        let map_array_ref = arena.alloc(map_array_token);

        let map_token = Token::operator(OperatorType::Array(ArrayOp::Map), map_array_ref);
        let map_ref = arena.alloc(map_token);

        let rule = Logic::new(map_ref, arena);

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
        let arena = core.arena();

        let data_json = json!({
            "numbers": [1, 2, 3, 4, 5, 6, 7, 8]
        });

        // Test filtering for even numbers
        // Create: {"filter": [{"var": "numbers"}, {"==": [{"mod": [{"var": ""}, 2]}, 0]}]}

        // First create {"var": "numbers"}
        let numbers_var_token = Token::variable("numbers", None);
        let numbers_var_ref = arena.alloc(numbers_var_token);

        // Now create {"var": ""}
        let empty_var_token = Token::variable("", None);
        let empty_var_ref = arena.alloc(empty_var_token);

        // Create 2 literal and 0 literal
        let two_token = Token::literal(DataValue::integer(2));
        let two_ref = arena.alloc(two_token);

        let zero_token = Token::literal(DataValue::integer(0));
        let zero_ref = arena.alloc(zero_token);

        // Create {"mod": [{"var": ""}, 2]}
        let mod_args = vec![empty_var_ref, two_ref];
        let mod_array_token = Token::ArrayLiteral(mod_args);
        let mod_array_ref = arena.alloc(mod_array_token);

        let mod_token = Token::operator(
            OperatorType::Arithmetic(ArithmeticOp::Modulo),
            mod_array_ref,
        );
        let mod_ref = arena.alloc(mod_token);

        // Create {"==": [{"mod": [{"var": ""}, 2]}, 0]}
        let equal_args = vec![mod_ref, zero_ref];
        let equal_array_token = Token::ArrayLiteral(equal_args);
        let equal_array_ref = arena.alloc(equal_array_token);

        let equal_token = Token::operator(
            OperatorType::Comparison(crate::logic::operators::comparison::ComparisonOp::Equal),
            equal_array_ref,
        );
        let equal_ref = arena.alloc(equal_token);

        // Create {"filter": [{"var": "numbers"}, {"==": [{"mod": [{"var": ""}, 2]}, 0]}]}
        let filter_args = vec![numbers_var_ref, equal_ref];
        let filter_array_token = Token::ArrayLiteral(filter_args);
        let filter_array_ref = arena.alloc(filter_array_token);

        let filter_token = Token::operator(OperatorType::Array(ArrayOp::Filter), filter_array_ref);
        let filter_ref = arena.alloc(filter_token);

        let rule = Logic::new(filter_ref, arena);

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
        let arena = core.arena();

        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });

        // Test reducing an array to sum its values
        // Create: {"reduce": [{"var": "numbers"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}

        // First create {"var": "numbers"}
        let numbers_var_token = Token::variable("numbers", None);
        let numbers_var_ref = arena.alloc(numbers_var_token);

        // Create {"var": "current"} and {"var": "accumulator"}
        let current_var_token = Token::variable("current", None);
        let current_var_ref = arena.alloc(current_var_token);

        let accumulator_var_token = Token::variable("accumulator", None);
        let accumulator_var_ref = arena.alloc(accumulator_var_token);

        // Create {"+": [{"var": "current"}, {"var": "accumulator"}]}
        let add_args = vec![current_var_ref, accumulator_var_ref];
        let add_array_token = Token::ArrayLiteral(add_args);
        let add_array_ref = arena.alloc(add_array_token);

        let add_token = Token::operator(OperatorType::Arithmetic(ArithmeticOp::Add), add_array_ref);
        let add_ref = arena.alloc(add_token);

        // Create initial value 0
        let zero_token = Token::literal(DataValue::integer(0));
        let zero_ref = arena.alloc(zero_token);

        // Create {"reduce": [{"var": "numbers"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}
        let reduce_args = vec![numbers_var_ref, add_ref, zero_ref];
        let reduce_array_token = Token::ArrayLiteral(reduce_args);
        let reduce_array_ref = arena.alloc(reduce_array_token);

        let reduce_token = Token::operator(OperatorType::Array(ArrayOp::Reduce), reduce_array_ref);
        let reduce_ref = arena.alloc(reduce_token);

        let rule = Logic::new(reduce_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(10)); // 1 + 2 + 3 + 4 = 10

        // Test with empty array - should return initial value
        let data_json = json!({
            "numbers": []
        });
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(0));

        // Test with different initial value
        // Create: {"reduce": [{"var": "numbers"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 10]}

        // Create initial value 10
        let ten_token = Token::literal(DataValue::integer(10));
        let ten_ref = arena.alloc(ten_token);

        // Create {"reduce": [{"var": "numbers"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 10]}
        let reduce_args = vec![numbers_var_ref, add_ref, ten_ref];
        let reduce_array_token = Token::ArrayLiteral(reduce_args);
        let reduce_array_ref = arena.alloc(reduce_array_token);

        let reduce_token = Token::operator(OperatorType::Array(ArrayOp::Reduce), reduce_array_ref);
        let reduce_ref = arena.alloc(reduce_token);

        let rule = Logic::new(reduce_ref, arena);

        let data_json = json!({
            "numbers": [1, 2, 3, 4]
        });
        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!(20)); // 10 + 1 + 2 + 3 + 4 = 20
    }

    #[test]
    fn test_length_operator() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Test array length
        // Create: {"length": {"var": "array"}}
        let array_var_token = Token::variable("array", None);
        let array_var_ref = arena.alloc(array_var_token);

        let length_token = Token::operator(OperatorType::Array(ArrayOp::Length), array_var_ref);
        let length_ref = arena.alloc(length_token);

        let rule = Logic::new(length_ref, arena);

        let json_data = json!({"array": [1, 2, 3, 4, 5]});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!(5));

        // Test string length
        // Create: {"length": {"var": "string"}}
        let string_var_token = Token::variable("string", None);
        let string_var_ref = arena.alloc(string_var_token);

        let length_token = Token::operator(OperatorType::Array(ArrayOp::Length), string_var_ref);
        let length_ref = arena.alloc(length_token);

        let rule = Logic::new(length_ref, arena);

        let json_data = json!({"string": "hello"});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!(5));

        // Test Unicode string length
        // Create: {"length": {"var": "unicode"}}
        let unicode_var_token = Token::variable("unicode", None);
        let unicode_var_ref = arena.alloc(unicode_var_token);

        let length_token = Token::operator(OperatorType::Array(ArrayOp::Length), unicode_var_ref);
        let length_ref = arena.alloc(length_token);

        let rule = Logic::new(length_ref, arena);

        let json_data = json!({"unicode": "👋🌍"});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!(2));

        // Also test with JSON parsing for compatibility
        let json_rule = json!({"length": {"var": "array"}});
        let rule = Logic::new(parse_json(&json_rule, core.arena()).unwrap(), core.arena());
        let json_data = json!({"array": [1, 2, 3, 4, 5]});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn test_slice_operator() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Test array slice with start and end
        // Create: {"slice": [{"var": "array"}, 1, 3]}
        let array_var_token = Token::variable("array", None);
        let array_var_ref = arena.alloc(array_var_token);

        let start_token = Token::literal(DataValue::integer(1));
        let start_ref = arena.alloc(start_token);

        let end_token = Token::literal(DataValue::integer(3));
        let end_ref = arena.alloc(end_token);

        let slice_args = vec![array_var_ref, start_ref, end_ref];
        let slice_array_token = Token::ArrayLiteral(slice_args);
        let slice_array_ref = arena.alloc(slice_array_token);

        let slice_token = Token::operator(OperatorType::Array(ArrayOp::Slice), slice_array_ref);
        let slice_ref = arena.alloc(slice_token);

        let rule = Logic::new(slice_ref, arena);

        let json_data = json!({"array": [1, 2, 3, 4, 5]});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!([2, 3]));

        // Test negative indices
        // Create: {"slice": [{"var": "array"}, -3, -1]}
        let neg_start_token = Token::literal(DataValue::integer(-3));
        let neg_start_ref = arena.alloc(neg_start_token);

        let neg_end_token = Token::literal(DataValue::integer(-1));
        let neg_end_ref = arena.alloc(neg_end_token);

        let slice_args = vec![array_var_ref, neg_start_ref, neg_end_ref];
        let slice_array_token = Token::ArrayLiteral(slice_args);
        let slice_array_ref = arena.alloc(slice_array_token);

        let slice_token = Token::operator(OperatorType::Array(ArrayOp::Slice), slice_array_ref);
        let slice_ref = arena.alloc(slice_token);

        let rule = Logic::new(slice_ref, arena);

        let json_data = json!({"array": [1, 2, 3, 4, 5]});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!([3, 4]));

        // Test with step
        // Create: {"slice": [{"var": "array"}, 0, 5, 2]}
        let start_zero_token = Token::literal(DataValue::integer(0));
        let start_zero_ref = arena.alloc(start_zero_token);

        let end_five_token = Token::literal(DataValue::integer(5));
        let end_five_ref = arena.alloc(end_five_token);

        let step_token = Token::literal(DataValue::integer(2));
        let step_ref = arena.alloc(step_token);

        let slice_args = vec![array_var_ref, start_zero_ref, end_five_ref, step_ref];
        let slice_array_token = Token::ArrayLiteral(slice_args);
        let slice_array_ref = arena.alloc(slice_array_token);

        let slice_token = Token::operator(OperatorType::Array(ArrayOp::Slice), slice_array_ref);
        let slice_ref = arena.alloc(slice_token);

        let rule = Logic::new(slice_ref, arena);

        let json_data = json!({"array": [1, 2, 3, 4, 5]});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!([1, 3, 5]));

        // Test string slicing
        // Create: {"slice": [{"var": "string"}, 0, 3]}
        let string_var_token = Token::variable("string", None);
        let string_var_ref = arena.alloc(string_var_token);

        let slice_args = vec![string_var_ref, start_zero_ref, end_ref];
        let slice_array_token = Token::ArrayLiteral(slice_args);
        let slice_array_ref = arena.alloc(slice_array_token);

        let slice_token = Token::operator(OperatorType::Array(ArrayOp::Slice), slice_array_ref);
        let slice_ref = arena.alloc(slice_token);

        let rule = Logic::new(slice_ref, arena);

        let json_data = json!({"string": "hello world"});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!("hel"));
    }

    #[test]
    fn test_sort_operator() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Test sort array in ascending order (default)
        // Create: {"sort": [{"var": "array"}]}
        let array_var_token = Token::variable("array", None);
        let array_var_ref = arena.alloc(array_var_token);

        let sort_args = vec![array_var_ref];
        let sort_array_token = Token::ArrayLiteral(sort_args);
        let sort_array_ref = arena.alloc(sort_array_token);

        let sort_token = Token::operator(OperatorType::Array(ArrayOp::Sort), sort_array_ref);
        let sort_ref = arena.alloc(sort_token);

        let rule = Logic::new(sort_ref, arena);

        let json_data = json!({"array": [5, 3, 1, 4, 2]});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!([1, 2, 3, 4, 5]));

        // Test sort array in descending order
        // Create: {"sort": [{"var": "array"}, false]}
        let false_token = Token::literal(DataValue::Bool(false));
        let false_ref = arena.alloc(false_token);

        let sort_args = vec![array_var_ref, false_ref];
        let sort_array_token = Token::ArrayLiteral(sort_args);
        let sort_array_ref = arena.alloc(sort_array_token);

        let sort_token = Token::operator(OperatorType::Array(ArrayOp::Sort), sort_array_ref);
        let sort_ref = arena.alloc(sort_token);

        let rule = Logic::new(sort_ref, arena);

        let json_data = json!({"array": [5, 3, 1, 4, 2]});
        let result = core.apply(&rule, &json_data).unwrap();
        assert_eq!(result, json!([5, 4, 3, 2, 1]));

        // Test sort array of objects by field
        // Create: {"sort": [{"var": "people"}, true, {"var": "age"}]}
        let people_var_token = Token::variable("people", None);
        let people_var_ref = arena.alloc(people_var_token);

        let true_token = Token::literal(DataValue::Bool(true));
        let true_ref = arena.alloc(true_token);

        let age_var_token = Token::variable("age", None);
        let age_var_ref = arena.alloc(age_var_token);

        let sort_args = vec![people_var_ref, true_ref, age_var_ref];
        let sort_array_token = Token::ArrayLiteral(sort_args);
        let sort_array_ref = arena.alloc(sort_array_token);

        let sort_token = Token::operator(OperatorType::Array(ArrayOp::Sort), sort_array_ref);
        let sort_ref = arena.alloc(sort_token);

        let rule = Logic::new(sort_ref, arena);

        let json_data = json!({
            "people": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25},
                {"name": "Charlie", "age": 35}
            ]
        });
        let result = core.apply(&rule, &json_data).unwrap();

        // Verify the sorted order
        let result_array = result.as_array().unwrap();
        assert_eq!(result_array.len(), 3);
        assert_eq!(
            result_array[0].as_object().unwrap()["name"]
                .as_str()
                .unwrap(),
            "Bob"
        );
        assert_eq!(
            result_array[1].as_object().unwrap()["name"]
                .as_str()
                .unwrap(),
            "Alice"
        );
        assert_eq!(
            result_array[2].as_object().unwrap()["name"]
                .as_str()
                .unwrap(),
            "Charlie"
        );
    }

    #[test]
    fn test_map_with_objects() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Test mapping over object properties
        let data_json = json!({
            "person": {
                "name": "Alice",
                "age": 30,
                "city": "New York"
            }
        });

        // Create: {"map": [{"var": "person"}, {"var": ""}]}
        let person_var_token = Token::variable("person", None);
        let person_var_ref = arena.alloc(person_var_token);

        let empty_var_token = Token::variable("", None);
        let empty_var_ref = arena.alloc(empty_var_token);

        let map_args = vec![person_var_ref, empty_var_ref];
        let map_array_token = Token::ArrayLiteral(map_args);
        let map_array_ref = arena.alloc(map_array_token);

        let map_token = Token::operator(OperatorType::Array(ArrayOp::Map), map_array_ref);
        let map_ref = arena.alloc(map_token);

        let rule = Logic::new(map_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        println!("Values result: {result:?}");

        // Verify the values are returned
        let values: Vec<serde_json::Value> = serde_json::from_value(result).unwrap();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&json!(30)));
        assert!(values.contains(&json!("Alice")));
        assert!(values.contains(&json!("New York")));

        // Test with empty object
        let data_json = json!({
            "empty": {}
        });

        // Create: {"map": [{"var": "empty"}, {"var": ""}]}
        let empty_obj_var_token = Token::variable("empty", None);
        let empty_obj_var_ref = arena.alloc(empty_obj_var_token);

        let map_args = vec![empty_obj_var_ref, empty_var_ref];
        let map_array_token = Token::ArrayLiteral(map_args);
        let map_array_ref = arena.alloc(map_array_token);

        let map_token = Token::operator(OperatorType::Array(ArrayOp::Map), map_array_ref);
        let map_ref = arena.alloc(map_token);

        let rule = Logic::new(map_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_map_with_single_values() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        // Test mapping over a single number
        let data_json = json!({
            "number": 42
        });

        // Create: {"map": [{"var": "number"}, {"var": ""}]}
        let number_var_token = Token::variable("number", None);
        let number_var_ref = arena.alloc(number_var_token);

        let empty_var_token = Token::variable("", None);
        let empty_var_ref = arena.alloc(empty_var_token);

        let map_args = vec![number_var_ref, empty_var_ref];
        let map_array_token = Token::ArrayLiteral(map_args);
        let map_array_ref = arena.alloc(map_array_token);

        let map_token = Token::operator(OperatorType::Array(ArrayOp::Map), map_array_ref);
        let map_ref = arena.alloc(map_token);

        let rule = Logic::new(map_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();
        assert_eq!(result, json!([42]));

        // Test mapping over a single string
        let data_json = json!({
            "string": "hello"
        });

        // Create: {"map": [{"var": "string"}, {"var": ""}]}
        let string_var_token = Token::variable("string", None);
        let string_var_ref = arena.alloc(string_var_token);

        let empty_var_token = Token::variable("", None);
        let empty_var_ref = arena.alloc(empty_var_token);

        let map_args = vec![string_var_ref, empty_var_ref];
        let map_array_token = Token::ArrayLiteral(map_args);
        let map_array_ref = arena.alloc(map_array_token);

        let map_token = Token::operator(OperatorType::Array(ArrayOp::Map), map_array_ref);
        let map_ref = arena.alloc(map_token);

        let rule = Logic::new(map_ref, arena);

        let result = core.apply(&rule, &data_json).unwrap();

        // The result should be an array with 1 element (the string itself)
        assert_eq!(result, json!(["hello"]));
    }
}
