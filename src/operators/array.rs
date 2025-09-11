use serde_json::Value;

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::context::{index_key, key_key};
use crate::value_helpers::is_truthy;
use crate::{ContextStack, Error, Evaluator, Operator, Result};

/// Merge operator - merges arrays
pub struct MergeOperator;

impl Operator for MergeOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        let mut result = Vec::new();

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;
            match value {
                Value::Array(arr) => result.extend(arr.iter().cloned()),
                v => result.push(v.clone()),
            }
        }

        Ok(Value::Array(result))
    }
}

/// Map operator - transforms array/object elements
pub struct MapOperator;

impl Operator for MapOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let logic = &args[1];

        match &collection {
            Value::Array(arr) => {
                let mut results = Vec::with_capacity(arr.len());

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let result = evaluator.evaluate(logic, context)?;
                    results.push(result);
                    context.pop();
                }

                Ok(Value::Array(results))
            }
            Value::Object(obj) => {
                let mut results = Vec::with_capacity(obj.len());

                for (index, (key, value)) in obj.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(2);
                    metadata.insert(key_key().clone(), Value::String(key.clone()));
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(value.clone(), metadata);
                    let result = evaluator.evaluate(logic, context)?;
                    results.push(result);
                    context.pop();
                }

                Ok(Value::Array(results))
            }
            Value::Null => Ok(Value::Array(vec![])),
            // For primitive values (number, string, bool), treat as single-element collection
            _ => {
                let mut metadata = HashMap::with_capacity(1);
                metadata.insert(index_key().clone(), Value::Number(0.into()));

                context.push_with_metadata(collection, metadata);
                let result = evaluator.evaluate(logic, context)?;
                context.pop();

                Ok(Value::Array(vec![result]))
            }
        }
    }
}

/// Filter operator - filters array/object elements
pub struct FilterOperator;

impl Operator for FilterOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match &collection {
            Value::Array(arr) => {
                // Fast path for constant predicates
                match predicate {
                    Value::Bool(false) => return Ok(Value::Array(vec![])),
                    Value::Bool(true) => return Ok(Value::Array(arr.clone())),
                    _ => {}
                }

                let mut results = Vec::new();

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let keep = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(&keep) {
                        results.push(item.clone());
                    }
                }

                Ok(Value::Array(results))
            }
            Value::Object(obj) => {
                let mut result_obj = serde_json::Map::new();

                for (index, (key, value)) in obj.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(2);
                    metadata.insert(key_key().clone(), Value::String(key.clone()));
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(value.clone(), metadata);
                    let keep = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(&keep) {
                        result_obj.insert(key.clone(), value.clone());
                    }
                }

                Ok(Value::Object(result_obj))
            }
            Value::Null => Ok(Value::Array(vec![])),
            _ => Err(Error::InvalidArguments("Invalid Arguments".to_string())),
        }
    }
}

/// Reduce operator
pub struct ReduceOperator;

impl Operator for ReduceOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() != 3 {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        let array = evaluator.evaluate(&args[0], context)?;
        let logic = &args[1];
        let initial = evaluator.evaluate(&args[2], context)?;

        match &array {
            Value::Array(arr) => {
                if arr.is_empty() {
                    return Ok(initial);
                }

                let mut accumulator = initial;

                for current in arr {
                    let mut frame_data = serde_json::Map::with_capacity(2);
                    frame_data.insert("current".to_string(), current.clone());
                    frame_data.insert("accumulator".to_string(), accumulator);

                    context.push(Value::Object(frame_data));
                    accumulator = evaluator.evaluate(logic, context)?;
                    context.pop();
                }

                Ok(accumulator)
            }
            Value::Null => Ok(initial),
            _ => Err(Error::InvalidArguments("Invalid Arguments".to_string())),
        }
    }
}

/// All operator - tests if all elements pass
pub struct AllOperator;

