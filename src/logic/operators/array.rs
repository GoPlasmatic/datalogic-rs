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
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "all".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.false_value()),
        _ => return Err(LogicError::OperatorError {
            operator: "all".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // If the array is empty, return false (vacuously false)
    if items.is_empty() {
        return Ok(arena.false_value());
    }
    
    // Cache the condition token
    let condition = &args[1];
    
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
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "some".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.false_value()),
        _ => return Err(LogicError::OperatorError {
            operator: "some".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // If the array is empty, return false (vacuously false)
    if items.is_empty() {
        return Ok(arena.false_value());
    }
    
    // Cache the condition token
    let condition = &args[1];
    
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
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "none".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.true_value()),
        _ => return Err(LogicError::OperatorError {
            operator: "none".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // If the array is empty, return true (vacuously true)
    if items.is_empty() {
        return Ok(arena.true_value());
    }
    
    // Cache the condition token
    let condition = &args[1];
    
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
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "map".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.alloc(DataValue::Array(&[]))),
        _ => return Err(LogicError::OperatorError {
            operator: "map".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // Fast path for empty array
    if items.is_empty() {
        return Ok(arena.alloc(DataValue::Array(&[])));
    }
    
    // Cache the function token
    let function = &args[1];
    
    // Map each item in the array
    let mut results = arena.get_data_value_vec();
    results.reserve(items.len());
    
    for item in items.iter() {
        // Evaluate the function with the item as context
        let result = evaluate(function, item, arena)?;
        results.push(result.clone());
    }
    
    // Create the result array
    let result_array = DataValue::Array(arena.alloc_slice_clone(&results));
    arena.release_data_value_vec(results);
    
    Ok(arena.alloc(result_array))
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
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() != 2 {
        return Err(LogicError::OperatorError {
            operator: "filter".to_string(),
            reason: format!("Expected 2 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => return Ok(arena.alloc(DataValue::Array(&[]))),
        _ => return Err(LogicError::OperatorError {
            operator: "filter".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // Fast path for empty array
    if items.is_empty() {
        return Ok(arena.alloc(DataValue::Array(&[])));
    }
    
    // Cache the condition token
    let condition = &args[1];
    
    // Filter the array
    let mut results = arena.get_data_value_vec();
    
    for item in items.iter() {
        // Evaluate the condition with the item as context
        if evaluate(condition, item, arena)?.coerce_to_bool() {
            results.push(item.clone());
        }
    }
    
    // Create the result array
    let result_array = DataValue::Array(arena.alloc_slice_clone(&results));
    arena.release_data_value_vec(results);
    
    Ok(arena.alloc(result_array))
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
) -> Result<&'a DataValue<'a>> {
    // Fast path for invalid arguments
    if args.len() < 2 || args.len() > 3 {
        return Err(LogicError::OperatorError {
            operator: "reduce".to_string(),
            reason: format!("Expected 2 or 3 arguments, got {}", args.len()),
        });
    }
    
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // Check that the first argument is an array
    let items = match array {
        DataValue::Array(items) => items,
        // Fast path for common case of null (treat as empty array)
        DataValue::Null => {
            // If we have an initial value, return it
            if args.len() == 3 {
                return evaluate(&args[2], data, arena);
            }
            return Err(LogicError::OperatorError {
                operator: "reduce".to_string(),
                reason: "Cannot reduce empty array without initial value".to_string(),
            });
        },
        _ => return Err(LogicError::OperatorError {
            operator: "reduce".to_string(),
            reason: format!("First argument must be an array, got {:?}", array),
        }),
    };
    
    // Fast path for empty array
    if items.is_empty() {
        // If we have an initial value, return it
        if args.len() == 3 {
            return evaluate(&args[2], data, arena);
        }
        return Err(LogicError::OperatorError {
            operator: "reduce".to_string(),
            reason: "Cannot reduce empty array without initial value".to_string(),
        });
    }
    
    // Get the initial value
    let initial = if args.len() == 3 {
        evaluate(&args[2], data, arena)?
    } else {
        // If no initial value is provided, use the first item
        &items[0]
    };
    
    // Cache the function token
    let function = &args[1];
    
    // Check for fast path optimizations
    if let Token::Operator { op_type: _op_type, args: _op_args } = &args[1] {
        // Commenting out problematic code for now
        /*
        // Check if op_args is an ArrayLiteral
        if let Token::ArrayLiteral(op_items) = op_args {
            if let crate::logic::OperatorType::Arithmetic(op) = op_type {
                // Check if this is a simple addition or multiplication
                if *op == crate::logic::operators::arithmetic::ArithmeticOp::Add {
                    // Fast path for sum reduction with any initial value
                    return eval_reduce_sum(args, data, arena, initial);
                } else if *op == crate::logic::operators::arithmetic::ArithmeticOp::Multiply {
                    // Fast path for product reduction with any initial value
                    return eval_reduce_product(args, data, arena, initial);
                }
            }
        }
        */
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
            
            // Start from the first item if no initial value was provided
            let start_idx = if args.len() == 3 { 0 } else { 1 };
            
            // Reduce the array
            for i in start_idx..items.len() {
                // Update the context with the current item and accumulator
                entries[0].1 = items[i].clone();
                entries[1].1 = acc.clone();
                
                // Create the context object and allocate it in the arena
                let context_entries = arena.alloc_slice_clone(&entries);
                let context = arena.alloc(DataValue::Object(context_entries));
                
                // Evaluate the function with the context
                acc = evaluate(function, context, arena)?;
            }
            
            Ok(acc)
        },
        _ => unreachable!(), // We already checked that array is an array
    }
}

/// Fast path implementation for sum reduction.
fn eval_reduce_sum<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
    initial: &'a DataValue<'a>,
) -> Result<&'a DataValue<'a>> {
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // We already checked that array is an array in the caller
    if let DataValue::Array(items) = array {
        // Fast path for empty arrays
        if items.is_empty() {
            return Ok(initial);
        }
        
        // Initialize the accumulator with the initial value
        let mut sum = initial.coerce_to_number().unwrap_or_else(|| {
            // If initial value can't be coerced to a number, start with 0
            crate::value::NumberValue::Integer(0)
        });
        
        // Sum all items in the array
        for item in items.iter() {
            if let Some(num) = item.coerce_to_number() {
                sum = match (sum, num) {
                    (crate::value::NumberValue::Integer(a), crate::value::NumberValue::Integer(b)) => {
                        // Check for overflow
                        match a.checked_add(b) {
                            Some(result) => crate::value::NumberValue::Integer(result),
                            None => crate::value::NumberValue::Float(a as f64 + b as f64),
                        }
                    },
                    _ => {
                        // Use floating point for mixed or float types
                        let a_f64 = sum.as_f64();
                        let b_f64 = num.as_f64();
                        crate::value::NumberValue::Float(a_f64 + b_f64)
                    }
                };
            }
        }
        
        // Return the sum
        Ok(arena.alloc(DataValue::Number(sum)))
    } else {
        // This should never happen as we already checked in the caller
        unreachable!()
    }
}

/// Fast path implementation for product reduction.
fn eval_reduce_product<'a>(
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
    initial: &'a DataValue<'a>,
) -> Result<&'a DataValue<'a>> {
    // Evaluate the first argument to get the array
    let array = evaluate(&args[0], data, arena)?;
    
    // We already checked that array is an array in the caller
    if let DataValue::Array(items) = array {
        // Fast path for empty arrays
        if items.is_empty() {
            return Ok(initial);
        }
        
        // Initialize the accumulator with the initial value
        let mut product = initial.coerce_to_number().unwrap_or_else(|| {
            // If initial value can't be coerced to a number, start with 1
            crate::value::NumberValue::Integer(1)
        });
        
        // Multiply all items in the array
        for item in items.iter() {
            if let Some(num) = item.coerce_to_number() {
                product = match (product, num) {
                    (crate::value::NumberValue::Integer(a), crate::value::NumberValue::Integer(b)) => {
                        // Check for overflow
                        match a.checked_mul(b) {
                            Some(result) => crate::value::NumberValue::Integer(result),
                            None => crate::value::NumberValue::Float(a as f64 * b as f64),
                        }
                    },
                    _ => {
                        // Use floating point for mixed or float types
                        let a_f64 = product.as_f64();
                        let b_f64 = num.as_f64();
                        crate::value::NumberValue::Float(a_f64 * b_f64)
                    }
                };
            }
        }
        
        // Return the product
        Ok(arena.alloc(DataValue::Number(product)))
    } else {
        // This should never happen as we already checked in the caller
        unreachable!()
    }
}

/// Evaluates a merge operation.
pub fn eval_merge<'a>(
    args: &'a [Token<'a>],
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