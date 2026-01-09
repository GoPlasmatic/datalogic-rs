# Quick Start

This guide will get you evaluating JSONLogic rules in minutes.

## Basic Workflow

The typical workflow with datalogic-rs is:

1. Create an engine instance
2. Compile your rule (once)
3. Evaluate against data (many times)

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

// 1. Create engine
let engine = DataLogic::new();

// 2. Compile rule
let rule = json!({ ">": [{ "var": "score" }, 50] });
let compiled = engine.compile(&rule).unwrap();

// 3. Evaluate against data
let result = engine.evaluate_owned(&compiled, json!({ "score": 75 })).unwrap();
assert_eq!(result, json!(true));
```

## Quick JSON Evaluation

For one-off evaluations, use `evaluate_json`:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

let result = engine.evaluate_json(
    r#"{ "+": [1, 2, 3] }"#,
    r#"{}"#
).unwrap();

assert_eq!(result, json!(6));
```

## Working with Variables

Access data using the `var` operator:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// Simple variable access
let rule = json!({ "var": "name" });
let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({ "name": "Alice" })).unwrap();
assert_eq!(result, json!("Alice"));

// Nested variable access with dot notation
let rule = json!({ "var": "user.address.city" });
let compiled = engine.compile(&rule).unwrap();
let data = json!({
    "user": {
        "address": {
            "city": "New York"
        }
    }
});
let result = engine.evaluate_owned(&compiled, data).unwrap();
assert_eq!(result, json!("New York"));

// Default values
let rule = json!({ "var": ["missing_key", "default_value"] });
let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
assert_eq!(result, json!("default_value"));
```

## Conditional Logic

Use `if` for branching:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

let rule = json!({
    "if": [
        { ">=": [{ "var": "age" }, 18] },
        "adult",
        "minor"
    ]
});

let compiled = engine.compile(&rule).unwrap();

let result = engine.evaluate_owned(&compiled, json!({ "age": 25 })).unwrap();
assert_eq!(result, json!("adult"));

let result = engine.evaluate_owned(&compiled, json!({ "age": 15 })).unwrap();
assert_eq!(result, json!("minor"));
```

## Combining Conditions

Use `and` and `or` to combine conditions:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// AND: all conditions must be true
let rule = json!({
    "and": [
        { ">=": [{ "var": "age" }, 18] },
        { "==": [{ "var": "verified" }, true] }
    ]
});

let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({
    "age": 21,
    "verified": true
})).unwrap();
assert_eq!(result, json!(true));

// OR: at least one condition must be true
let rule = json!({
    "or": [
        { "==": [{ "var": "role" }, "admin"] },
        { "==": [{ "var": "role" }, "moderator"] }
    ]
});

let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({ "role": "admin" })).unwrap();
assert_eq!(result, json!(true));
```

## Array Operations

Filter, map, and reduce arrays:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// Filter: keep elements matching a condition
let rule = json!({
    "filter": [
        { "var": "numbers" },
        { ">": [{ "var": "" }, 5] }
    ]
});

let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({
    "numbers": [1, 3, 5, 7, 9]
})).unwrap();
assert_eq!(result, json!([7, 9]));

// Map: transform each element
let rule = json!({
    "map": [
        { "var": "numbers" },
        { "*": [{ "var": "" }, 2] }
    ]
});

let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({
    "numbers": [1, 2, 3]
})).unwrap();
assert_eq!(result, json!([2, 4, 6]));
```

## Error Handling

Handle errors gracefully:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// Compilation errors
let result = engine.compile(&json!({ "unknown_op": [] }));
// Result depends on preserve_structure setting

// Evaluation errors with try/catch
let rule = json!({
    "try": [
        { "/": [10, { "var": "divisor" }] },
        0  // Fallback value
    ]
});

let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({ "divisor": 0 })).unwrap();
assert_eq!(result, json!(0));  // Division by zero caught
```

## Next Steps

- [Basic Concepts](basic-concepts.md) - Understand the architecture
- [Operators](../operators/overview.md) - Explore all available operators
- [Custom Operators](../advanced/custom-operators.md) - Extend with your own logic
