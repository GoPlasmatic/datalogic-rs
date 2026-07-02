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

`datalogic` separates rule processing into two distinct phases for maximum execution speed.

### Compilation Phase

When you compile a rule, the engine parses the JSON rule, resolves string operator names to integer OpCodes, performs strength reduction and constant folding, and produces a reusable, immutable compiled logic AST:

<div class="codetabs">

```rust
// Compiles to a reusable Logic AST
let compiled = engine.compile(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

// Logic is Send + Sync; wrap in Arc for cross-thread sharing
let shared = std::sync::Arc::new(compiled);
```

```javascript
// Compiles to a reusable Rule (the engine-less CompiledRule class is separate)
const rule = engine.compile('{">": [{"var": "x"}, 10]}');
```

```python
# Compiles to a reusable Rule object
rule = engine.compile({">": [{"var": "x"}, 10]})
```

```go
// Compiles to a reusable *Rule
rule, _ := engine.Compile(`{">": [{"var": "x"}, 10]}`)
defer rule.Close()
```

</div>

### Evaluation Phase

During evaluation, the engine dispatches operations via OpCodes and walks the data context. The actual evaluation buffers are allocated within a transient or session-scoped memory arena.

Here is how you evaluate a compiled rule against data using a reusable session:

<div class="codetabs">

```rust
let engine = Engine::new();
let compiled = engine.compile(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

// Reusable session — reuses the memory buffer across calls.
let mut session = engine.session();
let result = session.eval_str(&compiled, r#"{"x": 42}"#).unwrap();
assert_eq!(result, "true");
session.reset(); // Reset between batches
```

```javascript
import init, { CompiledRule } from '@goplasmatic/datalogic-wasm';
await init();

const rule = new CompiledRule('{">": [{"var": "x"}, 10]}', false);
const result = rule.evaluate('{"x": 42}');
console.log(result); // "true"
```

```python
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({">": [{"var": "x"}, 10]})

# Direct evaluation against python dictionaries
result = rule.evaluate({"x": 42})
print(result) # True
```

```go
engine := datalogic.NewEngine()
defer engine.Close()

rule, _ := engine.Compile(`{">": [{"var": "x"}, 10]}`)
defer rule.Close()

session := engine.Session()
defer session.Close()

result, _ := session.Evaluate(rule, `{"x": 42}`)
fmt.Println(result) // "true"
```

</div>

## The Engine

The `Engine` is the central component that holds custom configurations and registered operators. Once constructed, the engine is frozen and immutable.

Here is how to construct and configure an engine across runtimes:

<div class="codetabs">

```rust
use datalogic_rs::{Engine, EvaluationConfig};

// 1. Default engine
let engine = Engine::new();

// 2. Engine with custom configurations
let engine = Engine::builder()
    .with_config(EvaluationConfig::strict())
    .build();

// 3. Engine with custom operators
let engine = Engine::builder()
    .add_operator("double", DoubleOperator)
    .build();
```

```javascript
import { Engine } from '@goplasmatic/datalogic-node';

// 1. Default engine
const engine = new Engine();

// 2. Engine with custom operators
const engineWithOps = new Engine({}, {
  double: (argsJson) => {
    const args = JSON.parse(argsJson);
    return JSON.stringify(args[0] * 2);
  }
});
```

```python
from datalogic_py import Engine

# 1. Default engine
engine = Engine()

# 2. Configured engine with custom operators
engine_with_ops = Engine(
    templating=True, # Enable JSON templating mode
    custom_operators={
        "double": lambda args_json: json.dumps(json.loads(args_json)[0] * 2)
    }
)
```

```go
import datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"

// 1. Default engine
engine := datalogic.NewEngine()
defer engine.Close()

// 2. Engine with custom operators via a fluent builder
engineWithOps := datalogic.NewEngineBuilder().
    AddOperator("double", func(argsJson string) (string, error) {
        // implementation
        return "result", nil
    }).
    Build()
defer engineWithOps.Close()
```

</div>

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
let r = datalogic_rs::eval_str(
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
let compiled = engine.compile_arc(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();

let handles: Vec<_> = (0..4).map(|i| {
    let engine = Arc::clone(&engine);
    let compiled = Arc::clone(&compiled);
    thread::spawn(move || {
        let mut session = engine.session();
        session.eval_str(&compiled, &format!(r#"{{"x": {}}}"#, i)).unwrap()
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
