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

Our secret sauce? An arena-based memory model that delivers zero-copy parsing and evaluation, making it up to **30% faster** than other implementations.

## 🚀 What Makes `datalogic-rs` Awesome?

- **Blazing Fast:** Our arena-based allocator minimizes memory overhead and maximizes speed.
- **100% JSONLogic Compliant:** Full compatibility with the official JSONLogic test suite.
- **Extensible:** Easily add your own custom operators with our simple or advanced APIs.
- **Powerful Templating:** Preserve object structures for dynamic, structured output.
- **Ready for Production:** Thread-safe, statically dispatched, and optimized for real-world workloads.
- **Batteries Included:** Over 50 built-in operators, including datetime and regex support.
- **WASM Support:** Compile to WebAssembly for use in browser environments.

## 🏗️ How It Works: The Arena Advantage

`datalogic-rs` uses an arena allocator for memory-efficient evaluation. This means we can parse a rule once and evaluate it many times with minimal overhead.

```rust
use datalogic_rs::DataLogic;

// Create an evaluator with a custom arena chunk size.
let dl = DataLogic::with_chunk_size(8192);

// Parse your rule and data once.
let rule = dl.parse_logic(r#"{ ">": [{ "var": "temp" }, 100] }"#).unwrap();
let data = dl.parse_data(r#"{ "temp": 110 }"#).unwrap();

// Evaluate efficiently.
let result = dl.evaluate(&rule, &data).unwrap();
assert!(result.to_json().as_bool().unwrap());

// Reset the arena to reuse memory for the next batch of evaluations.
dl.reset_arena();
```

### Structured Object Preservation

Generate powerful, structured outputs by preserving non-operator keys in your JSON.

```rust
use datalogic_rs::DataLogic;

// Enable structure preservation for templating.
let dl = DataLogic::with_preserve_structure();

let result = dl.evaluate_str(
    r#"{
        "status": { "if": [{ ">=": [{ "var": "score" }, 90] }, "pass", "fail"] },
        "grade": { "+": [{ "var": "score" }, { "var": "bonus" }] },
        "timestamp": { "now": [] }
    }"#,
    r#"{ "score": 85, "bonus": 10 }"#,
).unwrap();

// The result is a structured object with evaluated fields.
// { "status": "pass", "grade": 95, "timestamp": "2024-01-15T10:30:00Z" }
```

## 🎯 Getting Started: Core API

### 1. `evaluate` - For Reusable Rules & Data

Ideal for scenarios where you need to run the same rule against different data.

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

// Parse the rule and data once.
let rule = dl.parse_logic(r#"{ ">": [{ "var": "temp" }, 100] }"#).unwrap();
let data = dl.parse_data(r#"{ "temp": 110 }"#).unwrap();

// Evaluate as many times as you need.
let result = dl.evaluate(&rule, &data).unwrap();
assert!(result.to_json().as_bool().unwrap());
```

### 2. `evaluate_str` - For Quick, One-Shot Evaluations

Perfect for when your rules are dynamic or you only need a single evaluation.

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

let result = dl.evaluate_str(
    r#"{ "abs": -42 }"#,
    r#"{}"#, // No data needed for this rule.
).unwrap();

assert_eq!(result.as_i64().unwrap(), 42);
```

### 3. `evaluate_json` - For Seamless JSON Integration

Works directly with `serde_json::Value` for easy integration into your existing JSON workflows.

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let dl = DataLogic::new();

let logic = json!({
    "if": [
        { ">": [{ "var": "cart.total" }, 100] },
        "Premium",
        "Standard"
    ]
});

let data = json!({ "cart": { "total": 120 } });

let result = dl.evaluate_json(&logic, &data).unwrap();
assert_eq!(result.as_str().unwrap(), "Premium");
```

## 🔧 Installation

Add `datalogic-rs` to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "3.0"
```

## 📖 Real-World Examples

### Complex Business Rules

Check for multi-condition eligibility with nested logic.

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

let result = dl.evaluate_str(
    r#"{
        "and": [
            { ">=": [{ "var": "age" }, 18] },
            { "<": [{ "var": "age" }, 65] },
            { "or": [
                { "==": [{ "var": "subscription" }, "premium"] },
                { ">=": [{ "var": "purchases" }, 5] }
            ] }
        ]
    }"#,
    r#"{ "age": 25, "subscription": "basic", "purchases": 7 }"#,
).unwrap();

assert!(result.as_bool().unwrap());
```

### Array Processing & Filtering

Filter and map user data with ease.

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

let result = dl.evaluate_str(
    r#"{
        "map": [
            {
                "filter": [
                    { "var": "users" },
                    { ">=": [{ "var": "age" }, 18] }
                ]
            },
            { "var": "name" }
        ]
    }"#,
    r#"{
        "users": [
            { "name": "Alice", "age": 20 },
            { "name": "Bob", "age": 15 },
            { "name": "Charlie", "age": 25 }
        ]
    }"#,
).unwrap();

// Returns ["Alice", "Charlie"]
assert_eq!(result.as_array().unwrap().len(), 2);
```

### DateTime Operations

Perform date arithmetic with timezone support.

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

let result = dl.evaluate_str(
    r#"{
        ">": [
            { "+": [
                { "datetime": "2023-07-15T08:30:00Z" },
                { "duration": "2d" }
            ] },
            { "datetime": "2023-07-16T08:30:00Z" }
        ]
    }"#,
    r#"{}"#,
).unwrap();

