# Array Operators Extension Proposal for DataLogic

## Overview

This proposal outlines the implementation of new array operators for the DataLogic core engine to enhance its data manipulation capabilities. These operators will support both arrays and strings where relevant, enabling more advanced data transformations without requiring custom code.

## Proposed Operators

We propose adding the following operators to the existing operator types in the DataLogic engine:

1. **Slice**: Extract a portion of an array or string
2. **Length**: Get the length of an array or string
3. **Sort**: Sort an array in ascending or descending order with optional field extraction

## Implementation Details

### 1. Operator Type Extensions

Extend the `ArrayOp` enum in the `src/logic/operators/array.rs` file to include the new operators:

```rust
pub enum ArrayOp {
    // Existing operators...
    Map,
    Filter,
    Reduce,
    
    // New operators
    Slice,
    Length,
    Sort,
}
```

### 2. Token Handling

For most of these operators, the existing token structure is sufficient, as they can be implemented as standard operators. For the Slice operator, we need to handle multiple parameters (array, start, end, step) but can still use the standard token structure.

### 3. Implementation Approach

#### Slice Operator

The Slice operator will extract a portion of an array or string based on start, end, and optional step parameters:

```rust
fn evaluate_array_slice<'a>(
    args: &[&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Validate we have at least one argument (the array or string)
    if args.is_empty() {
        return Err(LogicError::ArityError {
            operator: "slice".to_string(),
            expected: 1,
            received: 0,
        });
    }

    // Evaluate the array/string
    let array_value = evaluate(args[0], arena)?;
    
    // Get slice parameters (start, end, step)
    let start = if args.len() > 1 { Some(evaluate(args[1], arena)?) } else { None };
    let end = if args.len() > 2 { Some(evaluate(args[2], arena)?) } else { None };
    let step = if args.len() > 3 { Some(evaluate(args[3], arena)?) } else { None };
    
    // Handle arrays
    if let Some(arr) = array_value.as_array() {
        let start_idx = resolve_slice_index(start, 0, arr.len())?;
        let end_idx = resolve_slice_index(end, arr.len(), arr.len())?;
        let step_val = resolve_step(step)?;
        
        // Create sliced array
        let mut result = Vec::new();
        let mut i = start_idx;
        
        while (step_val > 0 && i < end_idx) || (step_val < 0 && i > end_idx) {
            if i < arr.len() {
                result.push(arr[i]);
            }
            
            i = ((i as isize) + step_val) as usize;
        }
        
        // Allocate the result array in the arena
        Ok(DataValue::Array(arena.vec_into_slice(result)))
    }
    // Handle strings
    else if let Some(s) = array_value.as_str() {
        let chars: Vec<char> = s.chars().collect();
        
        let start_idx = resolve_slice_index(start, 0, chars.len())?;
        let end_idx = resolve_slice_index(end, chars.len(), chars.len())?;
        let step_val = resolve_step(step)?;
        
        // Create sliced string
        let mut result = String::new();
        let mut i = start_idx;
        
        while (step_val > 0 && i < end_idx) || (step_val < 0 && i > end_idx) {
            if i < chars.len() {
                result.push(chars[i]);
            }
            
            i = ((i as isize) + step_val) as usize;
        }
        
        // Allocate the result string in the arena
        Ok(DataValue::string(arena, &result))
    }
    else {
        Err(LogicError::TypeError {
            expected: "array or string".to_string(),
            received: array_value.type_name().to_string(),
        })
    }
}

// Helper function to resolve slice indices
fn resolve_slice_index(index: Option<&DataValue>, default: usize, length: usize) -> Result<usize> {
    match index {
        Some(idx) => {
            if let Some(i) = idx.as_integer() {
                if i >= 0 {
                    Ok(i.min(length as i64) as usize)
                } else {
                    // Negative indices count from the end
                    let from_end = length as i64 + i;
                    if from_end < 0 {
                        Ok(0)
                    } else {
                        Ok(from_end as usize)
                    }
                }
            } else {
                Err(LogicError::TypeError {
                    expected: "integer".to_string(),
                    received: idx.type_name().to_string(),
                })
            }
        }
        None => Ok(default),
    }
}

// Helper function to resolve step value
fn resolve_step(step: Option<&DataValue>) -> Result<isize> {
    match step {
        Some(s) => {
            if let Some(i) = s.as_integer() {
                if i == 0 {
                    Err(LogicError::ValueError {
                        message: "Step cannot be zero".to_string(),
                    })
                } else {
                    Ok(i as isize)
                }
            } else {
                Err(LogicError::TypeError {
                    expected: "integer".to_string(),
                    received: s.type_name().to_string(),
                })
            }
        }
        None => Ok(1), // Default step is 1
    }
}
```