impl Operator for AllOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match &collection {
            Value::Array(arr) if !arr.is_empty() => {
                // Fast path for constant predicates
                match predicate {
                    Value::Bool(false) => return Ok(Value::Bool(false)),
                    Value::Bool(true) => return Ok(Value::Bool(true)),
                    _ => {}
                }

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if !is_truthy(&result) {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            Value::Array(arr) if arr.is_empty() => Ok(Value::Bool(false)),
            Value::Null => Ok(Value::Bool(false)),
            _ => Err(Error::InvalidArguments("Invalid Arguments".to_string())),
        }
    }
}

/// Some operator - tests if any element passes
pub struct SomeOperator;

impl Operator for SomeOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match &collection {
            Value::Array(arr) => {
                // Fast path for constant predicates
                match predicate {
                    Value::Bool(false) => return Ok(Value::Bool(false)),
                    Value::Bool(true) => return Ok(Value::Bool(!arr.is_empty())),
                    _ => {}
                }

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(&result) {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            Value::Null => Ok(Value::Bool(false)),
            _ => Err(Error::InvalidArguments("Invalid Arguments".to_string())),
        }
    }
}

/// None operator - tests if no elements pass
pub struct NoneOperator;

impl Operator for NoneOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match &collection {
            Value::Array(arr) => {
                // Fast path for constant predicates
                match predicate {
                    Value::Bool(false) => return Ok(Value::Bool(true)),
                    Value::Bool(true) => return Ok(Value::Bool(arr.is_empty())),
                    _ => {}
                }

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(&result) {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            Value::Null => Ok(Value::Bool(true)),
            _ => Err(Error::InvalidArguments("Invalid Arguments".to_string())),
        }
    }
}

/// Sort operator - sorts arrays with optional custom comparator
pub struct SortOperator;

impl Operator for SortOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() || (args.len() == 1 && args[0] == Value::Null) {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        // Evaluate the array
        let array_value = evaluator.evaluate(&args[0], context)?;

        let mut array = match array_value {
            Value::Array(arr) => arr,
            Value::Null => return Ok(Value::Null), // Missing variable returns null
            _ => return Err(Error::InvalidArguments("Invalid Arguments".to_string())),
        };

        // Get sort direction (default ascending)
        let ascending = if args.len() > 1 {
            let dir = evaluator.evaluate(&args[1], context)?;
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
                let extracted = evaluator.evaluate(extractor, context)?;
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
}

/// Slice operator - extracts a portion of an array or string
pub struct SliceOperator;

impl Operator for SliceOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        // Evaluate the collection
        let collection = evaluator.evaluate(&args[0], context)?;

        // Handle null/missing values
        if collection == Value::Null {
            return Ok(Value::Null);
        }

        // Get start index (default to 0 or end for negative step)
        let start = if args.len() > 1 {
            let start_val = evaluator.evaluate(&args[1], context)?;
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
            let end_val = evaluator.evaluate(&args[2], context)?;
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
            let step_val = evaluator.evaluate(&args[3], context)?;
            match step_val {
                Value::Number(n) => {
                    let s = n.as_i64().unwrap_or(1);
                    if s == 0 {
                        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
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
            _ => Err(Error::InvalidArguments("Invalid Arguments".to_string())),
        }
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

    // Normalize indices
    let (actual_start, actual_end) = if step > 0 {
        let s = normalize_index(start.unwrap_or(0), len);
        let e = normalize_index(end.unwrap_or(len), len);
        (s, e)
    } else {
        // For negative step, defaults are reversed
        let s = normalize_index(start.unwrap_or(len - 1), len);
        let e = if let Some(e) = end {
            normalize_index(e, len)
        } else {
            -1 // Go all the way to the beginning
        };
        (s, e)
    };

    // Collect elements
    if step > 0 {
        let mut i = actual_start;
        while i < actual_end && i < len {
            if i >= 0 {
                result.push(arr[i as usize].clone());
            }
            i += step;
        }
    } else {
        let mut i = actual_start;
        while i > actual_end && i >= 0 && i < len {
            result.push(arr[i as usize].clone());
            i += step; // step is negative
        }
    }

    result
}

// Helper function to normalize slice indices
fn normalize_index(index: i64, len: i64) -> i64 {
    if index < 0 {
        (len + index).max(0)
    } else {
        index.min(len)
    }
}
