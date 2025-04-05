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

## Memory Management

The library uses an arena allocator for efficient memory use. For long-running applications processing many rules:

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

## Complete API Reference

For a full list of available methods and types, refer to the Rust documentation:

```
cargo doc --open
``` 