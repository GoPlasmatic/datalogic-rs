use serde_json::Value;

use std::cmp::Ordering;
use std::collections::HashMap;

use super::helpers::is_truthy;
use crate::constants::INVALID_ARGS;
use crate::context::{ACCUMULATOR_KEY, CURRENT_KEY, INDEX_KEY, KEY_KEY};
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// The `merge` operator - combines multiple arrays into one.
///
/// # Syntax
/// ```json
/// {"merge": [array1, array2, ...]}
/// ```
///
/// # Arguments
/// Any number of arrays or values to merge together.
///
/// # Behavior
/// - Arrays are flattened one level (elements are extracted)
/// - Non-array values are added as-is
/// - `null` values are filtered out from the result
///
/// # Example
/// ```json
/// {"merge": [[1, 2], [3, 4], 5]}
/// ```
/// Returns: `[1, 2, 3, 4, 5]`
///
/// # Example with nulls
/// ```json
/// {"merge": [[1, null, 2], [3]]}
/// ```
/// Returns: `[1, 2, 3]` (nulls filtered)
#[inline]
pub fn evaluate_merge(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let mut result = Vec::new();

    for arg in args {
        let value = engine.evaluate_node(arg, context)?;
        match value {
            Value::Array(arr) => {
                // Filter out null values when extending
                result.extend(arr.iter().filter(|v| !v.is_null()).cloned())
            }
            Value::Null => {
                // Skip null values entirely
            }
            v => result.push(v.clone()),
        }
    }

    Ok(Value::Array(result))
}