assert!(result.as_bool().unwrap());
```

### Regex Data Extraction

Extract structured data from strings using named capture groups.

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

let result = dl.evaluate_str(
    r#"{
        "split": [
            "SBININBB101",
            "^(?P<bank>[A-Z]{4})(?P<country>[A-Z]{2})(?P<location>[A-Z0-9]{2})(?P<branch>[A-Z0-9]{3})?$"
        ]
    }"#,
    r#"{}"#,
).unwrap();

// Returns: { "bank": "SBIN", "country": "IN", "location": "BB", "branch": "101" }
let obj = result.as_object().unwrap();
assert_eq!(obj.get("bank").unwrap().as_str().unwrap(), "SBIN");
```

## 🔌 Custom Operators

Extend `datalogic-rs` with your own domain-specific logic.

```rust
use datalogic_rs::{DataLogic, DataValue};
use datalogic_rs::value::NumberValue;

// Define a custom operator that doubles a number.
fn double(args: Vec<DataValue>, _data: DataValue) -> Result<DataValue, String> {
    if let Some(n) = args.first().and_then(|v| v.as_f64()) {
        return Ok(DataValue::Number(NumberValue::from_f64(n * 2.0)));
    }
    Err("Argument must be a number".to_string())
}

let mut dl = DataLogic::new();
dl.register_simple_operator("double", double);

// Use your custom operator in a rule.
let result = dl.evaluate_str(r#"{ "double": 21 }"#, r#"{}"#).unwrap();

assert_eq!(result.as_f64().unwrap(), 42.0);
```

## 📊 Performance

`datalogic-rs` is fast. Here's how it stacks up against other implementations on an Apple M2 Pro:

| Implementation                    | Execution Time | Relative Performance |
| --------------------------------- | -------------- | -------------------- |
| **`datalogic-rs`**                | **380ms**      | **1.0x (baseline)**  |
| `json-logic-engine` (pre-compiled) | 417ms          | 1.1x slower          |
| `json-logic-engine` (interpreted) | 986ms          | 2.6x slower          |
| `json-logic-js`                   | 5,755ms        | 15.1x slower         |

*Benchmarks run on the standard JSONLogic test suite.*

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

- [SwiftMTMessage](https://github.com/GoPlasmatic/SwiftMTMessage): A library for parsing SWIFT MT messages with CBPR+ compliance.
- [Reframe](https://github.com/GoPlasmatic/Reframe): A transformation engine for converting SWIFT MT messages to ISO 20022.
- [MXMessage](https://github.com/GoPlasmatic/MXMessage): A library for parsing ISO 20022 (MX) messages.

## 📄 License

`datalogic-rs` is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

-----

<div align="center">
<p>Built with ❤️ by the <a href="https://github.com/GoPlasmatic">Plasmatic</a> team</p>
</div>
