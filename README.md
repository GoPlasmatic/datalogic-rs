# datalogic-rs

[![Release Crates](https://github.com/codetiger/datalogic-rs/actions/workflows/crate-publish.yml/badge.svg)](https://github.com/codetiger/datalogic-rs/actions?query=crate-publish)
[![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
[![crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
[![Downloads](https://img.shields.io/crates/d/datalogic-rs)](https://crates.io/crates/datalogic-rs)

A **lightweight, high-performance** Rust implementation of [JSONLogic](http://jsonlogic.com), optimized for **rule-based decision-making** and **dynamic expressions**.

✨ **Why `datalogic-rs`?**
- 🏆 **Fully JSONLogic-compliant** (100% test coverage)
- 🚀 **Fast & lightweight**: Zero-copy JSON parsing, minimal allocations
- 🔒 **Thread-safe**: Designed for parallel execution
- ⚡ **Optimized for production**: Static dispatch and rule optimization
- 🔌 **Extensible**: Support for custom operators

## Overview

datalogic-rs provides a robust implementation of JSONLogic rules with arena-based memory management for optimal performance. The library provides both a parser for JSON-based rules and a fluent builder API for constructing rules in a type-safe manner.

## Features

- Arena-based memory management for optimal performance
- Comprehensive JSONLogic operator support
- Fluent builder API for type-safe rule construction
- Factory methods for common rule patterns
- Optimizations for static rule components
- Zero copy rule creation and evaluation
- High test coverage and compatibility with standard JSONLogic

## Installation

Add `datalogic-rs` to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "3.0.0"
```

## Usage Examples

### 1. Simple Comparison Rule

**Builder API:**
```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let logic = DataLogic::new();
let builder = logic.builder();

let rule = builder
    .compare()
    .greater_than()
    .var("score")
    .value(50)
    .build();

let data = json!({"score": 75});
let result = logic.evaluate(&rule, &logic.parse_data(&data.to_string()).unwrap()).unwrap();
assert!(result.to_json().as_bool().unwrap());
```

**Raw JSON Evaluation:**
```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let logic = DataLogic::new();
let result = logic.evaluate_str(
    r#"{" > ": [{"var": "score"}, 50]}"#,
    r#"{"score": 75}"#,
    None
).unwrap();
assert!(result.as_bool().unwrap());
```

### 2. Complex Logical Rule (AND/OR)

**Builder API:**
```rust
let rule = builder
    .control()
    .and()
    .add(
        builder
            .compare()
            .greater_than_or_equal()
            .var("age")
            .value(18)
            .build()
    )
    .add(
        builder
            .compare()
            .less_than()
            .var("age")
            .value(65)
            .build()
    )
    .build();

let data = json!({"age": 25});
let result = logic.evaluate(&rule, &logic.parse_data(&data.to_string()).unwrap()).unwrap();
assert!(result.to_json().as_bool().unwrap());
```

**Raw JSON Evaluation:**
```rust
let result = logic.evaluate_str(
    r#"{
        "and": [
            {">=": [{"var": "age"}, 18]},
            {"<": [{"var": "age"}, 65]}
        ]
    }"#,
    r#"{"age": 25}"#,
    None
).unwrap();
assert!(result.as_bool().unwrap());
```

### 3. Array Operations

**Builder API:**
```rust
let adult_names = builder
    .array()
    .map()
    .array(
        builder
            .array()
            .filter()
            .array(builder.var("users"))
            .condition(
                builder
                    .compare()
                    .greater_than_or_equal()
                    .var("age")
                    .value(18)
                    .build()
            )
            .build()
    )
    .mapper(builder.var("name"))
    .build();

let data = json!({
    "users": [
        {"name": "Alice", "age": 20},
        {"name": "Bob", "age": 15},
        {"name": "Charlie", "age": 25}
    ]
});
let result = logic.evaluate(&adult_names, &logic.parse_data(&data.to_string()).unwrap()).unwrap();
assert_eq!(result.to_json().as_array().unwrap().len(), 2);
```

**Raw JSON Evaluation:**
```rust
let result = logic.evaluate_str(
    r#"{
        "map": [
            {
                "filter": [
                    {"var": "users"},
                    {">=": [{"var": "age"}, 18]}
                ]
            },
            {"var": "name"}
        ]
    }"#,
    r#"{
        "users": [
            {"name": "Alice", "age": 20},
            {"name": "Bob", "age": 15},
            {"name": "Charlie", "age": 25}
        ]
    }"#,
    None
).unwrap();
assert_eq!(result.as_array().unwrap().len(), 2);
```

### 4. Conditional Logic (IF)

**Builder API:**
```rust
let rule = builder
    .control()
    .if_()
    .condition(
        builder
            .compare()
            .greater_than()
            .var("cart.total")
            .value(100)
            .build()
    )
    .then(builder.value("Eligible for discount"))
    .else_(builder.value("No discount"))
    .build();

let data = json!({"cart": {"total": 120}});
let result = logic.evaluate(&rule, &logic.parse_data(&data.to_string()).unwrap()).unwrap();
assert_eq!(result.to_json().as_str().unwrap(), "Eligible for discount");
```

**Raw JSON Evaluation:**
```rust
let result = logic.evaluate_str(
    r#"{
        "if": [
            {">": [{"var": "cart.total"}, 100]},
            "Eligible for discount",
            "No discount"
        ]
    }"#,
    r#"{"cart": {"total": 120}}"#,
    None
).unwrap();
assert_eq!(result.as_str().unwrap(), "Eligible for discount");
```

## Performance Benefits

The builder API leverages arena allocation for all rule components, providing several performance benefits:

1. Zero-copy rule construction
2. Reduced memory allocations
3. Improved cache locality
4. Optimization opportunities during construction

## Supported Operations

| Category | Operators |
|----------|-----------|
| **Comparison** | `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=` |
| **Logic** | `and`, `or`, `!`, `!!` |
| **Arithmetic** | `+`, `-`, `*`, `/`, `%`, `min`, `max` |
| **Control Flow** | `if`, `?:`, `??` |
| **Arrays** | `map`, `filter`, `reduce`, `merge`, `all`, `none`, `some` |
| **Strings** | `substr`, `cat`, `in` |
| **Data Access** | `var`, `val`, `exists`, `missing`, `missing_some` |
| **Special** | `preserve`, `throw`, `try` |
| **Custom** | Support for user-defined operators |

## Custom Operators

```rust
use datalogic_rs::{DataLogic, CustomOperator, LogicError};
use serde_json::{json, Value};
use std::borrow::Cow;

struct PowerOperator;

impl CustomOperator for PowerOperator {
    fn name(&self) -> &str {
        "pow"
    }
    
    fn apply<'a>(&self, args: &[Value], _data: &'a Value) -> Result<Cow<'a, Value>, LogicError> {
        if args.len() != 2 {
            return Err(LogicError::InvalidArguments {
                reason: "pow requires 2 arguments".into()
            });
        }
        let base = args[0].as_f64().unwrap_or(0.0);
        let exp = args[1].as_f64().unwrap_or(0.0);
        Ok(Cow::Owned(json!(base.powf(exp))))
    }
}

// Using Builder API
let mut dl = DataLogic::new();
dl.register_custom_operator(Box::new(PowerOperator));
let builder = dl.builder();

let rule = builder
    .custom("pow")
    .args(vec![
        builder.value(2).build(),
        builder.value(3).build()
    ])
    .build();

let result = dl.evaluate(&rule, &dl.parse_data("{}").unwrap()).unwrap();
assert_eq!(result.to_json().as_f64().unwrap(), 8.0);

// Using Raw JSON
let result = dl.evaluate_str(
    r#"{"pow": [2, 3]}"#,
    r#"{}"#,
    None
).unwrap();
assert_eq!(result.as_f64().unwrap(), 8.0);
```

## Use Cases

`datalogic-rs` is ideal for **rule-based decision engines** in:
- **Feature flagging** (Enable features dynamically based on user attributes)
- **Dynamic pricing** (Apply discounts or surge pricing based on conditions)
- **Fraud detection** (Evaluate transaction risk using JSON-based rules)
- **Form validation** (Check field dependencies dynamically)
- **Authorization rules** (Implement complex access control policies)
- **Business rule engines** (Enforce business policies with configurable rules)

## Performance

**Benchmark results show** `datalogic-rs` is **30% faster** than the next fastest JSONLogic implementations, thanks to:
- Arena-based memory management
- Static operator dispatch
- Zero-copy deserialization
- Optimized rule compilation

## Contributing

We welcome contributions! See the [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

## License

Licensed under Apache License, Version 2.0

---

### Next Steps
✅ Try out `datalogic-rs` today!  
📖 Check out the [API documentation](./API.md) for detailed usage instructions  
📚 See the [docs.rs documentation](https://docs.rs/datalogic-rs) for comprehensive reference  
⭐ Star the [GitHub repository](https://github.com/json-logic/datalogic-rs) if you find it useful!

