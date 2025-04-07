# Arena-Based Memory Management in DataLogic-rs

## Overview

DataLogic-rs uses an arena-based memory management system that significantly improves performance and memory efficiency compared to traditional allocation strategies. This document explains how to effectively use this system, especially for users implementing custom operators or working with long-running applications.

## What is Arena Allocation?

Arena allocation (also known as "bump allocation") is a memory management technique where:

- Memory is allocated in large, contiguous chunks
- Individual allocations are extremely fast (just incrementing a pointer)
- Memory is freed all at once instead of individually
- Improved memory locality leads to better cache performance

This approach is particularly well-suited for applications like JSON Logic evaluation, where many small objects are created during rule evaluation but can all be discarded together once the evaluation is complete.

## Benefits of Arena Allocation in DataLogic-rs

Using the arena allocator in DataLogic-rs provides several key advantages:

- **Performance**: Allocation is much faster than general-purpose allocators
- **Memory Efficiency**: Reduced overhead per allocation
- **Cache Locality**: Related data structures are stored close together
- **Simplifies Memory Management**: No need to track individual deallocations
- **Eliminates Memory Leaks**: All memory is freed together with `reset_arena()`

## Using the Arena in Client Code

### Basic Memory Management

The arena is managed internally within the `DataLogic` struct. Most users only need to be aware of when to reset the arena to free memory:

```rust
use datalogic_rs::DataLogic;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a DataLogic instance with default settings
    let mut dl = DataLogic::new();
    
    // Use the library for evaluations
    let result1 = dl.evaluate_str(
        r#"{ ">": [{"var": "temp"}, 100] }"#,
        r#"{"temp": 110}"#,
        None
    )?;
    
    // When processing large batches, reset the arena periodically
    dl.reset_arena();
    
    // Continue with new evaluations
    let result2 = dl.evaluate_str(
        r#"{ "<": [{"var": "count"}, 10] }"#,
        r#"{"count": 5}"#,
        None
    )?;
    
    println!("Results: {}, {}", result1, result2);
    Ok(())
}
```

### Tuning Arena Size

For applications with specific memory requirements, you can configure the chunk size used for arena allocations:

```rust
use datalogic_rs::DataLogic;

// Create a DataLogic instance with a 1MB chunk size
let mut dl = DataLogic::with_chunk_size(1024 * 1024);
```

Benefits of tuning chunk size:
- Smaller chunks: Better for memory-constrained environments
- Larger chunks: Better for performance in rule-heavy applications
- Setting a size limit: Helps prevent unbounded memory growth

### Processing Large Batches

When processing many rules in a long-running application, it's important to periodically reset the arena:

```rust
use datalogic_rs::{DataLogic, Result};

fn process_batches(batches: Vec<(String, String)>) -> Result<()> {
    let mut dl = DataLogic::new();
    
    for (i, (rule_str, data_str)) in batches.iter().enumerate() {
        // Process each batch
        let result = dl.evaluate_str(rule_str, data_str, None)?;
        println!("Batch {}: {}", i, result);
        
        // Reset arena after each batch or after X batches
        // to prevent excessive memory accumulation
        dl.reset_arena();
    }
    
    Ok(())
}
```

## Implementing Custom Operators with Arena Awareness

Custom operators in DataLogic-rs need to be arena-aware to properly interact with the memory management system. The key concept is that values returned from custom operators must be allocated within the arena to ensure proper lifetime management.

### Custom Operator Interface

```rust
pub trait CustomOperator: fmt::Debug + Send + Sync {
    fn evaluate<'a>(
        &self,
        args: &'a [DataValue<'a>],
        arena: &'a DataArena,
    ) -> Result<&'a DataValue<'a>>;
}
```

### Example: A Simple Custom Operator

Here's how to implement a custom operator that multiplies all numeric arguments:

