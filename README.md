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

## Getting Started

### 1. Basic Usage

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let logic = json!({ ">": [{ "var": "age" }, 18] });
let compiled = engine.compile(&logic).unwrap();

let result = engine.evaluate_owned(&compiled, json!({ "age": 21 })).unwrap();
assert_eq!(result, json!(true));
```

### 2. Quick JSON Evaluation

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let result = engine.evaluate_json(
    r#"{ "+": [1, 2] }"#,
    r#"{}"#
).unwrap();
assert_eq!(result, json!(3));
```

### 3. Thread-Safe Evaluation

```rust
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;

let engine = Arc::new(DataLogic::new());
let logic = json!({ "*": [{ "var": "x" }, 2] });
let compiled = engine.compile(&logic).unwrap();

// Share across threads
let engine2 = Arc::clone(&engine);
let compiled2 = Arc::clone(&compiled);
std::thread::spawn(move || {
    engine2.evaluate_owned(&compiled2, json!({ "x": 5 })).unwrap();
});
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

### Conditional Logic

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let logic = json!({
    "if": [
        { ">=": [{ "var": "age" }, 18] },
        "adult",
        "minor"
    ]
});

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({ "age": 25 })).unwrap();
assert_eq!(result, json!("adult"));
```

### Array Operations

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let logic = json!({
    "filter": [
        [1, 2, 3, 4, 5],
        { ">": [{ "var": "" }, 2] }
    ]
});

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
assert_eq!(result, json!([3, 4, 5]));
```

### String Operations

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let logic = json!({ "cat": ["Hello, ", { "var": "name" }, "!"] });

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({ "name": "World" })).unwrap();
assert_eq!(result, json!("Hello, World!"));
```

### Math Operations

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let logic = json!({ "max": [{ "var": "scores" }] });

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({ "scores": [10, 20, 15] })).unwrap();
assert_eq!(result, json!(20));
```

## Custom Operators

Extend the engine with your own logic. **Important:** Custom operators receive unevaluated arguments—you must call `evaluator.evaluate()` to evaluate them:

```rust
use datalogic_rs::{DataLogic, Operator, ContextStack, Evaluator, Result, Error};
use serde_json::{json, Value};

struct DoubleOperator;

impl Operator for DoubleOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Arguments are unevaluated - must call evaluate() first!
        let value = evaluator.evaluate(args.first().unwrap_or(&Value::Null), context)?;
        match value.as_f64() {
            Some(n) => Ok(json!(n * 2.0)),
            None => Err(Error::InvalidArguments("Expected number".to_string()))
        }
    }
}

let mut engine = DataLogic::new();
engine.add_operator("double".to_string(), Box::new(DoubleOperator));

// Works with literals
let result = engine.evaluate_json(r#"{ "double": 21 }"#, r#"{}"#).unwrap();
assert_eq!(result, json!(42.0));

// Also works with variable references (because we evaluate the argument)
let logic = json!({ "double": { "var": "x" } });
let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({ "x": 10 })).unwrap();
assert_eq!(result, json!(20.0));
```

For more examples including averaging, range checking, and string formatting operators, see [`examples/custom_operator.rs`](examples/custom_operator.rs).

## Configuration

Customize evaluation behavior to match your needs:

### Basic Configuration

```rust
use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling};
use serde_json::json;

// Configure how non-numeric values are handled in arithmetic
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::IgnoreValue);
let engine = DataLogic::with_config(config);

// Non-numeric values are ignored instead of throwing errors
let logic = json!({"+": [1, "text", 2]});
let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
assert_eq!(result, json!(3)); // "text" is ignored
```

### Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| **NaN Handling** | How to handle non-numeric values in arithmetic | `ThrowError` |
| **Division by Zero** | How to handle division by zero | `ReturnBounds` |
| **Truthy Evaluator** | How to evaluate truthiness (JavaScript, Python, StrictBoolean, Custom) | `JavaScript` |
| **Loose Equality Errors** | Whether to throw errors for incompatible types in `==` | `true` |
| **Numeric Coercion** | Rules for converting values to numbers | Permissive |

### Custom Truthiness

```rust
use datalogic_rs::{DataLogic, EvaluationConfig, TruthyEvaluator};
use std::sync::Arc;

// Only positive numbers are truthy
let custom_evaluator = Arc::new(|value: &serde_json::Value| -> bool {
    value.as_f64().map_or(false, |n| n > 0.0)
});

let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::Custom(custom_evaluator));
let engine = DataLogic::with_config(config);
```

### Configuration Presets

```rust
// Safe arithmetic - ignores invalid values
let engine = DataLogic::with_config(EvaluationConfig::safe_arithmetic());

// Strict mode - throws more errors
let engine = DataLogic::with_config(EvaluationConfig::strict());
```

### Combining Configuration with Structure Preservation

```rust
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::CoerceToZero);
let engine = DataLogic::with_config_and_structure(config, true);
```

## Advanced Features

### Nested Data Access

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let logic = json!({ "var": "user.address.city" });

let data = json!({
    "user": {
        "address": {
            "city": "New York"
        }
    }
});

let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, data).unwrap();
assert_eq!(result, json!("New York"));
```

### Error Handling

The `try` and `throw` operators provide exception-like error handling:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// Basic try/catch - returns fallback on error
let logic = json!({
    "try": [
        { "/": [10, { "var": "divisor" }] },
        0  // Fallback value if division fails
    ]
});
let compiled = engine.compile(&logic).unwrap();

let result = engine.evaluate_owned(&compiled, json!({ "divisor": 0 })).unwrap();
assert_eq!(result, json!(0));  // Division by zero caught, returns fallback

// Throw custom errors
let logic = json!({
    "if": [
        { "<": [{ "var": "age" }, 0] },
        { "throw": { "code": "INVALID_AGE", "message": "Age cannot be negative" } },
        { "var": "age" }
    ]
});

// Access error details in catch block
let logic = json!({
    "try": [
        { "throw": { "code": 404, "message": "Not found" } },
        { "cat": ["Error: ", { "var": "message" }] }  // Access thrown error properties
    ]
});
let compiled = engine.compile(&logic).unwrap();
let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
assert_eq!(result, json!("Error: Not found"));
```

### Structured Objects (Templating)

Enable `preserve_structure` mode to use JSONLogic as a templating engine. Unknown keys become output fields instead of being treated as operators:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::with_preserve_structure();

// Template with mixed operators and literal fields
let template = json!({
    "user": {
        "name": { "var": "firstName" },
        "email": { "var": "userEmail" },
        "verified": true
    },
    "generatedAt": { "now": [] }
});

let compiled = engine.compile(&template).unwrap();
let data = json!({
    "firstName": "Alice",
    "userEmail": "alice@example.com"
});

let result = engine.evaluate_owned(&compiled, data).unwrap();
// Result: {
//   "user": { "name": "Alice", "email": "alice@example.com", "verified": true },
//   "generatedAt": "2024-01-15T10:30:00Z"
// }
```

This is useful for:
- API response transformation
- Dynamic document generation
- Configuration templating

For more examples, see [`examples/structured_objects.rs`](examples/structured_objects.rs).

## Async Support

Works seamlessly with async runtimes:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let engine = Arc::new(DataLogic::new());
    let logic = json!({ "*": [{ "var": "x" }, 2] });
    let compiled = engine.compile(&logic).unwrap();

    let handle = tokio::task::spawn_blocking(move || {
        engine.evaluate_owned(&compiled, json!({ "x": 5 }))
    });

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result, json!(10));
}
```

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

