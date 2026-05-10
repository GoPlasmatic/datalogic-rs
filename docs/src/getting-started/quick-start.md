# Quick Start

This guide will get you evaluating JSONLogic rules in minutes.

## Basic Workflow

The typical workflow with datalogic-rs is:

1. Create an `Engine` instance
2. Compile your rule (once) into a `Logic`
3. Evaluate against data (many times)

```rust
use datalogic_rs::Engine;

// 1. Create an engine
let engine = Engine::new();

// 2. Compile a rule (string in, Logic out)
let compiled = engine.compile(r#"{">": [{"var": "score"}, 50]}"#).unwrap();

// 3. Evaluate against data via a Session — owned String result
let mut session = engine.session();
let result = session.evaluate_str(&compiled, r#"{"score": 75}"#).unwrap();
assert_eq!(result, "true");
```

## One-Shot Evaluation

For a single evaluation, skip the explicit compile step:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let result = engine
    .evaluate_str(r#"{"+": [1, 2, 3]}"#, r#"{}"#)
    .unwrap();
assert_eq!(result, "6");
```

`evaluate_str` parses the rule + data, evaluates once, and returns the
result as a JSON `String`.

## Power-User: Compile Once, Evaluate Many (Zero-Copy Results)

When you want zero-copy `&DataValue<'a>` results and are willing to manage
the arena yourself, call `Engine::evaluate` directly:

```rust
use bumpalo::Bump;
use datalogic_rs::{DataValue, Engine};

let engine = Engine::new();
let compiled = engine.compile(r#"{"==": [{"var": "status"}, "active"]}"#).unwrap();

let arena = Bump::new();
let data = DataValue::from_str(r#"{"status": "active"}"#, &arena).unwrap();
let result = engine.evaluate(&compiled, data, &arena).unwrap();
assert_eq!(result.as_bool(), Some(true));
```

`Engine::evaluate` accepts any input shape via [`EvalInput`](../api/reference.md): `&str`,
`&DataValue<'a>`, `DataValue<'a>`, `&OwnedDataValue`, or
`&serde_json::Value` (under the `compat` feature).

## Working with Variables

Access data using the `var` operator:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();

// Simple variable access
let r = engine.evaluate_str(r#"{"var": "name"}"#, r#"{"name": "Alice"}"#).unwrap();
assert_eq!(r, "\"Alice\"");

// Nested variable access with dot notation
let r = engine.evaluate_str(
    r#"{"var": "user.address.city"}"#,
    r#"{"user": {"address": {"city": "New York"}}}"#,
).unwrap();
assert_eq!(r, "\"New York\"");

// Default values
let r = engine.evaluate_str(
    r#"{"var": ["missing_key", "default_value"]}"#,
    r#"{}"#,
).unwrap();
assert_eq!(r, "\"default_value\"");
```

## Conditional Logic

Use `if` for branching:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let rule = r#"{"if": [{">=": [{"var": "age"}, 18]}, "adult", "minor"]}"#;

let r = engine.evaluate_str(rule, r#"{"age": 25}"#).unwrap();
assert_eq!(r, "\"adult\"");

let r = engine.evaluate_str(rule, r#"{"age": 15}"#).unwrap();
assert_eq!(r, "\"minor\"");
```

## Combining Conditions

Use `and` and `or` to combine conditions:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();

// AND: all conditions must be true
let rule = r#"{"and": [
    {">=": [{"var": "age"}, 18]},
    {"==": [{"var": "verified"}, true]}
]}"#;
let r = engine.evaluate_str(rule, r#"{"age": 21, "verified": true}"#).unwrap();
assert_eq!(r, "true");

// OR: at least one condition must be true
let rule = r#"{"or": [
    {"==": [{"var": "role"}, "admin"]},
    {"==": [{"var": "role"}, "moderator"]}
]}"#;
let r = engine.evaluate_str(rule, r#"{"role": "admin"}"#).unwrap();
assert_eq!(r, "true");
```

## Array Operations

Filter, map, and reduce arrays:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();

// Filter: keep elements matching a condition
let r = engine.evaluate_str(
    r#"{"filter": [{"var": "numbers"}, {">": [{"var": ""}, 5]}]}"#,
    r#"{"numbers": [1, 3, 5, 7, 9]}"#,
).unwrap();
assert_eq!(r, "[7,9]");

// Map: transform each element
let r = engine.evaluate_str(
    r#"{"map": [{"var": "numbers"}, {"*": [{"var": ""}, 2]}]}"#,
    r#"{"numbers": [1, 2, 3]}"#,
).unwrap();
assert_eq!(r, "[2,4,6]");
```

## Error Handling

`Engine::evaluate*` returns `Result<_, datalogic_rs::Error>`. The error
carries a stable `kind`, the offending operator, and a path breadcrumb so
callers can surface where the failure occurred:

```rust
use datalogic_rs::{Engine, ErrorKind};

let engine = Engine::new();
match engine.evaluate_str(r#"{"+": ["text", 1]}"#, r#"{}"#) {
    Ok(value) => println!("ok: {}", value),
    Err(err) => {
        println!("kind: {}", err.tag());
        if let ErrorKind::Thrown(payload) = &err.kind {
            println!("thrown payload: {:?}", payload);
        }
    }
}
```

For runtime errors that should be caught inside the rule, enable the
`error-handling` feature and use the `try` operator:

```rust
// Cargo.toml: features = ["error-handling"]
let engine = Engine::new();
let r = engine.evaluate_str(
    r#"{"try": [{"/": [10, {"var": "divisor"}]}, 0]}"#,
    r#"{"divisor": 0}"#,
).unwrap();
// `0` is returned when the divide raises.
```

## Next Steps

- [Basic Concepts](basic-concepts.md) - Understand the architecture
- [Operators](../operators/overview.md) - Explore all available operators
- [Custom Operators](../advanced/custom-operators.md) - Extend with your own logic
- [Migration Guide](../migration.md) - Move from v4 to v5
