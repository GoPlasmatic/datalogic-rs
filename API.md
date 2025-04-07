# DataLogic-rs API Guide

This guide documents the public API for the DataLogic-rs crate, which provides a Rust implementation for evaluating JSON Logic rules.

## Core Types

The library exposes several key types that most users will need:

- `DataLogic`: The main entry point for parsing and evaluating logic rules
- `DataValue`: A memory-efficient value type for representing JSON-like data
- `Logic`: Represents a compiled logic rule ready for evaluation
- `LogicError`: Error type for all operations in the library
- `Result<T>`: Alias for `std::result::Result<T, LogicError>`

## API Overview

The DataLogic-rs library provides multiple ways to evaluate rules, depending on your specific needs:

| Method | Input Types | Output Type | Use Case |
|--------|------------|-------------|----------|
| `evaluate` | `Logic`, `DataValue` | `&DataValue` | Best for reusing parsed rules and data |
| `evaluate_json` | `&JsonValue`, `&JsonValue` | `JsonValue` | Working directly with JSON values |
| `evaluate_str` | `&str`, `&str` | `JsonValue` | One-step parsing and evaluation from strings |

## Basic Usage

Here's a simple example of using the library with `evaluate_str`:

```rust
use datalogic_rs::DataLogic;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new DataLogic instance
    let dl = DataLogic::new();
    
    // Parse and evaluate in one step
    let result = dl.evaluate_str(
        r#"{ ">": [{"var": "temp"}, 100] }"#,  // Logic rule
        r#"{"temp": 110, "name": "user"}"#,    // Data
        None                                    // Use default parser
    )?;
    
    println!("Result: {}", result);  // Output: true
    Ok(())
}
```

## Core API Methods

### Method 1: `evaluate` - For Maximum Reusability

When you need to reuse rules or data across multiple evaluations:

```rust
use datalogic_rs::DataLogic;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dl = DataLogic::new();
    
    // Parse the rule and data separately
    let rule = dl.parse_logic(r#"{ ">": [{"var": "temp"}, 100] }"#, None)?;
    let data = dl.parse_data(r#"{"temp": 110}"#)?;
    
    // Evaluate the rule against the data
    let result = dl.evaluate(&rule, &data)?;
    
    println!("Result: {}", result); // Prints: true
    Ok(())
}
```

This approach is most efficient when:
- Evaluating the same rule against different data sets
- Evaluating different rules against the same data
- You need fine-grained control over the parsing and evaluation steps

### Method 2: `evaluate_str` - One-Step Evaluation

For quick, one-time evaluations from string inputs:

```rust
use datalogic_rs::DataLogic;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dl = DataLogic::new();
    
    // Parse and evaluate in one step
    let result = dl.evaluate_str(
        r#"{ "abs": -42 }"#,
        r#"{}"#,
        None
    )?;
    
    println!("Result: {}", result); // Prints: 42
    Ok(())
}
```

### Method 3: `evaluate_json` - Working with JSON Values

When your application already has the rule and data as `serde_json::Value` objects:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dl = DataLogic::new();
    
    // Use serde_json values directly
    let logic = json!({"ceil": 3.14});
    let data = json!({});
    
    // Evaluate using the JSON values
    let result = dl.evaluate_json(&logic, &data, None)?;
    
    println!("Result: {}", result); // Prints: 4
    Ok(())
}
```

## Parsing Methods

DataLogic-rs provides methods to parse rules and data separately:

### Logic Parsing

- `parse_logic(&self, source: &str, format: Option<&str>) -> Result<Logic>`: Parse a logic rule from a string
- `parse_logic_json(&self, source: &JsonValue, format: Option<&str>) -> Result<Logic>`: Parse a logic rule from a JSON value

### Data Parsing

- `parse_data(&self, source: &str) -> Result<DataValue>`: Parse data from a string
- `parse_data_json(&self, source: &JsonValue) -> Result<DataValue>`: Parse data from a JSON value

## Arena-Based Memory Management

DataLogic-rs uses an arena-based memory management system for efficient allocation and deallocation of values during rule evaluation. This approach significantly improves performance and reduces memory overhead.

### Memory Management Methods

- `DataLogic::with_chunk_size(size: usize) -> Self`: Create a new instance with a specific arena chunk size
- `reset_arena(&mut self)`: Reset the arena to free all allocated memory

### Using the Arena in Long-Running Applications

For long-running applications or when processing many rules, periodically reset the arena to prevent excessive memory usage:

```rust
use datalogic_rs::{DataLogic, Result};

fn process_batches(batches: Vec<(String, String)>) -> Result<()> {
    let mut dl = DataLogic::new();
    
    for (rule_str, data_str) in batches {
        // Process each batch
        let result = dl.evaluate_str(&rule_str, &data_str, None)?;
        println!("Result: {}", result);
        
        // Reset the arena to free memory after processing a batch
        dl.reset_arena();
    }
    
    Ok(())
}
```

### Best Practices for Arena Management

1. **Reset Periodically**: Call `reset_arena()` after processing batches of rules to free memory
2. **Tune Chunk Size**: For memory-sensitive applications, customize the arena chunk size
3. **Reuse Parsed Rules**: Parse rules once and reuse them to avoid repeated parsing costs
4. **Beware of Dangling References**: After `reset_arena()` is called, all previously returned values become invalid

For more detailed information on using the arena, see the [ARENA.md](ARENA.md) document.

## Error Handling

All operations that can fail return a `Result<T, LogicError>` which should be properly handled:

```rust
use datalogic_rs::{DataLogic, LogicError, Result};

