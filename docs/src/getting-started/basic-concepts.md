# Basic Concepts

This page covers the model behind datalogic-rs: rules, compilation, evaluation and the engine.

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

datalogic-rs works in two phases: you compile a rule once, then evaluate it against as many data payloads as you need.

### Compilation Phase

When you compile a rule, the engine parses the JSON, resolves operator names to OpCodes (and custom operators to their slot on the engine), runs constant folding, strength reduction, dead-branch removal and common-subexpression elimination, and returns an immutable compiled tree you can reuse:

<div class="codetabs">

```rust
// Compiles to a reusable Logic AST
let compiled = engine.compile(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

// Logic is Send + Sync; wrap in Arc for cross-thread sharing
let shared = std::sync::Arc::new(compiled);
```

```javascript
// Compiles to a reusable Rule handle
const rule = engine.compile({ '>': [{ var: 'x' }, 10] });
// browser/edge: same API via @goplasmatic/datalogic-wasm, see the WASM chapter
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

```java
// Compiles to a reusable Rule (AutoCloseable; thread-safe, share freely)
Rule rule = engine.compile("{\">\": [{\"var\": \"x\"}, 10]}");
```

```csharp
// Compiles to a reusable Rule (IDisposable; thread-safe, share freely)
using var rule = engine.Compile("""{">": [{"var": "x"}, 10]}""");
```

```php
// Compiles to a reusable Rule object
$rule = $engine->compile('{">": [{"var": "x"}, 10]}');
```

</div>

`compile` accepts a rule with an unknown operator and leaves it to fail when evaluation reaches it. `check` reports that and other mistakes before anything runs, and `compile_checked` (`compileChecked` in the bindings) refuses such a rule; see [Rule Analysis](../advanced/rule-analysis.md).

### Evaluation Phase

During evaluation, the engine dispatches each node by its OpCode and reads from the data context. Intermediate values live in a memory arena: a fresh one per one-shot call, or the one a session owns.

To evaluate a compiled rule against data with a reusable session:

<div class="codetabs">

```rust
let engine = Engine::new();
let compiled = engine.compile(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

// Reusable session: reuses the memory buffer across calls.
let mut session = engine.session();
let result = session.eval_str(&compiled, r#"{"x": 42}"#).unwrap();
assert_eq!(result, "true");
session.reset(); // Reset between batches
```

```javascript
import { Engine } from '@goplasmatic/datalogic-node';

const engine = new Engine();
const rule = engine.compile({ '>': [{ var: 'x' }, 10] });

// Session reuses one arena across calls
const sess = engine.session();
const result = sess.evaluate(rule, { x: 42 });
console.log(result); // true
// browser/edge: same API via @goplasmatic/datalogic-wasm, see the WASM chapter
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

```java
// try-with-resources frees the native handles
try (Engine engine = new Engine();
     Rule rule = engine.compile("{\">\": [{\"var\": \"x\"}, 10]}");
     Session session = engine.openSession()) {
    String result = session.evaluate(rule, "{\"x\": 42}");
    System.out.println(result); // "true"
}
```

```csharp
using var engine = new Engine();
using var rule = engine.Compile("""{">": [{"var": "x"}, 10]}""");

// Session reuses one arena across calls
using var session = engine.OpenSession();
var result = session.Evaluate(rule, """{"x": 42}""");
Console.WriteLine(result); // "true"
```

```php
$engine = new Engine();
$rule = $engine->compile('{">": [{"var": "x"}, 10]}');

// Session reuses one arena across calls
$session = $engine->openSession();
$result = $session->evaluate($rule, '{"x": 42}');
echo $result; // "true"
```

</div>

## The Engine

The `Engine` holds the evaluation configuration and the registered custom operators.

To build and configure an engine in each runtime:

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
import json
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
engineWithOps, err := datalogic.NewEngineBuilder().
    AddOperator("double", func(argsJson string) (string, error) {
        // implementation
        return "result", nil
    }).
    Build()
if err != nil {
    panic(err)
}
defer engineWithOps.Close()
```

```java
import com.goplasmatic.datalogic.Engine;

// 1. Default engine (AutoCloseable; close it when done)
Engine engine = new Engine();

// 2. Engine with custom operators via the builder
// (argsJson is a JSON array string; parse with your JSON library, Jackson shown)
Engine engineWithOps = Engine.builder()
    .addOperator("double", argsJson -> {
        int n = mapper.readTree(argsJson).get(0).asInt();
        return String.valueOf(n * 2);
    })
    .build();
```

```csharp
using Goplasmatic.Datalogic;

// 1. Default engine
using var engine = new Engine();

// 2. Engine with custom operators via the builder
using var engineWithOps = Engine.Builder()
    .AddOperator("double", argsJson =>
    {
        var n = System.Text.Json.Nodes.JsonNode.Parse(argsJson)![0]!.GetValue<double>();
        return (n * 2).ToString();
    })
    .Build();
```

```php
use Goplasmatic\Datalogic\Engine;

// 1. Default engine
$engine = new Engine();

// 2. Engine with custom operators via the builder
$engineWithOps = Engine::builder()
    ->addOperator('double', function (string $argsJson): string {
        $args = json_decode($argsJson, true);
        return (string) ((int) $args[0] * 2);
    })
    ->build();
```

</div>

The engine:
- owns the registered custom operators, fixed at `build()`
- holds the evaluation configuration
- provides the compile and evaluate methods

Operator registration is builder-only. To change a running engine's
operators or configuration, start a builder from it with
`Engine::to_builder()` and build a new engine; the two engines share
each operator instance.

## Context Stack

The context stack tracks variable scope during evaluation. Iterator
operators such as `map`, `filter` and `reduce` push a frame for each
element.

```rust
// In a filter operation, "" refers to the current element
let r = datalogic_rs::eval_str(
    r#"{"filter": [[1, 2, 3, 4, 5], {">": [{"var": ""}, 3]}]}"#,
    r#"{}"#,
).unwrap();
// Result: "[4,5]"
```

Inside an iterator body:
- `{"var": ""}` reads the current element
- `val` with a level marker (`{"val": [[2], "field"]}`) reads an enclosing frame or the root (see [Variable Access](../operators/variable-access.md))
- each nested iterator pushes its own frame and pops it when done

## Type Coercion

Several operators coerce types:

### Arithmetic
- Arithmetic operators parse numeric strings as numbers (`"5" + 3 = 8`)
- A non-numeric string, including `"NaN"`, `"inf"` and `"infinity"`, raises
  a `Thrown { type: "NaN" }` error by default; change that with
  [`EvaluationConfig::arithmetic_nan_handling`](../advanced/configuration.md#nan-handling)
- Integers stay exact up to the `i64` range: values above 2^53 compare,
  divide and sort without rounding

### Comparison
- `==` performs loose equality (with type coercion)
- `===` performs strict equality (no coercion)

### Truthiness
By default, the engine uses JavaScript-style truthiness:
- Falsy: `false`, `0`, `NaN`, `""`, `null`, `[]`, `{}`
- Truthy: everything else

`EvaluationConfig::truthy_evaluator` changes the rule, and `Engine::truthy_of`
(`truthy` in the bindings) applies the engine's rule to a value you
already hold.

## Thread Safety

`Logic` is `Send + Sync`, so you can share it across threads in an `Arc`:

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

A `Session` borrows its engine. To keep a session in a struct field, move
it to another thread, or hold it across an `.await`, use `SharedSession`,
which holds the engine by `Arc` (`SharedSession::new(Arc<Engine>)`); see
[Thread Safety](../advanced/threading.md).

## Next Steps

- [Operators Overview](../operators/overview.md): every available operator
- [Configuration](../advanced/configuration.md): customize evaluation behavior
- [Custom Operators](../advanced/custom-operators.md): extend the engine with your own logic
- [Migration Guide](../migration.md): move from v4 to v5, or from an earlier 5.x release
