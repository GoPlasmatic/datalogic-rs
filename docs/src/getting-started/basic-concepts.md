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

datalogic-rs separates rule processing into two phases.

### Compilation Phase

When you call `engine.compile(rule_str)`, the library:

1. **Parses** the JSON rule into an internal representation
2. **Assigns OpCodes** to operators for fast dispatch
3. **Pre-evaluates** constant sub-expressions
4. Produces a reusable `Logic` (no `Arc` wrap by default — wrap explicitly when sharing across threads)

```rust
let compiled = engine.compile(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

// Wrap when you want to share across threads:
let shared = std::sync::Arc::new(compiled);
```

### Evaluation Phase

When you evaluate, the engine:

1. **Dispatches** operations via `OpCode` (O(1)) for built-ins
2. **Walks** the context stack for variable lookups
3. **Returns** an arena-resident `&DataValue<'a>` (or an owned `String` /
   `OwnedDataValue` / `serde_json::Value` depending on the entry point)

There are three evaluation entry points, picked by what the caller has on
hand and how much arena lifetime they want to manage:

| Entry point | When to use | Returns |
|-------------|-------------|---------|
| `Engine::evaluate_str(logic, data)` | One-shot. Inputs and output are JSON strings. | `String` |
| `Engine::session().evaluate*` | Repeated calls — the session owns a reusable arena and resets it between calls. | `String` / `OwnedDataValue` / `serde_json::Value` |
| `Engine::evaluate(logic, data, &arena)` | Hot path. You own the `bumpalo::Bump` and want zero-copy `&DataValue<'a>` results. | `&DataValue<'a>` |

```rust
use bumpalo::Bump;
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

// Reusable session — arena resets between calls.
let mut session = engine.session();
let _ = session.evaluate_str(&compiled, r#"{"x": 42}"#).unwrap();

// Or manage the arena yourself for zero-copy results.
let arena = Bump::new();
let r = engine.evaluate(&compiled, r#"{"x": 42}"#, &arena).unwrap();
assert_eq!(r.as_bool(), Some(true));
```

## The Engine

The `Engine` struct is your main entry point. It is built via `Engine::new`
or the `EngineBuilder`:

```rust
use datalogic_rs::{Engine, EvaluationConfig};

// Default engine
let engine = Engine::new();

// Engine with custom configuration
let engine = Engine::builder()
    .config(EvaluationConfig::strict())
    .build();

// Engine with structure preservation (templating mode) — needs feature = ["preserve"]
# #[cfg(feature = "preserve")]
let engine = Engine::builder().preserve_structure(true).build();

// Engine with custom operators
# struct MyOp;
# impl datalogic_rs::CustomOperator for MyOp {
#     fn evaluate<'a>(
#         &self,
#         _args: &[&'a datalogic_rs::DataValue<'a>],
#         _ctx: &mut datalogic_rs::operator::ContextStack<'a>,
#         arena: &'a bumpalo::Bump,
#     ) -> datalogic_rs::Result<&'a datalogic_rs::DataValue<'a>> {
#         Ok(arena.alloc(datalogic_rs::DataValue::Null))
#     }
# }
let engine = Engine::builder()
    .add_operator("my_op", MyOp)
    .build();
```

The engine:
- Owns the registered custom operators (frozen at `build()`)
- Holds the evaluation configuration
- Provides compile and evaluate methods

> **Note:** v5 makes operator registration **builder-only**. You can no
> longer mutate an `Engine` to add operators after construction.

## Context Stack

The context stack manages variable scope during evaluation. This is
important for array operations like `map`, `filter`, and `reduce`.

```rust
// In a filter operation, "" refers to the current element
let r = engine.evaluate_str(
    r#"{"filter": [[1, 2, 3, 4, 5], {">": [{"var": ""}, 3]}]}"#,
    r#"{}"#,
).unwrap();
// Result: "[4,5]"
```

During array operations:
- `""` (or `var` with empty string) refers to the current element
- The outer data context is still accessible
- Nested operations push and pop frames automatically

## Type Coercion

JSONLogic operators often perform type coercion:

### Arithmetic
- Strings are parsed as numbers when possible (`"5" + 3 = 8`)
- Non-numeric strings raise a `Thrown { type: "NaN" }` error by default;
  configurable via [`EvaluationConfig::arithmetic_nan_handling`](../advanced/configuration.md)

### Comparison
- `==` performs loose equality (with type coercion)
- `===` performs strict equality (no coercion)

### Truthiness
By default, uses JavaScript-style truthiness:
- Falsy: `false`, `0`, `""`, `null`, `[]`
- Truthy: everything else

This is configurable via `EvaluationConfig`.

## Thread Safety

`Logic` is `Send + Sync` and can be shared across threads via `Arc`:

```rust
use datalogic_rs::Engine;
use std::sync::Arc;
use std::thread;

let engine = Arc::new(Engine::new());
let compiled = Arc::new(engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap());

let handles: Vec<_> = (0..4).map(|i| {
    let engine = Arc::clone(&engine);
    let compiled = Arc::clone(&compiled);
    thread::spawn(move || {
        let mut session = engine.session();
        session.evaluate_str(&compiled, &format!(r#"{{"x": {}}}"#, i)).unwrap()
    })
}).collect();

for h in handles {
    println!("{}", h.join().unwrap());
}
```

## Next Steps

- [Operators Overview](../operators/overview.md) - Learn about all available operators
- [Configuration](../advanced/configuration.md) - Customize evaluation behavior
- [Custom Operators](../advanced/custom-operators.md) - Extend with your own logic
- [Migration Guide](../migration.md) - Move from v4 to v5