```rust
use datalogic_rs::{CustomOperator, DataLogic, DataValue, LogicError, Result};
use datalogic_rs::value::NumberValue;
use std::fmt::Debug;
use datalogic_rs::arena::DataArena;

#[derive(Debug)]
struct MultiplyAll;

impl CustomOperator for MultiplyAll {
    fn evaluate<'a>(&self, args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
        // Default to 1 if no arguments provided
        if args.is_empty() {
            return Ok(arena.alloc(DataValue::Number(NumberValue::from_i64(1))));
        }
        
        // Calculate product of all numeric values
        let mut product = 1.0;
        for arg in args {
            if let Some(n) = arg.as_f64() {
                product *= n;
            }
        }
        
        // Allocate the result in the arena
        Ok(arena.alloc(DataValue::Number(NumberValue::from_f64(product))))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut dl = DataLogic::new();
    
    // Register the custom operator
    dl.register_custom_operator("multiply_all", Box::new(MultiplyAll));
    
    // Use the custom operator
    let result = dl.evaluate_str(
        r#"{"multiply_all": [2, 3, 4]}"#,
        r#"{}"#,
        None
    )?;
    
    println!("Result: {}", result);  // Output: 24
    Ok(())
}
```

### Key Points for Custom Operators

1. **Always Use the Arena for Allocations**: Use `arena.alloc()` for any new values you create
2. **Return References from the Arena**: Values must be allocated in the arena to have proper lifetimes
3. **Use Arena Helper Methods**: For complex data structures, use methods like:
   - `arena.alloc_str()` for string values
   - `arena.alloc_slice_copy()` for array/slice values
   - `arena.get_data_value_vec()` for temporary vectors during construction

### Complex Example: Custom Operator with Collections

Here's a more complex example that demonstrates working with collections:

```rust
use datalogic_rs::{CustomOperator, DataLogic, DataValue, LogicError, Result};
use std::fmt::Debug;
use datalogic_rs::arena::DataArena;

#[derive(Debug)]
struct FilterEven;

impl CustomOperator for FilterEven {
    fn evaluate<'a>(&self, args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
        if args.is_empty() {
            return Ok(arena.empty_array_value());
        }
        
        // Create a temporary vector for building our result
        let mut even_numbers = arena.get_data_value_vec();
        
        // Process the first argument, which should be an array
        if let Some(array) = args[0].as_array() {
            for value in array {
                if let Some(num) = value.as_i64() {
                    if num % 2 == 0 {
                        even_numbers.push(DataValue::Number(num.into()));
                    }
                }
            }
        }
        
        // Convert our temporary vector to a permanent slice in the arena
        let result_slice = arena.bump_vec_into_slice(even_numbers);
        
        // Create and return the result array
        Ok(arena.alloc(DataValue::Array(result_slice)))
    }
}
```

## Advanced Arena Features

For advanced users who directly interact with the arena:

### String Interning

The arena uses string interning to store duplicate strings only once:

```rust
// Without interning (creates multiple copies of the same string)
let s1 = arena.alloc_str("hello");
let s2 = arena.alloc_str("hello");

// With interning (reuses the same string)
let s1 = arena.intern_str("hello");
let s2 = arena.intern_str("hello");
```

### Temporary Vectors

When building complex data structures, you can use temporary vectors:

```rust
// Get a temporary vector backed by the arena
let mut temp_vec = arena.get_data_value_vec();

// Add elements to it
temp_vec.push(DataValue::Number(1.into()));
temp_vec.push(DataValue::Number(2.into()));
temp_vec.push(DataValue::Number(3.into()));

// Convert to a permanent slice in the arena
let slice = arena.bump_vec_into_slice(temp_vec);

// Create a DataValue array from the slice
let array_value = DataValue::Array(slice);
```

## Best Practices

1. **Reset the Arena Regularly**: For long-running applications, call `reset_arena()` periodically
2. **Tune Chunk Size**: Use `DataLogic::with_chunk_size()` to optimize for your workload
3. **Don't Hold Old References**: After `reset_arena()`, all previous data values are invalid
4. **Use Helper Methods**: For custom operators, use the arena's helper methods for allocation
5. **Parse Once, Evaluate Many**: Parse rules once and reuse them for better performance

## Performance Considerations

- Arena allocation is typically 10-100x faster than general-purpose allocation
- String interning can significantly reduce memory usage for repeated strings
- Properly sized chunks reduce fragmentation and improve cache locality
- Temporary vectors can help build complex structures efficiently

By understanding and properly using the arena allocation system in DataLogic-rs, you can achieve optimal performance and memory efficiency in your applications. 