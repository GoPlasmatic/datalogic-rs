<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

# datalogic-rs
**A fast, production-ready Rust engine for JSONLogic.**

Effortlessly evaluate complex rules and dynamic expressions with a powerful, memory-efficient, and developer-friendly toolkit.

  [![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
  [![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
  [![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
  [![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
  [![Downloads](https://img.shields.io/crates/d/datalogic-rs)](https://crates.io/crates/datalogic-rs)

  <p>
    <a href="https://github.com/GoPlasmatic">🏢 Organization</a> •
    <a href="https://docs.rs/datalogic-rs">📖 Docs</a> •
    <a href="https://github.com/GoPlasmatic/datalogic-rs/issues">🐛 Report a Bug</a>
  </p>
</div>

-----

`datalogic-rs` brings the power of [JSONLogic](http://jsonlogic.com) to Rust, focusing on speed, safety, and ease of use. Whether you’re building feature flags, dynamic pricing, or complex validation, this engine is designed to be flexible and robust.

**What’s New in v4?**
We’ve redesigned the API to be more ergonomic and developer-friendly. If you need maximum speed with arena allocation, v3 is still available and maintained. Choose v4 for a smoother experience, or stick with v3 for raw performance—both are supported.

## Key Features

- **Thread-Safe:** Compile your logic once, then evaluate it anywhere—no locks, no fuss.
- **Intuitive API:** Works seamlessly with `serde_json::Value`.
- **Fully Compliant:** Passes the official JSONLogic test suite.
- **Extensible:** Add your own operators with a simple trait.
- **Templating Support:** Preserve object structures for dynamic output.
- **Battle-Tested:** Used in production, with thorough test coverage.
- **Feature-Rich:** Over 50 built-in operators, including datetime and regex.
- **Async-Ready:** Integrates smoothly with Tokio and async runtimes.

## How It Works

`datalogic-rs` v4 compiles your rules for fast, repeated evaluation. Parse and compile once, then reuse across threads with zero overhead.

```rust
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;

// Create an engine
let engine = Arc::new(DataLogic::new());

// Compile your rule once
let logic = json!({ ">": [{ "var": "temp" }, 100] });
let compiled = engine.compile(&logic).unwrap(); // Returns Arc<CompiledLogic>

// Evaluate across multiple threads
let handle = std::thread::spawn(move || {
    let data = json!({ "temp": 110 });
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    assert_eq!(result, json!(true));
});
```

### Structured Output

Preserve keys and generate structured results for templating:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

// Enable structure preservation for templating
let engine = DataLogic::with_preserve_structure();

let logic = json!({
    "status": { "if": [{ ">=": [{ "var": "score" }, 90] }, "pass", "fail"] },
    "grade": { "+": [{ "var": "score" }, { "var": "bonus" }] },
    "timestamp": { "now": [] }
});

let data = json!({ "score": 85, "bonus": 10 });

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, data).unwrap();

// The result is a structured object with evaluated fields.
// { "status": "pass", "grade": 95, "timestamp": "2024-01-15T10:30:00Z" }
```

## Getting Started

### 1. Compile & Evaluate (Best Performance)

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// Compile the rule once
let logic = json!({ ">": [{ "var": "temp" }, 100] });
let compiled = engine.compile(&logic).unwrap(); // Returns Arc<CompiledLogic>

// Evaluate with different data
let result1 = engine.evaluate_owned(&compiled, json!({ "temp": 110 })).unwrap();
let result2 = engine.evaluate_owned(&compiled, json!({ "temp": 90 })).unwrap();

assert_eq!(result1, json!(true));
assert_eq!(result2, json!(false));
```

### 2. Quick Evaluation

For one-off checks, use `evaluate_json`:

```rust
use datalogic_rs::DataLogic;

let engine = DataLogic::new();

let result = engine.evaluate_json(
    r#"{ "abs": -42 }"#,
    r#"{}"#, // No data needed for this rule
).unwrap();

assert_eq!(result, json!(42));
```

### 3. Multi-Threaded Evaluation

Share compiled logic for parallel processing:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;
use std::thread;

let engine = Arc::new(DataLogic::new());

let logic = json!({
    "if": [
        { ">": [{ "var": "score" }, 90] },
        "excellent",
        "good"
    ]
});

let compiled = engine.compile(&logic).unwrap();

// Spawn multiple threads
let handles: Vec<_> = vec![95, 85, 92]
    .into_iter()
    .map(|score| {
        let engine = Arc::clone(&engine);
        let compiled = Arc::clone(&compiled);
        
        thread::spawn(move || {
            let data = json!({ "score": score });
            engine.evaluate_owned(&compiled, data).unwrap()
        })
    })
    .collect();

// Collect results
let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
assert_eq!(results, vec![json!("excellent"), json!("good"), json!("excellent")]);
```

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "4.0"

# Or use v3 if you need arena-based allocation for maximum performance
# datalogic-rs = "3.0"
```

## Examples

### Business Rules

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

let logic = json!({
    "and": [
        { ">=": [{ "var": "age" }, 18] },
        { "<": [{ "var": "age" }, 65] },
        { "or": [
            { "==": [{ "var": "subscription" }, "premium"] },
            { ">=": [{ "var": "purchases" }, 5] }
        ] }
    ]
});

let compiled = engine.compile(&logic).unwrap();
let data = json!({ "age": 25, "subscription": "basic", "purchases": 7 });
let result = engine.evaluate_owned(&compiled, data).unwrap();

assert_eq!(result, json!(true));
```

### Array Filtering

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

let logic = json!({
    "map": [
        {
            "filter": [
                { "var": "users" },
                { ">=": [{ "var": "age" }, 18] }
            ]
        },
        { "var": "name" }
    ]
});

let data = json!({
    "users": [
        { "name": "Alice", "age": 20 },
        { "name": "Bob", "age": 15 },
        { "name": "Charlie", "age": 25 }
    ]
});

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, data).unwrap();

// Returns ["Alice", "Charlie"]
assert_eq!(result, json!(["Alice", "Charlie"]));
```

### DateTime Operations

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

let logic = json!({
    ">": [
        { "+": [
            { "datetime": "2023-07-15T08:30:00Z" },
            { "duration": "2d" }
        ] },
        { "datetime": "2023-07-16T08:30:00Z" }
    ]
});

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({})).unwrap();

assert_eq!(result, json!(true));
```

### Regex Extraction

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

let logic = json!({
    "split": [
        "SBININBB101",
        "^(?P<bank>[A-Z]{4})(?P<country>[A-Z]{2})(?P<location>[A-Z0-9]{2})(?P<branch>[A-Z0-9]{3})?$"
    ]
});

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({})).unwrap();

// Returns: { "bank": "SBIN", "country": "IN", "location": "BB", "branch": "101" }
assert_eq!(result["bank"], "SBIN");
assert_eq!(result["country"], "IN");
```

## Custom Operators

Extend the engine with your own logic:

```rust
use datalogic_rs::{DataLogic, Operator, ContextStack, Evaluator, Result};
use serde_json::{json, Value};

// Define a custom operator that doubles a number
struct DoubleOperator;

impl Operator for DoubleOperator {
    fn evaluate(
        &self,
        args: &[Value],
        _context: &mut ContextStack,
        _evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if let Some(n) = args.first().and_then(|v| v.as_f64()) {
            Ok(json!(n * 2.0))
        } else {
            Err("Argument must be a number".into())
        }
    }
}

let mut engine = DataLogic::new();
engine.add_operator("double".to_string(), Box::new(DoubleOperator));

// Use your custom operator in a rule
let logic = json!({ "double": 21 });
let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({})).unwrap();

assert_eq!(result, json!(42.0));
```

## Advanced Features

- **Context Stack:** Access parent context in nested operations.
- **Type Checking:** Validate and branch on input types.
- **Safe Error Handling:** Use `try` for graceful fallback.

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// Access parent context in nested operations
let logic = json!({
    "map": [
        { "var": "items" },
        {
            "cat": [
                { "var": "name" },      // Current item's name
                " - ",
                { "var": "category.1" }  // Parent context's category
            ]
        }
    ]
});

let data = json!({
    "category": "Electronics",
    "items": [
        { "name": "Laptop" },
        { "name": "Phone" }
    ]
});

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, data).unwrap();
// Returns: ["Laptop - Electronics", "Phone - Electronics"]
```

### Type Checking and Validation

```rust
let logic = json!({
    "if": [
        { "==": [{ "type": { "var": "input" } }, "number"] },
        { "*": [{ "var": "input" }, 2] },
        { "throw": "Input must be a number" }
    ]
});
```

### Safe Error Handling with Try

```rust
let logic = json!({
    "try": [
        { "/": [{ "var": "numerator" }, { "var": "denominator" }] },
        "Division failed",  // Default value on error
        { "var": "error" }   // Optional: capture error message
    ]
});
```

## Async Support

`datalogic-rs` v4 works great with async runtimes like Tokio. Compile your logic once, then process data concurrently with ease.

```rust
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let engine = Arc::new(DataLogic::new());
    
    // Compile logic once
    let logic = json!({ "filter": [{ "var": "items" }, { ">": [{ "var": "score" }, 90] }] });
    let compiled = engine.compile(&logic).unwrap();
    
    // Process data concurrently
    let mut handles = vec![];
    
    for batch in data_batches {
        let engine = Arc::clone(&engine);
        let compiled = Arc::clone(&compiled);
        
        handles.push(tokio::spawn(async move {
            // For CPU-intensive operations, use spawn_blocking
            tokio::task::spawn_blocking(move || {
                engine.evaluate_owned(&compiled, batch)
            }).await.unwrap()
        }));
    }
    
    // Collect all results
    let results = futures::future::join_all(handles).await;
}
```

Perfect for web servers, microservices, and real-time data processing pipelines.

## Use Cases

### Feature Flags

```json
{
    "and": [
        { "==": [{ "var": "user.country" }, "US"] },
        { "or": [
            { "==": [{ "var": "user.role" }, "beta_tester"] },
            { ">=": [{ "var": "user.account_age_days" }, 30] }
        ] }
    ]
}
```

### Dynamic Pricing

```json
{
    "if": [
        { ">=": [{ "var": "cart.total" }, 100] },
        { "-": [{ "var": "cart.total" }, { "*": [{ "var": "cart.total" }, 0.1] }] },
        { "var": "cart.total" }
    ]
}
```

### Fraud Detection

```json
{
    "or": [
        { "and": [
            { "!=": [{ "var": "transaction.billing_country" }, { "var": "user.country" }] },
            { ">=": [{ "var": "transaction.amount" }, 1000] }
        ] },
        { ">=": [{ "var": "transaction.attempts_last_hour" }, 5] }
    ]
}
```

## Supported Operators

Over 50 built-in operators, including:

| Category         | Operators                                                     |
| ---------------- | ------------------------------------------------------------- |
| **Comparison**   | `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`                |
| **Logic**        | `and`, `or`, `!`, `!!`                                        |
| **Arithmetic**   | `+`, `-`, `*`, `/`, `%`, `min`, `max`, `abs`, `ceil`, `floor`  |
| **Control Flow** | `if`, `?:` (ternary), `??` (coalesce)                        |
| **Arrays**       | `map`, `filter`, `reduce`, `all`, `some`, `none`, `merge`, `in`, `length`, `slice`, `sort` |
| **Strings**      | `cat`, `substr`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split` |
| **Data Access**  | `var`, `val`, `exists`, `missing`, `missing_some`             |
| **DateTime**     | `datetime`, `timestamp`, `now`, `parse_date`, `format_date`, `date_diff` |
| **Type**         | `type` (returns type name as string)                          |
| **Error Handling**| `throw`, `try`                                                |
| **Special**      | `preserve` (for structured object preservation)               |
| **Custom**       | User-defined operators via `Operator` trait                   |

## Architecture

- **Compilation:** Parses and optimizes logic for fast evaluation.
- **Evaluation:** Uses OpCode dispatch and context stack for speed.
- **Thread-Safe:** Share compiled logic with zero-copy via Arc.

1. **Compilation Phase:** JSON logic is parsed and compiled into a `CompiledLogic` structure with:
   - Static evaluation of constant expressions
   - OpCode assignment for built-in operators
   - Thread-safe Arc wrapping for sharing across threads

2. **Evaluation Phase:** The compiled logic is evaluated against data with:
   - Direct OpCode dispatch (avoiding string lookups)
   - Context stack for nested evaluations
   - Zero-copy operations where possible

## Performance Optimizations

- OpCode dispatch for built-in operators
- Static evaluation of constant expressions
- SmallVec for small arrays
- Arc sharing for thread safety
- Cow types for efficient value passing

## About Plasmatic

Created by [Plasmatic](https://github.com/GoPlasmatic), building open-source tools for financial infrastructure and data processing.

Check out our other projects:

- [DataFlow-rs](https://github.com/GoPlasmatic/dataflow-rs): Event-driven workflow orchestration in Rust.

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE) for details.

