# Basic Concepts

Understanding how datalogic-rs works will help you use it effectively.

## JSONLogic Format

A JSONLogic rule is a JSON object where:
- The **key** is the operator name
- The **value** is an array of arguments (or a single argument)

```json
{ "operator": [arg1, arg2, ...] }
```

Arguments can be:
- Literal values: `1`, `"hello"`, `true`, `null`
- Arrays: `[1, 2, 3]`
- Nested operations: `{ "var": "x" }`

### Examples

```json
// Simple comparison
{ ">": [5, 3] }  // true

// Variable access
{ "var": "user.name" }  // Access user.name from data

// Nested operations
{ "+": [{ "var": "a" }, { "var": "b" }] }  // Add two variables

// Multiple arguments
{ "and": [true, true, false] }  // false
```

## Compilation vs Evaluation

datalogic-rs separates rule processing into two phases:

### Compilation Phase

When you call `engine.compile(&rule)`, the library:

1. **Parses** the JSON into an internal representation
2. **Assigns OpCodes** to operators for fast dispatch
3. **Pre-evaluates** constant expressions
4. **Wraps** the result in `Arc` for thread-safe sharing

```rust
// Compile once
let compiled = engine.compile(&rule).unwrap();

// The compiled rule can be shared across threads
let compiled_clone = Arc::clone(&compiled);
```

### Evaluation Phase

When you call `engine.evaluate_owned(&compiled, data)`:

1. **Dispatches** operations via OpCode (O(1) lookup)
2. **Accesses** data through the context stack
3. **Returns** the result as a `Value`

```rust
// Evaluate many times with different data
let result1 = engine.evaluate_owned(&compiled, json!({ "x": 1 })).unwrap();
let result2 = engine.evaluate_owned(&compiled, json!({ "x": 2 })).unwrap();
```

## The DataLogic Engine

The `DataLogic` struct is your main entry point:

```rust
use datalogic_rs::DataLogic;

// Default engine
let engine = DataLogic::new();

// Engine with configuration
let engine = DataLogic::with_config(config);

// Engine with structure preservation (templating mode)
let engine = DataLogic::with_preserve_structure();

// Engine with both
let engine = DataLogic::with_config_and_structure(config, true);
```

The engine:
- Stores custom operators
- Holds evaluation configuration
- Provides compile and evaluate methods

## Context Stack

The context stack manages variable scope during evaluation. This is important for array operations like `map`, `filter`, and `reduce`.

```rust
// In a filter operation, "" refers to the current element
let rule = json!({
    "filter": [
        [1, 2, 3, 4, 5],
        { ">": [{ "var": "" }, 3] }  // "" = current element
    ]
});
// Result: [4, 5]
```

During array operations:
- `""` or `var` with empty string refers to the current element
- The outer data context is still accessible
- Nested operations create nested contexts

## Evaluation Methods

### `evaluate_owned`

Takes ownership of the data, best for most use cases:

```rust
let result = engine.evaluate_owned(&compiled, json!({ "x": 1 })).unwrap();
```

### `evaluate`

Borrows the data, useful when you need to reuse the data:

```rust
let data = json!({ "x": 1 });
let result = engine.evaluate(&compiled, &data).unwrap();
// data is still available here
```

### `evaluate_json`

Convenience method that parses JSON strings:

```rust
let result = engine.evaluate_json(
    r#"{ "+": [1, 2] }"#,  // Rule as JSON string
    r#"{"x": 10}"#         // Data as JSON string
).unwrap();
```

## Type Coercion

JSONLogic operators often perform type coercion:

### Arithmetic
- Strings are parsed as numbers when possible
- `"5" + 3` = `8`
- Non-numeric strings may result in errors or NaN (configurable)

### Comparison
- `==` performs loose equality (type coercion)
- `===` performs strict equality (no coercion)

### Truthiness
By default, uses JavaScript-style truthiness:
- Falsy: `false`, `0`, `""`, `null`, `[]`
- Truthy: everything else

This is configurable via `EvaluationConfig`.

## Thread Safety

`CompiledLogic` is wrapped in `Arc` and is `Send + Sync`:

```rust
use std::sync::Arc;
use std::thread;

let engine = Arc::new(DataLogic::new());
let compiled = engine.compile(&rule).unwrap();

let handles: Vec<_> = (0..4).map(|i| {
    let engine = Arc::clone(&engine);
    let compiled = Arc::clone(&compiled);

    thread::spawn(move || {
        engine.evaluate_owned(&compiled, json!({ "x": i }))
    })
}).collect();

for handle in handles {
    let result = handle.join().unwrap();
    // Each thread gets its result
}
```

## Next Steps

- [Operators Overview](../operators/overview.md) - Learn about all available operators
- [Configuration](../advanced/configuration.md) - Customize evaluation behavior
- [Custom Operators](../advanced/custom-operators.md) - Extend with your own logic
