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
- 🏗️ **Structured output**: Support for structured object preservation and templating

## Overview

datalogic-rs provides a robust implementation of JSONLogic rules with arena-based memory management for optimal performance. The library features comprehensive operator support, optimizations for static rule components, and high test coverage.

## Features

- Arena-based memory management for optimal performance
- Comprehensive JSONLogic operator support
- Optimizations for static rule components
- Zero copy rule creation and evaluation
- High test coverage and compatibility with standard JSONLogic
- Intuitive API for creating, parsing, and evaluating rules
- Structured object preservation for powerful output templating

## Installation

Add `datalogic-rs` to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "3.0.12"
```

## Core API Methods

datalogic-rs provides three primary API methods for evaluating rules, each suited for different use cases:

### 1. `evaluate` - For reusing parsed rules and data

Best for scenarios where the same rule will be evaluated against different data contexts, or vice versa.

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

// Parse rule and data once
let rule = dl.parse_logic(r#"{ ">": [{"var": "temp"}, 100] }"#, None).unwrap();
let data = dl.parse_data(r#"{"temp": 110}"#).unwrap();

// Evaluate the rule against the data
let result = dl.evaluate(&rule, &data).unwrap();
assert!(result.to_json().as_bool().unwrap());
```

### 2. `evaluate_str` - One-step parsing and evaluation

Ideal for one-time evaluations or when rules are dynamically generated.

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

// Parse and evaluate in one step
let result = dl.evaluate_str(
    r#"{ "abs": -42 }"#,
    r#"{}"#,
    None
).unwrap();

assert_eq!(result.as_i64().unwrap(), 42);
```

### 3. `evaluate_json` - Work directly with JSON values

Perfect when your application already has the rule and data as serde_json Values.

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let dl = DataLogic::new();

// Use serde_json values directly
let logic = json!({
    "if": [
        {">": [{"var": "cart.total"}, 100]},
        "Eligible for discount",
        "No discount"
    ]
});
let data = json!({"cart": {"total": 120}});

let result = dl.evaluate_json(&logic, &data, None).unwrap();
assert_eq!(result.as_str().unwrap(), "Eligible for discount");
```

## Real-World Examples

### 1. Complex Logical Rules (AND/OR)

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();
let result = dl.evaluate_str(
    r#"{
        "and": [
            {">=": [{"var": "age"}, 18]},
            {"<": [{"var": "age"}, 65]},
            {"or": [
                {"==": [{"var": "subscription"}, "premium"]},
                {">=": [{"var": "purchases"}, 5]}
            ]}
        ]
    }"#,
    r#"{"age": 25, "subscription": "basic", "purchases": 7}"#,
    None
).unwrap();

assert!(result.as_bool().unwrap());
```

### 2. Array Operations

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();
let result = dl.evaluate_str(
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

// Returns ["Alice", "Charlie"]
assert_eq!(result.as_array().unwrap().len(), 2);
```

### 3. String Processing

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

// Replace text and split into words
let result = dl.evaluate_str(
    r#"{
        "split": [
            {"replace": [
                {"var": "message"}, 
                "hello", 
                "hi"
            ]}, 
            " "
        ]
    }"#,
    r#"{"message": "hello world hello there"}"#,
    None
).unwrap();

// Returns ["hi", "world", "hi", "there"]
assert_eq!(result.as_array().unwrap().len(), 4);
```

### 4. DateTime Operations

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();
let result = dl.evaluate_str(
    r#"{
        ">": [
            {"+": [
                {"datetime": "2023-07-15T08:30:00Z"},
                {"timestamp": "2d"}
            ]},
            {"datetime": "2023-07-16T08:30:00Z"}
        ]
    }"#,
    r#"{}"#,
    None
).unwrap();

assert!(result.as_bool().unwrap());
```

### 5. Timezone Offset Extraction

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

// Extract timezone offset from datetime with timezone info
let result = dl.evaluate_str(
    r#"{
        "format_date": [
            {"datetime": "2022-07-06T13:20:06+05:00"}, 
            "z"
        ]
    }"#,
    r#"{}"#,
    None
).unwrap();

assert_eq!(result.as_str().unwrap(), "+0500");

// Timezone-aware datetime operations preserve original timezone
let result = dl.evaluate_str(
    r#"{
        "format_date": [
            {"datetime": "2022-07-06T13:20:06+05:00"}, 
            "yyyy-MM-ddTHH:mm:ssXXX"
        ]
    }"#,
    r#"{}"#,
    None
).unwrap();

assert_eq!(result.as_str().unwrap(), "2022-07-06T13:20:06+05:00");
```

### 6. Regex Extraction with Split

```rust
use datalogic_rs::DataLogic;

let dl = DataLogic::new();