#### Length Operator

The Length operator will return the length of an array or string:

```rust
fn evaluate_length<'a>(
    array_value: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if let Some(arr) = array_value.as_array() {
        Ok(DataValue::integer(arr.len() as i64))
    } else if let Some(s) = array_value.as_str() {
        Ok(DataValue::integer(s.chars().count() as i64))
    } else {
        Err(LogicError::TypeError {
            expected: "array or string".to_string(),
            received: array_value.type_name().to_string(),
        })
    }
}
```

#### Sort Operator

The Sort operator will sort array elements in ascending or descending order, with optional field extraction:

```rust
fn evaluate_sort<'a>(
    args: &[&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Ensure we have at least one argument (the array)
    if args.is_empty() {
        return Err(LogicError::ArityError {
            operator: "sort".to_string(),
            expected: 1,
            received: 0,
        });
    }

    // Evaluate the array
    let array_value = evaluate(args[0], arena)?;
    
    if !array_value.is_array() {
        return Err(LogicError::TypeError {
            expected: "array".to_string(),
            received: array_value.type_name().to_string(),
        });
    }
    
    let arr = array_value.as_array().unwrap();
    
    // Parse direction - second argument
    let mut direction = false;
    if args.len() > 1 {
        let dir_value = evaluate(args[1], arena)?;
        if let Some(dir_bool) = dir_value.as_bool() {
            direction = dir;
        } else if let Some(dir_str) = dir_value.as_str() {
            let dir_lower = dir_str.to_lowercase();
            if dir_lower == "desc" || dir_lower == "descending" {
                direction = true;
            }
        }
    }
    
    // Check if we have a field extractor function as the third argument
    let has_field_extractor = args.len() > 2;
    
    // Clone the array to sort it
    let mut result: Vec<&DataValue> = arr.iter().collect();
    
    // Sort the array
    if has_field_extractor {
        let field_extractor = args[2];
        
        // Sort based on the extracted field value
        if direction {
            result.sort_by(|a, b| {
                // Extract field values for comparison
                let a_field = evaluate_field_extractor(a, field_extractor, arena);
                let b_field = evaluate_field_extractor(b, field_extractor, arena);
                
                // Compare the field values in descending order
                match (a_field, b_field) {
                    (Ok(a_val), Ok(b_val)) => b_val.partial_cmp(a_val).unwrap_or(std::cmp::Ordering::Equal),
                    _ => std::cmp::Ordering::Equal, // Handle errors by treating elements as equal
                }
            });
        } else {
            result.sort_by(|a, b| {
                // Extract field values for comparison
                let a_field = evaluate_field_extractor(a, field_extractor, arena);
                let b_field = evaluate_field_extractor(b, field_extractor, arena);
                
                // Compare the field values in ascending order
                match (a_field, b_field) {
                    (Ok(a_val), Ok(b_val)) => a_val.partial_cmp(b_val).unwrap_or(std::cmp::Ordering::Equal),
                    _ => std::cmp::Ordering::Equal, // Handle errors by treating elements as equal
                }
            });
        }
    } else {
        // Standard sorting of whole items
        if direction {
            result.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            result.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
    }
    
    // Allocate the sorted array in the arena
    Ok(DataValue::Array(arena.vec_into_slice(result)))
}

// Helper function to evaluate the field extractor against an item
fn evaluate_field_extractor<'a>(
    item: &'a DataValue<'a>,
    extractor: &'a Token<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    // Set the current item as the context for evaluation
    let old_context = arena.get_current_context();
    arena.set_current_context(item, &DataValue::String(""));
    
    // Evaluate the extractor with the item as context
    let result = evaluate(extractor, arena);
    
    // Restore the original context
    arena.set_current_context(old_context.0, old_context.1);
    
    result
}
```

### 4. JSON Representation

These operators will be accessible from JSONLogic expressions with the following syntax:

