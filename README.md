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

## Using the Builder API

The builder API provides a fluent interface for creating JSONLogic rules in a type-safe manner. All memory allocations happen directly in the arena for maximum performance.

```rust
use datalogic_rs::JsonLogic;
use serde_json::json;

// Create a new JSONLogic instance with its own arena
let logic = JsonLogic::new();

// Get a builder that uses the arena
let builder = logic.builder();

// Build a rule using the fluent API
let rule = builder
    .compare()
    .greater_than()
    .var("score")
    .value(50)
    .build();

// Evaluate the rule with data
let data = json!({"score": 75});
let result = logic.apply_logic(&rule, &data).unwrap();
assert_eq!(result, json!(true));
```

### Building More Complex Rules

You can build complex rules by composing simpler ones:

```rust
// Create a rule that checks if a person is an adult of working age
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
```

### Using the Factory

For common rule patterns, you can use the factory methods:

```rust
// Create a factory
let factory = logic.factory();

// Create a between rule (inclusive)
let between_rule = factory.between_inclusive("age", 18, 65);

// Create an "is one of" rule
let is_one_of_rule = factory.is_one_of("status", vec!["active", "pending"]);
```

### Working with Arrays

The library provides builders for array operations like map, filter, and reduce:

```rust
// Filter users by age and get their names
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
```

## Performance Benefits

The builder API leverages arena allocation for all rule components, providing several performance benefits:

1. Zero-copy rule construction
2. Reduced memory allocations
3. Improved cache locality
4. Optimization opportunities during construction

## License

Licensed under Apache License, Version 2.0

---

## **📦 Installation**

Add `datalogic-rs` to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "2.0.17"
```

---

## **🚀 Quick Start: Evaluating JSONLogic Rules**

```rust
use datalogic_rs::{JsonLogic, Rule};
use serde_json::json;

fn main() {
    let rule = Rule::from_value(&json!({
        "if": [
            {">": [{"var": "cart.total"}, 100]},
            "Eligible for discount",
            "No discount"
        ]
    })).unwrap();

    let data = json!({"cart": {"total": 120}});
    let result = JsonLogic::apply(&rule, &data).unwrap();
    
    assert_eq!(result, json!("Eligible for discount"));
}
```

---

## **🛠️ Features**
### **✅ Supported Operations**
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

### **💡 Advanced Features**
- **Static Optimization**: Rules are optimized at compile-time
- **Error Handling**: Built-in error handling with `try` operator
- **Memory Efficiency**: Zero-copy JSON deserialization
- **Type Coercion**: JSONLogic-compliant type conversions
- **Thread Safety**: All operations are thread-safe
- **Custom Operators**: Extend with your own operators

### **🔌 Custom Operators**
```rust
use datalogic_rs::{JsonLogic, CustomOperator, Error};
use serde_json::{json, Value};
use std::borrow::Cow;

// Define a custom power operator
struct PowerOperator;

impl CustomOperator for PowerOperator {
    fn name(&self) -> &str {
        "pow"
    }
    
    fn apply<'a>(&self, args: &[Value], _data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("pow requires 2 arguments".into()));
        }
        let base = args[0].as_f64().unwrap_or(0.0);
        let exp = args[1].as_f64().unwrap_or(0.0);
        Ok(Cow::Owned(json!(base.powf(exp))))
    }
}

// Register the operator
JsonLogic::global().add_operator(PowerOperator)?;

// Use in rules
let rule = Rule::from_value(&json!({"pow": [2, 3]}))?;
let result = JsonLogic::apply(&rule, &json!({}))?;
assert_eq!(result, json!(8.0));
```

---

## **🎯 Use Cases**
`datalogic-rs` is ideal for **rule-based decision engines** in:
- **Feature flagging** (Enable features dynamically based on user attributes)
- **Dynamic pricing** (Apply discounts or surge pricing based on conditions)
- **Fraud detection** (Evaluate transaction risk using JSON-based rules)
- **Form validation** (Check field dependencies dynamically)

---

## **📊 Performance**
**Benchmark results show** `datalogic-rs` is **2x faster** than other JSONLogic implementations, thanks to:
- Static operator dispatch
- Zero-copy deserialization
- Optimized rule compilation


To run benchmarks:
```bash
cargo bench
```

---

## **🛠️ Contributing**
We welcome contributions! See the [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

📜 **License**: Apache-2.0

---

### **🚀 Next Steps**
✅ Try out `datalogic-rs` today!  
📖 Check out the [docs.rs documentation](https://docs.rs/datalogic-rs)  
⭐ Star the [GitHub repository](https://github.com/json-logic/datalogic-rs) if you find it useful!