// Extract structured data from IBAN using regex named groups
let result = dl.evaluate_str(
    r#"{
        "split": [
            "SBININBB101",
            "^(?P<bank>[A-Z]{4})(?P<country>[A-Z]{2})(?P<location>[A-Z0-9]{2})(?P<branch>[A-Z0-9]{3})?$"
        ]
    }"#,
    r#"{}"#,
    None
).unwrap();

// Returns: {"bank": "SBIN", "country": "IN", "location": "BB", "branch": "101"}
let obj = result.as_object().unwrap();
assert_eq!(obj.get("bank").unwrap().as_str().unwrap(), "SBIN");
assert_eq!(obj.get("country").unwrap().as_str().unwrap(), "IN");
```

### 7. Structured Object Preservation

Create structured output objects with non-operator keys:

```rust
use datalogic_rs::DataLogic;

// Enable structured object preservation
let dl = DataLogic::with_preserve_structure();

// Create structured output with evaluated fields
let result = dl.evaluate_str(
    r#"{
        "result": {"==": [1, 1]},
        "score": {"+": [85, 10, 5]},
        "grade": {"if": [
            {">": [{"var": "score"}, 90]}, 
            "A", 
            "B"
        ]}
    }"#,
    r#"{"score": 95}"#,
    None
).unwrap();

// Returns: {"result": true, "score": 100, "grade": "A"}
let obj = result.as_object().unwrap();
assert_eq!(obj["result"].as_bool().unwrap(), true);
assert_eq!(obj["score"].as_i64().unwrap(), 100);
assert_eq!(obj["grade"].as_str().unwrap(), "A");
```

## Custom Operators

Create domain-specific operators to extend the system:

```rust
use datalogic_rs::{DataLogic, SimpleOperatorFn, DataValue};
use datalogic_rs::value::NumberValue;

// Define a custom operator function - simple approach
fn double<'r>(args: Vec<DataValue<'r>>, data: DataValue<'r>) -> std::result::Result<DataValue<'r>, String> {
    if args.is_empty() {
        // If no arguments, try to use a value from data context
        if let Some(obj) = data.as_object() {
            for (key, val) in obj {
                if *key == "value" && val.is_number() {
                    if let Some(n) = val.as_f64() {
                        return Ok(DataValue::Number(NumberValue::from_f64(n * 2.0)));
                    }
                }
            }
        }
        return Err("double operator requires an argument or 'value' in data".to_string());
    }
    
    if let Some(n) = args[0].as_f64() {
        return Ok(DataValue::Number(NumberValue::from_f64(n * 2.0)));
    }
    
    Err("Argument must be a number".to_string())
}

let mut dl = DataLogic::new();
dl.register_simple_operator("double", double);

// Using with an explicit argument
let result = dl.evaluate_str(
    r#"{"double": 4}"#,
    r#"{}"#,
    None
).unwrap();

assert_eq!(result.as_f64().unwrap(), 8.0);

// Using with data context
let result = dl.evaluate_str(
    r#"{"double": []}"#,
    r#"{"value": 5}"#,
    None
).unwrap();

assert_eq!(result.as_f64().unwrap(), 10.0);
```

Custom operators can be combined with built-in operators for complex logic:

```rust
let complex_rule = r#"{
    "*": [
        2,
        {"double": {"var": "value"}},
        3
    ]
}"#;

// With data: {"value": 3}, evaluates to 2 * (3*2) * 3 = 2 * 6 * 3 = 36
```

For more advanced use cases and complex data types, DataLogic-rs also provides an [advanced custom operator API](CUSTOM_OPERATORS.md).

## Use Cases

`datalogic-rs` excels in scenarios requiring runtime rule evaluation:

### Feature Flagging
Control feature access based on user attributes or context:

```rust
let rule = r#"{
    "and": [
        {"==": [{"var": "user.country"}, "US"]},
        {"or": [
            {"==": [{"var": "user.role"}, "beta_tester"]},
            {">=": [{"var": "user.account_age_days"}, 30]}
        ]}
    ]
}"#;

// Feature is available only to US users who are either beta testers or have accounts older than 30 days
let feature_enabled = dl.evaluate_str(rule, user_data_json, None).unwrap().as_bool().unwrap();
```

### Dynamic Pricing
Apply complex discount rules:

```rust
let pricing_rule = r#"{
    "if": [
        {">=": [{"var": "cart.total"}, 100]},
        {"-": [{"var": "cart.total"}, {"*": [{"var": "cart.total"}, 0.1]}]},
        {"var": "cart.total"}
    ]
}"#;

// 10% discount for orders over $100
let final_price = dl.evaluate_str(pricing_rule, order_data, None).unwrap().as_f64().unwrap();
```

### Fraud Detection
Evaluate transaction risk:

```rust
let fraud_check = r#"{
    "or": [
        {"and": [
            {"!=": [{"var": "transaction.billing_country"}, {"var": "user.country"}]},
            {">=": [{"var": "transaction.amount"}, 1000]}
        ]},
        {"and": [
            {">=": [{"var": "transaction.attempts_last_hour"}, 5]},
            {">": [{"var": "transaction.amount"}, 500]}
        ]}
    ]
}"#;

