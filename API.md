# DataLogic-rs API Guide

This guide documents the public API for the DataLogic-rs crate, which provides a Rust implementation for evaluating JSON Logic rules.

## Core Types

The library exposes several key types that most users will need:

- `DataLogic`: The main entry point for parsing and evaluating logic rules
- `DataValue`: A memory-efficient value type for representing JSON-like data
- `Logic`: Represents a compiled logic rule ready for evaluation
- `LogicError`: Error type for all operations in the library
- `Result<T>`: Alias for `std::result::Result<T, LogicError>`

## Basic Usage

Here's a simple example of using the library:

```rust
use datalogic::{DataLogic, Result};

fn main() -> Result<()> {
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

## Step-by-Step Usage

For more control, you can break down the process:

```rust
use datalogic::{DataLogic, Result};

fn main() -> Result<()> {
    let dl = DataLogic::new();
    
    // 1. Parse the data
    let data = dl.parse_data(r#"{"temp": 110, "name": "user"}"#)?;
    
    // 2. Parse the logic rule
    let rule = dl.parse_logic(r#"{ ">": [{"var": "temp"}, 100] }"#, None)?;
    
    // 3. Evaluate the rule with the data
    let result = dl.evaluate(&rule, &data)?;
    
    println!("Result: {}", result);
    Ok(())
}
```

## Advanced Usage

### Custom Operators

You can extend the logic system with custom operators by building rules programmatically:

```rust
use datalogic::{DataLogic, Result};

fn main() -> Result<()> {
    let dl = DataLogic::new();
    let data = dl.parse_data(r#"{"name": "Alice"}"#)?;
    
    // Build a rule programmatically
    let rule = dl.builder()
        .var("name")
        .string("Alice")
        .equal()
        .build();
    
    let result = dl.evaluate(&rule, &data)?;
    println!("Result: {}", result);  // Output: true
    
    Ok(())
}
```

### Memory Management

The library uses an arena allocator for efficient memory use. For long-running applications processing many rules:

```rust
use datalogic::{DataLogic, Result};

fn process_batches(batches: Vec<(String, String)>) -> Result<()> {
    let mut dl = DataLogic::new();
    
    for (rule_str, data_str) in batches {
        // Process each batch
        let result = dl.apply(&rule_str, &data_str, None)?;
        println!("Result: {}", result);
        
        // Reset the arena to free memory after processing a batch
        dl.reset_arena();
    }
    
    Ok(())
}
```

## Error Handling

All operations that can fail return a `Result<T, LogicError>` which should be properly handled:

```rust
use datalogic::{DataLogic, LogicError, Result};

fn process_input(rule: &str, data: &str) -> Result<()> {
    let dl = DataLogic::new();
    
    match dl.apply(rule, data, None) {
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
- Parse rules once and reuse them with different data inputs
- Use `reset_arena()` periodically for long-running applications

## Migrating from Previous Versions

If you're migrating from an earlier version of the library, note these changes:

- More consistent method naming
- Better error handling
- More convenience methods for common operations

## Complete API Reference

For a full list of available methods and types, refer to the Rust documentation:

```
cargo doc --open
``` 