fn process_input(rule: &str, data: &str) -> Result<()> {
    let dl = DataLogic::new();
    
    match dl.evaluate_str(rule, data, None) {
        Ok(result) => {
            println!("Success: {}", result);
            Ok(())
        },
        Err(LogicError::ParseError { reason }) => {
            eprintln!("Parse error: {}", reason);
            Err(LogicError::ParseError { reason })
        },
        Err(err) => {
            eprintln!("Other error: {}", err);
            Err(err)
        }
    }
}
```

## Performance Considerations

- Use `DataLogic::with_chunk_size()` to tune memory allocation for your workload
- Parse rules once and reuse them with different data inputs using the `evaluate` method
- Use `reset_arena()` periodically for long-running applications
- Choose the most appropriate method based on your input format:
  - Already have `serde_json::Value`? Use `evaluate_json`
  - Working with strings? Use `evaluate_str`
  - Need to reuse rules/data? Parse separately and use `evaluate`

## Custom Operators

The library supports extending its functionality with custom operators. These operators need to be arena-aware to properly interact with the memory management system.

### Implementing Custom Operators

Custom operators in DataLogic-rs implement the `CustomOperator` trait, which requires an `evaluate` method that takes arguments and returns a result allocated within the arena:

```rust
use datalogic_rs::{CustomOperator, DataLogic, DataValue, Result};
use datalogic_rs::value::NumberValue;
use datalogic_rs::arena::DataArena;
use std::fmt::Debug;

// 1. Define a struct that implements the CustomOperator trait
#[derive(Debug)]
struct PowerOperator;

impl CustomOperator for PowerOperator {
    fn evaluate<'a>(&self, args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
        if args.len() != 2 {
            return Err(LogicError::InvalidArgument {
                reason: "Power operator requires exactly 2 arguments".to_string(),
            });
        }
        
        if let (Some(base), Some(exp)) = (args[0].as_f64(), args[1].as_f64()) {
            // Allocate the result in the arena
            return Ok(arena.alloc(DataValue::Number(NumberValue::from_f64(base.powf(exp)))));
        }
        
        Err(LogicError::InvalidArgument {
            reason: "Arguments must be numbers".to_string(),
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut dl = DataLogic::new();
    
    // 2. Register the custom operator with DataLogic
    dl.register_custom_operator("pow", Box::new(PowerOperator));
    
    // 3. Use the custom operator in your logic expressions
    let result = dl.evaluate_str(
        r#"{"pow": [2, 3]}"#,
        r#"{}"#,
        None
    )?;
    
    println!("2^3 = {}", result); // Prints: 2^3 = 8
    Ok(())
}
```

### Arena Allocation in Custom Operators

When implementing custom operators, follow these arena allocation best practices:

1. **Always allocate results in the arena**: Use `arena.alloc()` for any values you return
2. **Use arena helper methods for collections**:
   - `arena.get_data_value_vec()` - Get a temporary vector for building collections
   - `arena.bump_vec_into_slice()` - Convert a temporary vector to a permanent slice
   - `arena.alloc_str()` - Allocate string values
   - `arena.alloc_slice_copy()` - Allocate arrays of copyable types

3. **Return references from the arena**: The return type must be a reference to a value in the arena

### Working with Collections in Custom Operators

For operators that need to build collections:

```rust
fn evaluate<'a>(&self, args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
    // Create a temporary vector backed by the arena
    let mut temp_vec = arena.get_data_value_vec();
    
    // Add elements to it
    for i in 1..=5 {
        temp_vec.push(DataValue::Number(i.into()));
    }
    
    // Convert to a permanent slice in the arena
    let result_slice = arena.bump_vec_into_slice(temp_vec);
    
    // Create and return a DataValue array allocated in the arena
    Ok(arena.alloc(DataValue::Array(result_slice)))
}
```

For more complex examples and detailed information on using the arena in custom operators, see the [ARENA.md](ARENA.md) document.

### Registration

To register a custom operator with DataLogic:

```rust
dl.register_custom_operator("operator_name", Box::new(OperatorImplementation));
```

### Advanced Use Cases

Custom operators can be combined with built-in operators and data access:

```rust
// Calculate 2 * (base^2) * 3 where base comes from input data
let rule = r#"{
    "*": [
        2,
        {"pow": [{"var": "base"}, 2]},
        3
    ]
}"#;

let data = r#"{"base": 4}"#;

// With base = 4, this calculates 2 * 4² * 3 = 2 * 16 * 3 = 96
let result = dl.evaluate_str(rule, data, None)?;
```

For more examples, see the `examples/custom.rs` file in the repository.

## Complete API Reference

For a full list of available methods and types, refer to the Rust documentation:

```
cargo doc --open
``` 