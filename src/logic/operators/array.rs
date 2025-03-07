//! Array operators for logic expressions.
//!
//! This module provides implementations for array operators
//! such as map, filter, reduce, etc.

use crate::arena::DataArena;
use crate::value::DataValue;
use crate::logic::token::Token;
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
}

/// Evaluates an all operation.
pub fn eval_all<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "all".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Reuse result objects
    let true_result = DataValue::Bool(true);
    let false_result = DataValue::Bool(false);
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match &array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(false_result),
        _ => return Err(LogicError::OperatorError {
            operator: "all".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // If the array is empty, return false (vacuously false)
    if items.is_empty() {
        return Ok(false_result);
    }
    
    // Cache the condition token
    let condition = &args[1];
    
    // Check if all items satisfy the condition
    for item in items.iter() {
        // Evaluate the function with the item as context
        if !evaluate(condition, item, arena)?.coerce_to_bool() {
            return Ok(false_result);
        }
    }
    
    // If all items satisfy the condition, return true
    Ok(true_result)
}

/// Evaluates a some operation.
pub fn eval_some<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "some".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Reuse result objects
    let true_result = DataValue::Bool(true);
    let false_result = DataValue::Bool(false);
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match &array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(false_result),
        _ => return Err(LogicError::OperatorError {
            operator: "some".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // If the array is empty, return false (vacuously false)
    if items.is_empty() {
        return Ok(false_result);
    }
    
    // Cache the condition token
    let condition = &args[1];
    
    // Check if any item satisfies the condition
    for item in items.iter() {
        // Evaluate the function with the item as context
        if evaluate(condition, item, arena)?.coerce_to_bool() {
            return Ok(true_result);
        }
    }
    
    // If no item satisfies the condition, return false
    Ok(false_result)
}

/// Evaluates a none operation.
pub fn eval_none<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "none".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Reuse result objects
    let true_result = DataValue::Bool(true);
    let false_result = DataValue::Bool(false);
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match &array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(true_result),
        _ => return Err(LogicError::OperatorError {
            operator: "none".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // If the array is empty, return true (vacuously true)
    if items.is_empty() {
        return Ok(true_result);
    }
    
    // Cache the condition token
    let condition = &args[1];
    
    // Check if no item satisfies the condition
    for item in items.iter() {
        // Evaluate the function with the item as context
        if evaluate(condition, item, arena)?.coerce_to_bool() {
            return Ok(false_result);
        }
    }
    
    // If no item satisfies the condition, return true
    Ok(true_result)
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
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Check that we have exactly 2 arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "map".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Handle the case where the array is null or not an array
    let items = match &array {
        DataValue::Array(items) => items,
        DataValue::Null => {
            // Return an empty array if the input is null
            return Ok(DataValue::Array(&[]));
        },
        _ => return Err(LogicError::OperatorError {
            operator: "map".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // If the array is empty, return an empty array
    if items.is_empty() {
        return Ok(DataValue::Array(&[]));
    }
    
    // Create a temporary arena for intermediate allocations
    let _temp_arena = arena.create_temp_arena();
    
    // Get a pre-allocated vector from the pool
    let mut results = arena.get_data_value_vec();
    
    // Apply the function to each item in the array
    for item in items.iter() {
        // Evaluate the function with the item as context
        let result = evaluate(&args[1], item, arena)?;
        results.push(result);
    }
    
    // Allocate the result array
    let mapped_slice = arena.alloc_slice_clone(&results);
    
    // Release the vector back to the pool
    arena.release_data_value_vec(results);
    
    // Return the mapped array
    Ok(DataValue::Array(mapped_slice))
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
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Check that we have exactly 2 arguments
    if args.len() != 2 {
        return Err(LogicError::operator_error("filter", format!("Expected 2 arguments, got {}", args.len())));
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Handle the case where the array is null or not an array
    let items = match &array {
        DataValue::Array(items) => items,
        DataValue::Null => {
            // Return an empty array if the input is null
            return Ok(DataValue::Array(&[]));
        },
        _ => return Err(LogicError::operator_error("filter", format!("First argument must be an array, got {:?}", array))),
    };
    
    // If the array is empty, return an empty array
    if items.is_empty() {
        return Ok(DataValue::Array(&[]));
    }
    
    // Create a temporary arena for intermediate allocations
    let _temp_arena = arena.create_temp_arena();
    
    // Get a pre-allocated vector from the pool
    let mut filtered = arena.get_data_value_vec();
    
    // Filter the array based on the condition
    for item in items.iter() {
        // Evaluate the condition with the item as context
        let result = evaluate(&args[1], item, arena)?;
        
        // If the result is truthy, include the item in the filtered array
        if result.coerce_to_bool() {
            filtered.push(item.clone());
        }
    }
    
    // Allocate the result array
    let filtered_slice = arena.alloc_slice_clone(&filtered);
    
    // Release the vector back to the pool
    arena.release_data_value_vec(filtered);
    
    // Return the filtered array
    Ok(DataValue::Array(filtered_slice))
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
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Fast path for invalid argument counts
    if args.len() < 3 {
        return Err(LogicError::OperatorError {
            operator: "reduce".to_string(),
            reason: format!("Expected at least 3 arguments, got {}", args.len()),
        });
    }

    // Evaluate array and initial value
    let array = evaluate(&args[0], data, arena)?;
    let initial = evaluate(&args[2], data, arena)?;
    
    // Fast path for common reduction patterns
    // If this is a simple sum or product reduction with constant initial value,
    // we can use specialized implementations
    if let Token::Operator { 
        op_type: crate::logic::OperatorType::Arithmetic(op), 
        args: op_args 
    } = &args[1] {
        if *op == crate::logic::operators::arithmetic::ArithmeticOp::Add {
            // Check if this is a simple variable access pattern
            if op_args.len() == 2 {
                if let (Token::Variable { path: acc_path, .. }, Token::Variable { path: curr_path, .. }) = (&op_args[0], &op_args[1]) {
                    // Check if it's the standard accumulator/current pattern
                    if (acc_path == &"accumulator" && curr_path == &"current") ||
                       (acc_path == &"current" && curr_path == &"accumulator") {
                        // Fast path for sum reduction with any initial value
                        return eval_reduce_sum(args, data, arena, initial);
                    }
                    // Check if it's accessing a property of current
                    if acc_path == &"accumulator" && curr_path.starts_with("current.") {
                        // This is a property access pattern, use the general implementation
                        // as it needs to handle nested property access
                    }
                }
            }
        } else if *op == crate::logic::operators::arithmetic::ArithmeticOp::Multiply {
            // Check if it's a simple variable access pattern
            if op_args.len() == 2 {
                if let (Token::Variable { path: acc_path, .. }, Token::Variable { path: curr_path, .. }) = (&op_args[0], &op_args[1]) {
                    // Check if it's the standard accumulator/current pattern
                    if (acc_path == &"accumulator" && curr_path == &"current") ||
                       (acc_path == &"current" && curr_path == &"accumulator") {
                        // Fast path for product reduction with any initial value
                        return eval_reduce_product(args, data, arena, initial);
                    }
                }
            }
        }
    }

    // Initialize static keys only once - these are interned and reused
    let curr_key = arena.intern_str("current");
    let acc_key = arena.intern_str("accumulator");
    
    match array {
        DataValue::Array(items) => {
            // Fast path for empty arrays
            if items.is_empty() {
                return Ok(initial);
            }
            
            // Pre-allocate the context entries array once and reuse it
            let mut acc = initial;
            let mut entries = [(curr_key, DataValue::Null), (acc_key, DataValue::Null)];
            
            // Process each item in the array
            for item in items.iter() {
                // Update context entries in place with cloned values
                entries[0].1 = item.clone(); // Clone is necessary here
                entries[1].1 = acc.clone();  // Clone is necessary here
                
                // Allocate in arena for this iteration
                let context_entries = arena.alloc_slice_clone(&entries);
                // Create and store context in arena
                let context_obj = DataValue::Object(context_entries);
                // Allocate context in arena to extend its lifetime
                let context = arena.alloc(context_obj);
                
                // Evaluate and update accumulator
                acc = evaluate(&args[1], context, arena)?;
            }
            
            // Return the final result
            Ok(acc)
        },
        DataValue::Null => Ok(initial),
        _ => Err(LogicError::OperatorError {
            operator: "reduce".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    }
}

/// Fast path implementation for sum reduction.
fn eval_reduce_sum<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
    initial: DataValue<'a>,
) -> Result<DataValue<'a>> {
    // Evaluate the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Handle the case where the array is null or not an array
    let items = match &array {
        DataValue::Array(items) => items,
        DataValue::Null => return Ok(initial),
        _ => return Err(LogicError::operator_error("reduce", format!("First argument must be an array, got {:?}", array))),
    };
    
    // If the array is empty, return the initial value
    if items.is_empty() {
        return Ok(initial);
    }
    
    // Fast path for numeric initial value
    if let Some(mut sum) = initial.as_f64() {
        // Directly sum all numeric values
        for item in items.iter() {
            if let Some(val) = item.as_f64() {
                sum += val;
            }
        }
        
        // Return the sum as the appropriate numeric type
        if sum.fract() == 0.0 && sum >= i64::MIN as f64 && sum <= i64::MAX as f64 {
            return Ok(DataValue::integer(sum as i64));
        } else {
            return Ok(DataValue::float(sum));
        }
    }
    
    // Fast path for string initial value
    if let Some(initial_str) = initial.as_str() {
        // Get a pre-allocated vector from the pool
        let mut parts = arena.get_data_value_vec();
        
        // Start with the initial string
        parts.push(DataValue::String(arena.intern_str(initial_str)));
        
        // Add all string values
        for item in items.iter() {
            if let Some(s) = item.as_str() {
                parts.push(DataValue::String(arena.intern_str(s)));
            } else {
                // For non-string values, convert to string
                let s = format!("{}", item);
                parts.push(DataValue::String(arena.intern_str(&s)));
            }
        }
        
        // Join all strings
        let mut result = String::new();
        for (i, part) in parts.iter().enumerate() {
            if let Some(s) = part.as_str() {
                if i > 0 {
                    result.push_str(s);
                } else {
                    result = s.to_string();
                }
            }
        }
        
        // Release the vector back to the pool
        arena.release_data_value_vec(parts);
        
        // Return the joined string
        return Ok(DataValue::String(arena.intern_str(&result)));
    }
    
    // Fall back to the general case implementation
    let curr_key = arena.intern_str("current");
    let acc_key = arena.intern_str("accumulator");
    
    let mut acc = initial; // Start with provided initial value
    let mut entries = [(curr_key, DataValue::Null), (acc_key, DataValue::Null)];
    
    for item in items.iter() {
        // Update entries in place with cloned values
        entries[0].1 = item.clone();
        entries[1].1 = acc.clone();
        
        // Allocate in arena for this iteration
        let context_entries = arena.alloc_slice_clone(&entries);
        // Create and store context in arena
        let context_obj = DataValue::Object(context_entries);
        // Allocate context in arena to extend its lifetime
        let context = arena.alloc(context_obj);
        
        // Evaluate and update accumulator
        acc = evaluate(&args[1], context, arena)?;
    }
    
    Ok(acc)
}

/// Fast path implementation for product reduction.
fn eval_reduce_product<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
    initial: DataValue<'a>,
) -> Result<DataValue<'a>> {
    // Evaluate the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Handle the case where the array is null or not an array
    let items = match &array {
        DataValue::Array(items) => items,
        DataValue::Null => return Ok(initial),
        _ => return Err(LogicError::operator_error("reduce", format!("First argument must be an array, got {:?}", array))),
    };
    
    // If the array is empty, return the initial value
    if items.is_empty() {
        return Ok(initial);
    }
    
    // Fast path for numeric initial value
    if let Some(mut product) = initial.as_f64() {
        // Directly multiply all numeric values
        for item in items.iter() {
            if let Some(val) = item.as_f64() {
                product *= val;
            }
        }
        
        // Return the product as the appropriate numeric type
        if product.fract() == 0.0 && product >= i64::MIN as f64 && product <= i64::MAX as f64 {
            return Ok(DataValue::integer(product as i64));
        } else {
            return Ok(DataValue::float(product));
        }
    }
    
    // Fall back to the general case implementation
    let curr_key = arena.intern_str("current");
    let acc_key = arena.intern_str("accumulator");
    
    let mut acc = initial; // Start with provided initial value
    let mut entries = [(curr_key, DataValue::Null), (acc_key, DataValue::Null)];
    
    for item in items.iter() {
        // Update entries in place with cloned values
        entries[0].1 = item.clone();
        entries[1].1 = acc.clone();
        
        // Allocate in arena for this iteration
        let context_entries = arena.alloc_slice_clone(&entries);
        // Create and store context in arena
        let context_obj = DataValue::Object(context_entries);
        // Allocate context in arena to extend its lifetime
        let context = arena.alloc(context_obj);
        
        // Evaluate and update accumulator
        acc = evaluate(&args[1], context, arena)?;
    }
    
    Ok(acc)
}

/// Evaluates a merge operation.
pub fn eval_merge<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // If there are no arguments, return an empty array
    if args.is_empty() {
        return Ok(DataValue::Array(&[]));
    }
    
    // Get a pre-allocated vector from the pool
    let mut merged = arena.get_data_value_vec();
    
    for arg in args {
        // Evaluate the argument
        let value = evaluate(arg, data, arena)?;
        
        // Check that the argument is an array
        match &value {
            DataValue::Array(items) => {
                // Add all items to the merged array
                for item in items.iter() {
                    merged.push(item.clone());
                }
            },
            _ => {
                // If the argument is not an array, add it as a single item
                merged.push(value);
            },
        }
    }
    
    // Allocate the result array
    let merged_slice = arena.alloc_slice_clone(&merged);
    
    // Release the vector back to the pool
    arena.release_data_value_vec(merged);
    
    // Return the merged array
    Ok(DataValue::Array(merged_slice))
}

#[cfg(test)]
mod tests {
    use crate::arena::DataArena;
    use crate::value::DataValue;
    use crate::logic::parser::parse_str;
    use crate::logic::evaluator::evaluate;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_map_operator() {
        let arena = DataArena::new();
        
        // Test case 1: Map integers to double their value
        let data_json = json!({
            "integers": [1, 2, 3]
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        let rule_str = r#"{"map": [{"var": "integers"}, {"*": [{"var": ""}, 2]}]}"#;
        let token = parse_str(rule_str, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        // Check that the result is an array with the expected values
        assert!(result.is_array());
        let result_array = result.as_array().unwrap();
        assert_eq!(result_array.len(), 3);
        assert_eq!(result_array[0].as_i64(), Some(2));
        assert_eq!(result_array[1].as_i64(), Some(4));
        assert_eq!(result_array[2].as_i64(), Some(6));
        
        // Test case 2: Map with null data should return empty array
        let null_data = DataValue::null();
        let result = evaluate(token, &null_data, &arena).unwrap();
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 0);
        
        // Test case 3: Map with object array
        let desserts_json = json!({
            "desserts": [
                {"name": "apple", "qty": 1},
                {"name": "brownie", "qty": 2},
                {"name": "cupcake", "qty": 3}
            ]
        });
        let desserts_data = DataValue::from_json(&desserts_json, &arena);
        
        let qty_rule_str = r#"{"map": [{"var": "desserts"}, {"var": "qty"}]}"#;
        let qty_token = parse_str(qty_rule_str, &arena).unwrap();
        let qty_result = evaluate(qty_token, &desserts_data, &arena).unwrap();
        
        // Check that the result is an array with the expected values
        assert!(qty_result.is_array());
        let qty_array = qty_result.as_array().unwrap();
        assert_eq!(qty_array.len(), 3);
        assert_eq!(qty_array[0].as_i64(), Some(1));
        assert_eq!(qty_array[1].as_i64(), Some(2));
        assert_eq!(qty_array[2].as_i64(), Some(3));
    }
    
    #[test]
    fn test_filter_operator() {
        let arena = DataArena::new();
        
        // Test case 1: Filter integers greater than or equal to 2
        let data_json = json!({
            "integers": [1, 2, 3]
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        let rule_str = r#"{"filter": [{"var": "integers"}, {">=": [{"var": ""}, 2]}]}"#;
        let token = parse_str(rule_str, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        // Check that the result is an array with the expected values
        assert!(result.is_array());
        let result_array = result.as_array().unwrap();
        assert_eq!(result_array.len(), 2);
        assert_eq!(result_array[0].as_i64(), Some(2));
        assert_eq!(result_array[1].as_i64(), Some(3));
        
        // Test case 2: Filter with constant true (should return all elements)
        let true_rule_str = r#"{"filter": [{"var": "integers"}, true]}"#;
        let true_token = parse_str(true_rule_str, &arena).unwrap();
        let true_result = evaluate(true_token, &data, &arena).unwrap();
        
        assert!(true_result.is_array());
        let true_array = true_result.as_array().unwrap();
        assert_eq!(true_array.len(), 3);
        
        // Test case 3: Filter with constant false (should return empty array)
        let false_rule_str = r#"{"filter": [{"var": "integers"}, false]}"#;
        let false_token = parse_str(false_rule_str, &arena).unwrap();
        let false_result = evaluate(false_token, &data, &arena).unwrap();
        
        assert!(false_result.is_array());
        assert_eq!(false_result.as_array().unwrap().len(), 0);
        
        // Test case 4: Filter odd numbers (using modulo)
        let odd_rule_str = r#"{"filter": [{"var": "integers"}, {"%": [{"var": ""}, 2]}]}"#;
        let odd_token = parse_str(odd_rule_str, &arena).unwrap();
        let odd_result = evaluate(odd_token, &data, &arena).unwrap();
        
        assert!(odd_result.is_array());
        let odd_array = odd_result.as_array().unwrap();
        assert_eq!(odd_array.len(), 2);
        assert_eq!(odd_array[0].as_i64(), Some(1));
        assert_eq!(odd_array[1].as_i64(), Some(3));
    }
    
    #[test]
    fn test_reduce_operator() {
        let arena = DataArena::new();
        
        // Test case 1: Sum of integers
        let data_json = json!({
            "integers": [1, 2, 3, 4]
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        let rule_str = r#"{"reduce": [{"var": "integers"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}"#;
        let token = parse_str(rule_str, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        // Check that the result is the sum of the integers
        assert_eq!(result.as_i64(), Some(10));
        
        // Test case 2: Product of integers
        let product_rule_str = r#"{"reduce": [{"var": "integers"}, {"*": [{"var": "current"}, {"var": "accumulator"}]}, 1]}"#;
        let product_token = parse_str(product_rule_str, &arena).unwrap();
        let product_result = evaluate(product_token, &data, &arena).unwrap();
        
        // Check that the result is the product of the integers
        assert_eq!(product_result.as_i64(), Some(24));
        
        // Test case 3: Reduce with variable initial value
        let var_initial_rule_str = r#"{"reduce": [{"var": "integers"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, {"var": "start_with"}]}"#;
        let var_initial_data_json = json!({
            "integers": [1, 2, 3, 4],
            "start_with": 59
        });
        let var_initial_data = DataValue::from_json(&var_initial_data_json, &arena);
        let var_initial_token = parse_str(var_initial_rule_str, &arena).unwrap();
        let var_initial_result = evaluate(var_initial_token, &var_initial_data, &arena).unwrap();
        
        // Check that the result is the sum of the integers plus the initial value
        assert_eq!(var_initial_result.as_i64(), Some(69));
        
        // Test case 4: Reduce with null array
        let null_data = DataValue::null();
        let null_result = evaluate(token, &null_data, &arena).unwrap();
        
        // Check that the result is the initial value
        assert_eq!(null_result.as_i64(), Some(0));
        
        // Test case 5: Reduce with object array
        let desserts_json = json!({
            "desserts": [
                {"name": "apple", "qty": 1},
                {"name": "brownie", "qty": 2},
                {"name": "cupcake", "qty": 3}
            ]
        });
        let desserts_data = DataValue::from_json(&desserts_json, &arena);
        
        let qty_sum_rule_str = r#"{"reduce": [{"var": "desserts"}, {"+": [{"var": "accumulator"}, {"var": "current.qty"}]}, 0]}"#;
        let qty_sum_token = parse_str(qty_sum_rule_str, &arena).unwrap();
        let qty_sum_result = evaluate(qty_sum_token, &desserts_data, &arena).unwrap();
        
        // Check that the result is the sum of the quantities
        assert_eq!(qty_sum_result.as_i64(), Some(6));
    }
} 