```json
// Slice an array from index 1 to 4
{ "slice": [{"var": "myArray"}, 1, 4] }

// Slice with step = 2
{ "slice": [{"var": "myArray"}, 0, null, 2] }

// Reverse an array using slice
{ "slice": [{"var": "myArray"}, null, null, -1] }

// Get the length of an array
{ "length": {"var": "myArray"} }

// Sort an array (ascending by default)
{ "sort": [{"var": "myArray"}] }

// Sort an array in descending order
{ "sort": [{"var": "myArray"}, true] }

// Sort an array of objects by a specific field
{ "sort": [{"var": "people"}, false, {"var": "age"}] }

// Sort by nested property
{ "sort": [{"var": "companies"}, true, {"var": "stats.revenue"}] }

// Sort using a more complex field expression
{ "sort": [{"var": "items"}, false, {"+": [{"var": "price"}, {"var": "tax"}]}] }
```

## Testing Strategy

Each operator should be tested with:

1. Arrays of different types (numbers, strings, objects)
2. Strings of different encodings
3. Edge cases (empty arrays/strings, single elements)
4. Error cases (non-array/string inputs)
5. For sort, test object sorting with field extraction in both directions

Example test cases:

```rust
#[test]
fn test_slice_operator() {
    let arena = DataArena::new();
    let array_json = json!([1, 2, 3, 4, 5]);
    let array = DataValue::from_json(&array_json, &arena);
    
    // Test basic slicing
    let slice_expr = json!({"slice": [{"var": ""}, 1, 4]});
    let token = jsonlogic::parse_json(&slice_expr, &arena).unwrap();
    
    arena.set_current_context(&array, &DataValue::String("$"));
    let result = evaluate(token, &arena).unwrap();
    
    assert_eq!(result.to_json(), json!([2, 3, 4]));
    
    // Test negative indices
    let slice_expr = json!({"slice": [{"var": ""}, -3, -1]});
    let token = jsonlogic::parse_json(&slice_expr, &arena).unwrap();
    
    arena.set_current_context(&array, &DataValue::String("$"));
    let result = evaluate(token, &arena).unwrap();
    
    assert_eq!(result.to_json(), json!([3, 4]));
}

#[test]
fn test_sort_with_field_extraction() {
    let arena = DataArena::new();
    
    // Create an array of people with different ages
    let people_json = json!([
        {"name": "Alice", "age": 30, "address": {"city": "New York"}},
        {"name": "Bob", "age": 25, "address": {"city": "Boston"}},
        {"name": "Charlie", "age": 35, "address": {"city": "Chicago"}}
    ]);
    let people = DataValue::from_json(&people_json, &arena);
    
    // Sort by age ascending
    let sort_expr = json!({"sort": [{"var": ""}, "asc", {"var": "age"}]});
    let token = jsonlogic::parse_json(&sort_expr, &arena).unwrap();
    
    arena.set_current_context(&people, &DataValue::String("$"));
    let result = evaluate(token, &arena).unwrap();
    let result_json = result.to_json();
    
    // Expected result: sorted by age (25, 30, 35)
    assert_eq!(result_json[0]["name"], "Bob");
    assert_eq!(result_json[1]["name"], "Alice");
    assert_eq!(result_json[2]["name"], "Charlie");
    
    // Sort by nested field (city) descending
    let sort_expr = json!({"sort": [{"var": ""}, "desc", {"var": "address.city"}]});
    let token = jsonlogic::parse_json(&sort_expr, &arena).unwrap();
    
    arena.set_current_context(&people, &DataValue::String("$"));
    let result = evaluate(token, &arena).unwrap();
    let result_json = result.to_json();
    
    // Expected result: sorted by city descending (New York, Chicago, Boston)
    assert_eq!(result_json[0]["name"], "Alice");
    assert_eq!(result_json[1]["name"], "Charlie");
    assert_eq!(result_json[2]["name"], "Bob");
}
```

## Integration with Core Engine

These operators extend the capabilities of the core engine, making it more expressive for common data manipulation tasks. The implementation adheres to the existing patterns in the codebase:

1. Operators accept arrays and/or strings as input
2. They produce consistent output types
3. Proper error handling is provided for type mismatches
4. They follow the same calling conventions as existing operators

## Implementation Phases

1. **Phase 1: Core Implementation** (1 week)
   - Add the new operators to the `ArrayOp` enum
   - Implement the core logic for each operator
   - Add operator evaluation to the main evaluator

2. **Phase 2: Testing and Edge Cases** (3 days)
   - Create comprehensive test suite
   - Handle edge cases and error conditions
   - Optimize performance

3. **Phase 3: Documentation and Examples** (2 days)
   - Document the new operators
   - Create usage examples
   - Update operator documentation

## Conclusion

Adding these array operators will significantly enhance DataLogic's data manipulation capabilities, making it easier to express common operations without custom code. They complement the existing operators and provide a more complete set of tools for working with arrays and strings.
