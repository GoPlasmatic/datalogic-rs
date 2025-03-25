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
use datalogic_rs::DataLogic;
use serde_json::json;

// Create a new DataLogic instance with its own arena
let logic = DataLogic::new();

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
let result = logic.evaluate(&rule, &logic.parse_data(&data.to_string()).unwrap()).unwrap();
assert!(result.to_json().as_bool().unwrap());
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
use datalogic_rs::DataLogic;
use serde_json::json;

fn main() {
    // Create a DataLogic instance
    let dl = DataLogic::new();
    
    // Parse and evaluate a rule in one step
    let result = dl.evaluate_str(
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
use datalogic_rs::{DataLogic, CustomOperator, LogicError};
use serde_json::{json, Value};
use std::borrow::Cow;

// Define a custom power operator
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

// Create a DataLogic instance
let mut dl = DataLogic::new();

// Register the operator
dl.register_custom_operator(Box::new(PowerOperator));

// Use in rules
let result = dl.evaluate_str(
    r#"{"pow": [2, 3]}"#,
    r#"{}"#,
    None
).unwrap();

assert_eq!(result.as_f64().unwrap(), 8.0);
```

---

## **🎯 Use Cases**
`datalogic-rs` is ideal for **rule-based decision engines** in:
- **Feature flagging** (Enable features dynamically based on user attributes)
- **Dynamic pricing** (Apply discounts or surge pricing based on conditions)
- **Fraud detection** (Evaluate transaction risk using JSON-based rules)
- **Form validation** (Check field dependencies dynamically)
- **Authorization rules** (Implement complex access control policies)
- **Business rule engines** (Enforce business policies with configurable rules)

---

## **📊 Performance**
**Benchmark results show** `datalogic-rs` is **30% faster** than the next fastest JSONLogic implementations, thanks to:
- Arena-based memory management
- Static operator dispatch
- Zero-copy deserialization
- Optimized rule compilation

---

## **🛠️ Contributing**
We welcome contributions! See the [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

📜 **License**: Apache-2.0

---

### **🚀 Next Steps**
✅ Try out `datalogic-rs` today!  
📖 Check out the [API documentation](./API.md) for detailed usage instructions  
📚 See the [docs.rs documentation](https://docs.rs/datalogic-rs) for comprehensive reference  
⭐ Star the [GitHub repository](https://github.com/json-logic/datalogic-rs) if you find it useful!

