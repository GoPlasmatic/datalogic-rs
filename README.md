<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

  # datalogic-rs

  **A high-performance, production-ready Rust implementation of JSONLogic.**

  *Evaluate complex rules and dynamic expressions with a powerful, memory-efficient, and developer-friendly engine.*

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

`datalogic-rs` is a Rust implementation of [JSONLogic](http://jsonlogic.com) designed for performance, safety, and ease of use. Whether you're building a feature flagging system, a dynamic pricing engine, or a complex validation pipeline, `datalogic-rs` provides the power and flexibility you need.

**Version 4 Update:** We've moved to a more ergonomic API in v4, prioritizing developer experience and maintainability over micro-optimizations. If you need ultra-high performance with arena allocation (30x faster for specific workloads), v3 is still maintained and available. Both v3 and v4 will continue to exist in parallel, letting you choose between maximum performance (v3) or better ergonomics (v4).

## 🚀 What Makes `datalogic-rs` v4 Awesome?

- **Thread-Safe by Design:** Compile once, evaluate across multiple threads with zero synchronization overhead.
- **Ergonomic API:** Simple, intuitive API using standard `serde_json::Value` types.
- **100% JSONLogic Compliant:** Full compatibility with the official JSONLogic test suite.
- **Extensible:** Easily add custom operators with our straightforward trait system.
- **Powerful Templating:** Preserve object structures for dynamic, structured output.
- **Production Ready:** Proven in real-world workloads with comprehensive test coverage.
- **Batteries Included:** Over 50 built-in operators, including datetime and regex support.
- **Async Compatible:** Works seamlessly with Tokio and other async runtimes.

## 🏗️ How It Works: Compilation-Based Optimization

`datalogic-rs` v4 uses a compilation step to optimize rule evaluation. Parse and compile your rules once, then evaluate them many times across different threads with zero overhead.

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

### Structured Object Preservation

Generate powerful, structured outputs by preserving non-operator keys in your JSON.

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

## 🎯 Getting Started: Core API

### 1. `compile` + `evaluate` - For Maximum Performance

Compile once, evaluate many times across threads with zero overhead.

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

### 2. `evaluate_json` - For Quick Evaluations

Perfect for one-shot evaluations or when you don't need to reuse compiled logic.

```rust
use datalogic_rs::DataLogic;

let engine = DataLogic::new();

let result = engine.evaluate_json(
    r#"{ "abs": -42 }"#,
    r#"{}"#, // No data needed for this rule
).unwrap();

assert_eq!(result, json!(42));
```

### 3. Thread-Safe Evaluation

Share compiled logic across threads for parallel processing.

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

## 🔧 Installation

Add `datalogic-rs` to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "4.0"

# Or use v3 if you need arena-based allocation for maximum performance
# datalogic-rs = "3.0"
```

## 📖 Real-World Examples

### Complex Business Rules

Check for multi-condition eligibility with nested logic.

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

### Array Processing & Filtering

Filter and map user data with ease.

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

Perform date arithmetic with timezone support.

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

### Regex Data Extraction

Extract structured data from strings using named capture groups.

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

## 🔌 Custom Operators

Extend `datalogic-rs` with your own domain-specific logic.

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

## 🚀 Async & Tokio Support

`datalogic-rs` v4 is fully compatible with async runtimes like Tokio:

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

## 🎯 Use Cases

### Feature Flagging

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

## 📋 Supported Operations

| Category         | Operators                                                     |
| ---------------- | ------------------------------------------------------------- |
| **Comparison**   | `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`                |
| **Logic**        | `and`, `or`, `!`, `!!`                                        |
| **Arithmetic**   | `+`, `-`, `*`, `/`, `%`, `min`, `max`, `abs`, `ceil`, `floor`  |
| **Control Flow** | `if`, `?:`, `??`                                              |
| **Arrays**       | `map`, `filter`, `reduce`, `all`, `some`, `none`, `merge`, `in`, `length`, `slice`, `sort` |
| **Strings**      | `cat`, `substr`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `replace`, `split` |
| **Data Access**  | `var`, `val`, `exists`, `missing`, `missing_some`             |
| **DateTime**     | `datetime`, `timestamp`, `now`, `parse_date`, `format_date`, `date_diff` |
| **Error Handling**| `throw`, `try`                                                |
| **Custom**       | User-defined operators                                        |

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for details on how to get started.

## 🏢 About Plasmatic

`datalogic-rs` is developed by [Plasmatic](https://github.com/GoPlasmatic), an organization dedicated to building open-source tools for financial infrastructure and data processing.

Check out our other projects:

- [DataFlow-rs](https://github.com/GoPlasmatic/dataflow-rs): An event-driven workflow orchestration engine written in Rust that empowers you to define and execute data pipelines as code.

## 📄 License

`datalogic-rs` is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

-----

<div align="center">
<p>Built with ❤️ by the <a href="https://github.com/GoPlasmatic">Plasmatic</a> team</p>
</div>
