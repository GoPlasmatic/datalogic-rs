# Quick Start

This guide will get you evaluating JSONLogic rules in minutes.

## The simplest path: one-shot helpers

For one-off evaluations with no custom operators or custom configurations, you can evaluate rules directly without manually initializing an engine. 

<div class="codetabs">

```rust
let result = datalogic_rs::eval_str(
    r#"{">": [{"var": "score"}, 50]}"#,
    r#"{"score": 75}"#,
).unwrap();
assert_eq!(result, "true");
```

```javascript
import init, { evaluate } from '@goplasmatic/datalogic-wasm';
await init();

const result = evaluate(
  '{">": [{"var": "score"}, 50]}',
  '{"score": 75}',
  false
);
console.log(result); // "true"
```

```python
from datalogic_py import apply

result = apply(
    {">": [{"var": "score"}, 50]},
    {"score": 75}
)
print(result) # True
```

```go
result, _ := datalogic.Apply(
    `{">": [{"var": "score"}, 50]}`,
    `{"score": 75}`,
)
fmt.Println(result) // "true"
```

</div>

These functions delegate to a lazily-constructed default engine under the hood. They are the right starting point for tutorials, scripts, and code that doesn't need custom operators or non-default configurations.

## When you need an Engine

Construct an `Engine` when you need any of: custom operators, custom configurations, templating mode, or a long-lived `Session` to recycle memory in hot loops.

<div class="codetabs">

```rust
use datalogic_rs::Engine;

// 1. Create an engine
let engine = Engine::new();

// 2. Compile a rule once (returns reusable compiled Logic)
let compiled = engine.compile(r#"{">": [{"var": "score"}, 50]}"#).unwrap();

// 3. Evaluate against data via a Session (reuses memory buffer)
let mut session = engine.session();
let result = session.eval_str(&compiled, r#"{"score": 75}"#).unwrap();
assert_eq!(result, "true");
session.reset(); // Reset between evaluations to prevent memory growth
```

```javascript
import init, { CompiledRule } from '@goplasmatic/datalogic-wasm';
await init();

// 1. Compile once
const rule = new CompiledRule('{">": [{"var": "score"}, 50]}', false);

// 2. Evaluate many times
const result = rule.evaluate('{"score": 75}');
console.log(result); // "true"
```

```python
from datalogic_py import Engine

# 1. Create an engine
engine = Engine()

# 2. Compile once
rule = engine.compile({">": [{"var": "score"}, 50]})

# 3. Evaluate
result = rule.evaluate({"score": 75})
print(result) # True
```

```go
// 1. Create engine (defer close to prevent FFI leak)
engine := datalogic.NewEngine()
defer engine.Close()

// 2. Compile once (defer close to prevent FFI leak)
rule, _ := engine.Compile(`{">": [{"var": "score"}, 50]}`)
defer rule.Close()

// 3. Open session for evaluation (defer close to prevent FFI leak)
session := engine.Session()
defer session.Close()

result, _ := session.Evaluate(rule, `{"score": 75}`)
fmt.Println(result) // "true"
```

</div>

Sessions reuse the same `bumpalo::Bump` across calls. They never
auto-reset — `session.reset()` between batches keeps peak memory bounded
by the largest single evaluation rather than the cumulative loop.

## One-shot via Engine

If you've already built an `Engine` (e.g. to register custom operators),
its one-shot methods mirror the module-level helpers:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let result = engine
    .eval_str(r#"{"+": [1, 2, 3]}"#, r#"{}"#)
    .unwrap();
assert_eq!(result, "6");
```

`eval_str` parses the rule + data, evaluates once, and returns the
result as a JSON `String`. `eval` returns an `OwnedDataValue`;
`eval_into::<T>` returns a typed `T: DeserializeOwned` (requires
`feature = "serde_json"`).

## Power-user: zero-copy borrowed results

When you want zero-copy `&DataValue<'a>` results and are willing to
manage the arena yourself, call [`Engine::evaluate`](../api/reference.md#evaluate-raw-tier)
directly:

```rust
use bumpalo::Bump;
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"==": [{"var": "status"}, "active"]}"#).unwrap();

let arena = Bump::new();
let result = engine.evaluate(&compiled, r#"{"status": "active"}"#, &arena).unwrap();
assert_eq!(result.as_bool(), Some(true));
```

`Engine::evaluate` accepts any input shape via [`EvalInput`](../api/reference.md#evalinput):
`&str`, `&DataValue<'a>`, `DataValue<'a>`, `&OwnedDataValue`, or
`&serde_json::Value` (under `feature = "serde_json"`).

## Working with Variables

Access data using the `var` operator:

```rust
// Simple variable access
let r = datalogic_rs::eval_str(r#"{"var": "name"}"#, r#"{"name": "Alice"}"#).unwrap();
assert_eq!(r, "\"Alice\"");

// Nested variable access with dot notation
let r = datalogic_rs::eval_str(
    r#"{"var": "user.address.city"}"#,
    r#"{"user": {"address": {"city": "New York"}}}"#,
).unwrap();
assert_eq!(r, "\"New York\"");

// Default values
let r = datalogic_rs::eval_str(
    r#"{"var": ["missing_key", "default_value"]}"#,
    r#"{}"#,
).unwrap();
assert_eq!(r, "\"default_value\"");
```

## Conditional Logic

Use `if` for branching:

```rust
let rule = r#"{"if": [{">=": [{"var": "age"}, 18]}, "adult", "minor"]}"#;

let r = datalogic_rs::eval_str(rule, r#"{"age": 25}"#).unwrap();
assert_eq!(r, "\"adult\"");

let r = datalogic_rs::eval_str(rule, r#"{"age": 15}"#).unwrap();
assert_eq!(r, "\"minor\"");
```

## Combining Conditions

Use `and` and `or` to combine conditions:

```rust
// AND: all conditions must be true
let rule = r#"{"and": [
    {">=": [{"var": "age"}, 18]},
    {"==": [{"var": "verified"}, true]}
]}"#;
let r = datalogic_rs::eval_str(rule, r#"{"age": 21, "verified": true}"#).unwrap();
assert_eq!(r, "true");

// OR: at least one condition must be true
let rule = r#"{"or": [
    {"==": [{"var": "role"}, "admin"]},
    {"==": [{"var": "role"}, "moderator"]}
]}"#;
let r = datalogic_rs::eval_str(rule, r#"{"role": "admin"}"#).unwrap();
assert_eq!(r, "true");
```

## Array Operations

Filter, map, and reduce arrays:

```rust
// Filter: keep elements matching a condition
let r = datalogic_rs::eval_str(
    r#"{"filter": [{"var": "numbers"}, {">": [{"var": ""}, 5]}]}"#,
    r#"{"numbers": [1, 3, 5, 7, 9]}"#,
).unwrap();
assert_eq!(r, "[7,9]");

// Map: transform each element
let r = datalogic_rs::eval_str(
    r#"{"map": [{"var": "numbers"}, {"*": [{"var": ""}, 2]}]}"#,
    r#"{"numbers": [1, 2, 3]}"#,
).unwrap();
assert_eq!(r, "[2,4,6]");
```

## Error Handling

The `eval*` methods return `Result<_, datalogic_rs::Error>`. The error
carries a stable `kind`, the offending operator, and a path breadcrumb so
callers can surface where the failure occurred:

```rust
use datalogic_rs::ErrorKind;

match datalogic_rs::eval_str(r#"{"+": ["text", 1]}"#, r#"{}"#) {
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
let r = datalogic_rs::eval_str(
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