let is_suspicious = dl.evaluate_str(fraud_check, transaction_data, None).unwrap().as_bool().unwrap();
```

### Authorization Rules
Implement complex access control:

```rust
let access_rule = r#"{
    "or": [
        {"==": [{"var": "user.role"}, "admin"]},
        {"and": [
            {"==": [{"var": "user.role"}, "editor"]},
            {"in": [{"var": "resource.project_id"}, {"var": "user.projects"}]}
        ]}
    ]
}"#;

let has_access = dl.evaluate_str(access_rule, access_context, None).unwrap().as_bool().unwrap();
```

### Form Validation
Check field dependencies dynamically:

```rust
let validation_rule = r#"{
    "if": [
        {"==": [{"var": "shipping_method"}, "international"]},
        {"and": [
            {"!": {"missing": "postal_code"}},
            {"!": {"missing": "country"}}
        ]},
        true
    ]
}"#;

let is_valid = dl.evaluate_str(validation_rule, form_data, None).unwrap().as_bool().unwrap();
```

### Data Extraction and Parsing
Extract structured data using regex patterns:

```rust
let extraction_rule = r#"{
    "split": [
        {"var": "iban"},
        "^(?P<bank>[A-Z]{4})(?P<country>[A-Z]{2})(?P<location>[A-Z0-9]{2})(?P<branch>[A-Z0-9]{3})?$"
    ]
}"#;

let data = r#"{"iban": "SBININBB101"}"#;
let parsed_iban = dl.evaluate_str(extraction_rule, data, None).unwrap();
// Returns: {"bank": "SBIN", "country": "IN", "location": "BB", "branch": "101"}

// Use extracted data for further validation
let validation_rule = r#"{
    "and": [
        {"==": [{"var": "result.country"}, "IN"]},
        {"in": [{"var": "result.bank"}, ["SBIN", "ICIC", "HDFC"]]}
    ]
}"#;

let validation_data = format!(r#"{{"result": {}}}"#, parsed_iban);
let is_valid_bank = dl.evaluate_str(validation_rule, &validation_data, None).unwrap().as_bool().unwrap();
```

## Supported Operations

| Category | Operators |
|----------|-----------|
| **Comparison** | `==` (equal), `===` (strict equal), `!=` (not equal), `!==` (strict not equal), `>` (greater than), `>=` (greater than or equal), `<` (less than), `<=` (less than or equal) |
| **Logic** | `and`, `or`, `!` (not), `!!` (double negation) |
| **Arithmetic** | `+` (addition), `-` (subtraction), `*` (multiplication), `/` (division), `%` (modulo), `min`, `max`, `abs` (absolute value), `ceil` (round up), `floor` (round down) |
| **Control Flow** | `if` (conditional), `?:` (ternary), `??` (nullish coalescing) |
| **Arrays** | `map`, `filter`, `reduce`, `all`, `some`, `none`, `merge`, `in` (contains), `length`, `slice`, `sort` |
| **Strings** | `cat` (concatenate), `substr`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `replace`, `split` (with regex extraction) |
| **Data Access** | `var` (variable access), `val` (value access), `exists`, `missing`, `missing_some` |
| **DateTime** | `datetime`, `timestamp`, `now`, `parse_date`, `format_date` (with timezone offset support), `date_diff` |
| **Error Handling** | `throw`, `try` |
| **Custom** | Support for user-defined operators |

## Performance

**Benchmark results show** `datalogic-rs` is **30% faster** than the next fastest JSONLogic implementations, thanks to:
- Arena-based memory management
- Static operator dispatch
- Zero-copy deserialization
- Optimized rule compilation

### Benchmark Metrics (Apple M2 Pro)

| Implementation | Execution Time | Relative Performance |
|----------------|---------------|---------------------|
| **datalogic-rs** | **380ms** | **1.0x (baseline)** |
| json-logic-engine (pre-compiled) | 417ms | 1.1x slower |
| json-logic-engine (interpreted) | 986.064ms | 2.6x slower |
| json-logic-js | 5,755ms | 15.1x slower |

These benchmarks represent execution time for the same standard suite of JSONLogic tests, demonstrating datalogic-rs's superior performance profile across common expression patterns.

## Contributing

We welcome contributions! See the [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

## License

Licensed under Apache License, Version 2.0

---

### Next Steps
✅ Try out `datalogic-rs` today!  
📖 Check out the [API documentation](./API.md) for detailed usage instructions  
📚 See the [docs.rs documentation](https://docs.rs/datalogic-rs) for comprehensive reference  
📝 Learn how to implement [custom operators](./CUSTOM_OPERATORS.md) to extend the engine  
⭐ Star the [GitHub repository](https://github.com/codetiger/datalogic-rs) if you find it useful!