/// The `map` operator - transforms each element in an array or object.
///
/// # Syntax
/// ```json
/// {"map": [collection, transformation]}
/// ```
///
/// # Arguments
/// 1. An array or object to iterate over
/// 2. A transformation logic to apply to each element
///
/// # Context
/// During iteration, the current item becomes the context, and metadata is available:
/// - `{"var": ""}` or `{"var": "."}` - current item value
/// - `{"var": "index"}` - current index (arrays) or key (objects)
/// - `{"var": "key"}` - current key (objects only)
///
/// # Example with Array
/// ```json
/// {
///   "map": [
///     [{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}],
///     {"var": "name"}
///   ]
/// }
/// ```
/// Returns: `["Alice", "Bob"]`
///
/// # Example with Object
/// ```json
/// {
///   "map": [
///     {"a": 1, "b": 2, "c": 3},
///     {"*": [{"var": ""}, 2]}
///   ]
/// }
/// ```
/// Returns: `[2, 4, 6]`
#[inline]
pub fn evaluate_map(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let logic = &args[1];

    match &collection {
        Value::Array(arr) => {
            let mut results = Vec::with_capacity(arr.len());

            for (index, item) in arr.iter().enumerate() {
                // Use push_with_index to avoid HashMap allocation
                context.push_with_index(item.clone(), index);
                let result = engine.evaluate_node(logic, context)?;
                results.push(result);
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Object(obj) => {
            let mut results = Vec::with_capacity(obj.len());

            for (index, (key, value)) in obj.iter().enumerate() {
                let mut metadata = HashMap::with_capacity(2);
                metadata.insert(KEY_KEY.to_string(), Value::String(key.clone()));
                metadata.insert(INDEX_KEY.to_string(), Value::Number(index.into()));

                context.push_with_metadata(value.clone(), metadata);
                let result = engine.evaluate_node(logic, context)?;
                results.push(result);
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Null => Ok(Value::Array(vec![])),
        // For primitive values (number, string, bool), treat as single-element collection
        _ => {
            // Use push_with_index to avoid HashMap allocation
            context.push_with_index(collection, 0);
            let result = engine.evaluate_node(logic, context)?;
            context.pop();

            Ok(Value::Array(vec![result]))
        }
    }
}

/// The `filter` operator - selects elements that match a condition.
///
/// # Syntax
/// ```json
/// {"filter": [collection, condition]}
/// ```
///
/// # Arguments
/// 1. An array or object to filter
/// 2. A condition logic that returns truthy/falsy for each element
///
/// # Context
/// Similar to `map`, each item becomes the context with index/key metadata.
///
/// # Example with Array
/// ```json
/// {
///   "filter": [
///     [{"age": 17}, {"age": 25}, {"age": 30}],
///     {">=": [{"var": "age"}, 18]}
///   ]
/// }
/// ```
/// Returns: `[{"age": 25}, {"age": 30}]`
///
/// # Example with Object
/// ```json
/// {
///   "filter": [
///     {"a": 10, "b": 5, "c": 20},
///     {">": [{"var": ""}, 8]}
///   ]
/// }
/// ```
/// Returns: `{"a": 10, "c": 20}`
#[inline]
pub fn evaluate_filter(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let predicate = &args[1];

    match &collection {
        Value::Array(arr) => {
            let mut results = Vec::new();

            for (index, item) in arr.iter().enumerate() {
                // Use push_with_index to avoid HashMap allocation
                context.push_with_index(item.clone(), index);
                let keep = engine.evaluate_node(predicate, context)?;
                context.pop();

                if is_truthy(&keep, engine) {
                    results.push(item.clone());
                }
            }

            Ok(Value::Array(results))
        }
        Value::Object(obj) => {
            let mut result_obj = serde_json::Map::new();

            for (index, (key, value)) in obj.iter().enumerate() {
                let mut metadata = HashMap::with_capacity(2);
                metadata.insert(KEY_KEY.to_string(), Value::String(key.clone()));
                metadata.insert(INDEX_KEY.to_string(), Value::Number(index.into()));

                context.push_with_metadata(value.clone(), metadata);
                let keep = engine.evaluate_node(predicate, context)?;
                context.pop();

                if is_truthy(&keep, engine) {
                    result_obj.insert(key.clone(), value.clone());
                }
            }

            Ok(Value::Object(result_obj))
        }
        Value::Null => Ok(Value::Array(vec![])),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.to_string())),
    }
}

/// The `reduce` operator - reduces a collection to a single value.
///
/// # Syntax
/// ```json
/// {"reduce": [collection, logic, initial_value]}
/// ```
///
/// # Arguments
/// 1. An array or object to reduce
/// 2. Reduction logic with access to `current` and `accumulator`
/// 3. Initial value for the accumulator
///
/// # Context Variables
/// - `{"var": "current"}` - current element value
/// - `{"var": "accumulator"}` - accumulated value
/// - `{"var": "index"}` - current index or key
///
/// # Example - Sum Array
/// ```json
/// {
///   "reduce": [
///     [1, 2, 3, 4],
///     {"+": [{"var": "accumulator"}, {"var": "current"}]},
///     0
///   ]
/// }
/// ```
/// Returns: `10`
#[inline]
pub fn evaluate_reduce(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 3 {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    let array = engine.evaluate_node(&args[0], context)?;
    let logic = &args[1];
    let initial = engine.evaluate_node(&args[2], context)?;

    match &array {
        Value::Array(arr) => {
            if arr.is_empty() {
                return Ok(initial);
            }

            let mut accumulator = initial;

            for current in arr {
                let mut frame_data = serde_json::Map::with_capacity(2);
                frame_data.insert(CURRENT_KEY.to_string(), current.clone());
                frame_data.insert(ACCUMULATOR_KEY.to_string(), accumulator.clone());

                context.push(Value::Object(frame_data));
                accumulator = engine.evaluate_node(logic, context)?;
                context.pop();
            }

            Ok(accumulator)
        }
        Value::Null => Ok(initial),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.to_string())),
    }
}

/// The `all` operator - checks if all elements satisfy a condition.
///
/// # Syntax
/// ```json
/// {"all": [collection, condition]}
/// ```
///
/// # Arguments
/// 1. An array or object to test
/// 2. A condition to evaluate for each element
///
/// # Returns
/// - `true` if all elements satisfy the condition
/// - `true` if the collection is empty
/// - `false` if any element fails the condition
///
/// # Example
/// ```json
/// {
///   "all": [
///     [10, 20, 30],
///     {">": [{"var": ""}, 5]}
///   ]
/// }
/// ```
/// Returns: `true` (all are greater than 5)
#[inline]
pub fn evaluate_all(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let predicate = &args[1];

    match &collection {
        Value::Array(arr) if !arr.is_empty() => {
            for (index, item) in arr.iter().enumerate() {
                // Use push_with_index to avoid HashMap allocation
                context.push_with_index(item.clone(), index);
                let result = engine.evaluate_node(predicate, context)?;
                context.pop();

                if !is_truthy(&result, engine) {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        Value::Array(arr) if arr.is_empty() => Ok(Value::Bool(false)),
        Value::Null => Ok(Value::Bool(false)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.to_string())),
    }
}

/// The `some` operator - checks if any element satisfies a condition.
///
/// # Syntax
/// ```json
/// {"some": [collection, condition]}
/// ```
///
/// # Arguments
/// 1. An array or object to test
/// 2. A condition to evaluate for each element
///
/// # Returns
/// - `true` if any element satisfies the condition
/// - `false` if no elements satisfy or collection is empty
///
/// # Example
/// ```json
/// {
///   "some": [
///     [{"status": "pending"}, {"status": "active"}],
///     {"==": [{"var": "status"}, "active"]}
///   ]
/// }
/// ```
/// Returns: `true`
#[inline]
pub fn evaluate_some(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let predicate = &args[1];

    match &collection {
        Value::Array(arr) => {
            for (index, item) in arr.iter().enumerate() {
                // Use push_with_index to avoid HashMap allocation
                context.push_with_index(item.clone(), index);
                let result = engine.evaluate_node(predicate, context)?;
                context.pop();

                if is_truthy(&result, engine) {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        Value::Null => Ok(Value::Bool(false)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.to_string())),
    }
}

/// The `none` operator - checks if no elements satisfy a condition.
///
/// # Syntax
/// ```json
/// {"none": [collection, condition]}
/// ```
///
/// # Arguments
/// 1. An array or object to test
/// 2. A condition to evaluate for each element
///
/// # Returns
/// - `true` if no elements satisfy the condition
/// - `true` if the collection is empty
/// - `false` if any element satisfies the condition
///
/// # Example
/// ```json
/// {
///   "none": [
///     [1, 3, 5, 7],
///     {"==": [{"%": [{"var": ""}, 2]}, 0]}
///   ]
/// }
/// ```
/// Returns: `true` (none are even)
#[inline]
pub fn evaluate_none(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let predicate = &args[1];

    match &collection {
        Value::Array(arr) => {
            for (index, item) in arr.iter().enumerate() {
                // Use push_with_index to avoid HashMap allocation
                context.push_with_index(item.clone(), index);
                let result = engine.evaluate_node(predicate, context)?;
                context.pop();

                if is_truthy(&result, engine) {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        Value::Null => Ok(Value::Bool(true)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.to_string())),
    }
}

/// The `sort` operator - sorts array elements.
///
/// # Syntax
/// ```json
/// {"sort": [array, accessor]}
/// ```
///
/// # Arguments
/// 1. An array to sort
/// 2. Optional: An accessor to extract sort key from each element
///
/// # Behavior
/// - Without accessor: sorts primitives directly
/// - With accessor: sorts by the extracted value
/// - Sorts in ascending order
/// - Maintains stable sort order
/// - Handles mixed types (nulls first, then bools, numbers, strings, arrays, objects)
///
/// # Example
/// ```json
/// {
///   "sort": [
///     [{"name": "Charlie", "age": 30}, {"name": "Alice", "age": 25}],
///     {"var": "name"}
///   ]
/// }
/// ```
/// Returns: Sorted by name alphabetically
#[inline]
pub fn evaluate_sort(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    // Check if the first argument is a Value node containing null
    if let CompiledNode::Value { value, .. } = &args[0]
        && value.is_null()
    {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    // Evaluate the array
    let array_value = engine.evaluate_node(&args[0], context)?;

    let mut array = match array_value {
        Value::Array(arr) => arr,
        Value::Null => return Ok(Value::Null), // Missing variable returns null
        _ => return Err(Error::InvalidArguments(INVALID_ARGS.to_string())),
    };

    // Get sort direction (default ascending)
    let ascending = if args.len() > 1 {
        let dir = engine.evaluate_node(&args[1], context)?;
        match dir {
            Value::Bool(b) => b,
            _ => true, // Default to ascending for invalid direction
        }
    } else {
        true
    };

    // Check if we have a field extractor (for sorting objects)
    let has_extractor = args.len() > 2;

    if has_extractor {
        // Sort objects by extracted field
        let extractor = &args[2];

        // Create a vector of (index, value, extracted_value) tuples
        let mut items_with_values: Vec<(usize, Value, Value)> = Vec::new();

        for (index, item) in array.iter().enumerate() {
            context.push(item.clone());
            let extracted = engine.evaluate_node(extractor, context)?;
            context.pop();
            items_with_values.push((index, item.clone(), extracted));
        }

        // Sort by extracted values
        items_with_values.sort_by(|a, b| {
            let cmp = compare_values(&a.2, &b.2);
            if ascending { cmp } else { cmp.reverse() }
        });

        // Rebuild array from sorted items
        array = items_with_values
            .into_iter()
            .map(|(_, item, _)| item)
            .collect();
    } else {
        // Sort primitive values directly
        array.sort_by(|a, b| {
            let cmp = compare_values(a, b);
            if ascending { cmp } else { cmp.reverse() }
        });
    }

    Ok(Value::Array(array))
}

/// The `slice` operator - extracts a portion of an array or string.
///
/// # Syntax
/// ```json
/// {"slice": [sequence, start, end]}
/// ```
///
/// # Arguments
/// 1. An array or string to slice
/// 2. Start index (inclusive)
/// 3. Optional: End index (exclusive)
///
/// # Behavior
/// - Negative indices count from the end (-1 is last element)
/// - If end is omitted, slices to the end
/// - Returns empty result if indices are out of bounds
/// - Works with both arrays and strings
///
/// # Example with Array
/// ```json
/// {
///   "slice": [
///     ["a", "b", "c", "d", "e"],
///     1,
///     3
///   ]
/// }
/// ```
/// Returns: `["b", "c"]`
///
/// # Example with String
/// ```json
/// {
///   "slice": [
///     "hello world",
///     0,
///     5
///   ]
/// }
/// ```
/// Returns: `"hello"`
#[inline]
pub fn evaluate_slice(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
    }

    // Evaluate the collection
    let collection = engine.evaluate_node(&args[0], context)?;

    // Handle null/missing values
    if collection == Value::Null {
        return Ok(Value::Null);
    }

    // Get start index (default to 0 or end for negative step)
    let start = if args.len() > 1 {
        let start_val = engine.evaluate_node(&args[1], context)?;
        match start_val {
            Value::Number(n) => n.as_i64(),
            Value::Null => None,
            _ => return Err(Error::InvalidArguments("NaN".to_string())),
        }
    } else {
        None
    };

    // Get end index (default to length)
    let end = if args.len() > 2 {
        let end_val = engine.evaluate_node(&args[2], context)?;
        match end_val {
            Value::Number(n) => n.as_i64(),
            Value::Null => None,
            _ => return Err(Error::InvalidArguments("NaN".to_string())),
        }
    } else {
        None
    };

    // Get step (default to 1)
    let step = if args.len() > 3 {
        let step_val = engine.evaluate_node(&args[3], context)?;
        match step_val {
            Value::Number(n) => {
                let s = n.as_i64().unwrap_or(1);
                if s == 0 {
                    return Err(Error::InvalidArguments(INVALID_ARGS.to_string()));
                }
                s
            }
            _ => 1,
        }
    } else {
        1
    };

    match collection {
        Value::Array(arr) => {
            let len = arr.len() as i64;
            let result = slice_sequence(&arr, len, start, end, step);
            Ok(Value::Array(result))
        }
        Value::String(s) => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as i64;
            let char_values: Vec<Value> = chars
                .into_iter()
                .map(|c| Value::String(c.to_string()))
                .collect();
            let sliced = slice_sequence(&char_values, len, start, end, step);
            let result_string: String = sliced
                .into_iter()
                .filter_map(|v| {
                    if let Value::String(s) = v {
                        Some(s)
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Value::String(result_string))
        }
        _ => Err(Error::InvalidArguments(INVALID_ARGS.to_string())),
    }
}

// Helper function to compare JSON values for sorting
fn compare_values(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        // Null is less than everything
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,

        // Booleans
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),

        // Numbers
        (Value::Number(a), Value::Number(b)) => {
            let a_f = a.as_f64().unwrap_or(0.0);
            let b_f = b.as_f64().unwrap_or(0.0);
            if a_f < b_f {
                Ordering::Less
            } else if a_f > b_f {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }

        // Strings
        (Value::String(a), Value::String(b)) => a.cmp(b),

        // Mixed types - use type order: null < bool < number < string < array < object
        (Value::Bool(_), Value::Number(_)) => Ordering::Less,
        (Value::Bool(_), Value::String(_)) => Ordering::Less,
        (Value::Bool(_), Value::Array(_)) => Ordering::Less,
        (Value::Bool(_), Value::Object(_)) => Ordering::Less,

        (Value::Number(_), Value::Bool(_)) => Ordering::Greater,
        (Value::Number(_), Value::String(_)) => Ordering::Less,
        (Value::Number(_), Value::Array(_)) => Ordering::Less,
        (Value::Number(_), Value::Object(_)) => Ordering::Less,

        (Value::String(_), Value::Bool(_)) => Ordering::Greater,
        (Value::String(_), Value::Number(_)) => Ordering::Greater,
        (Value::String(_), Value::Array(_)) => Ordering::Less,
        (Value::String(_), Value::Object(_)) => Ordering::Less,

        (Value::Array(_), _) => Ordering::Greater,
        (_, Value::Array(_)) => Ordering::Less,

        // Objects are greater than everything else (except other objects)
        (Value::Object(_), Value::Object(_)) => Ordering::Equal,
        (Value::Object(_), _) => Ordering::Greater,
    }
}

// Helper function to slice a sequence with start, end, and step
fn slice_sequence(
    arr: &[Value],
    len: i64,
    start: Option<i64>,
    end: Option<i64>,
    step: i64,
) -> Vec<Value> {
    let mut result = Vec::new();

    // Normalize indices with overflow protection
    let (actual_start, actual_end) = if step > 0 {
        let s = normalize_index(start.unwrap_or(0), len);
        let e = normalize_index(end.unwrap_or(len), len);
        (s, e)
    } else {
        // For negative step, defaults are reversed
        // Use saturating_sub to prevent underflow
        let default_start = len.saturating_sub(1);
        let s = normalize_index(start.unwrap_or(default_start), len);
        let e = if let Some(e) = end {
            normalize_index(e, len)
        } else {
            -1 // Go all the way to the beginning
        };
        (s, e)
    };

    // Collect elements with overflow-safe iteration
    if step > 0 {
        let mut i = actual_start;
        while i < actual_end && i < len {
            if i >= 0 && (i as usize) < arr.len() {
                result.push(arr[i as usize].clone());
            }
            // Use saturating_add to prevent overflow
            i = i.saturating_add(step);
            // Break if we've wrapped around
            if step > 0 && i < actual_start {
                break;
            }
        }
    } else {
        let mut i = actual_start;
        while i > actual_end && i >= 0 && i < len {
            if (i as usize) < arr.len() {
                result.push(arr[i as usize].clone());
            }
            // Use saturating_add for negative step (step is negative)
            let next_i = i.saturating_add(step);
            // Break if we've wrapped around
            if step < 0 && next_i > i {
                break;
            }
            i = next_i;
        }
    }

    result
}

// Helper function to normalize slice indices with overflow protection
fn normalize_index(index: i64, len: i64) -> i64 {
    if index < 0 {
        // Use saturating_add to prevent overflow when index is very negative
        let adjusted = len.saturating_add(index);
        adjusted.max(0)
    } else {
        index.min(len)
    }
}
