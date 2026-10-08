# Quick Start

This page takes you from a one-shot evaluation to a reusable `Engine`.

## The simplest path: one-shot helpers

For a one-off evaluation with default settings, call the module-level helper; you don't build an engine first.

<div class="codetabs">

```rust
let result = datalogic_rs::eval_str(
    r#"{">": [{"var": "score"}, 50]}"#,
    r#"{"score": 75}"#,
).unwrap();
assert_eq!(result, "true");
```

```javascript
import { apply } from '@goplasmatic/datalogic-node';

const result = apply(
  { '>': [{ var: 'score' }, 50] },
  { score: 75 }
);
console.log(result); // true
// browser/edge: same API via @goplasmatic/datalogic-wasm, see the WASM chapter
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

```java
import com.goplasmatic.datalogic.Engine;

try (Engine engine = new Engine()) {
    String result = engine.apply(
        "{\">\": [{\"var\": \"score\"}, 50]}",
        "{\"score\": 75}"
    );
    System.out.println(result); // "true"
}
```

```csharp
using Goplasmatic.Datalogic;

using var engine = new Engine();
var result = engine.Apply(
    """{">": [{"var": "score"}, 50]}""",
    """{"score": 75}"""
);
Console.WriteLine(result); // "true"
```

```php
use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
$result = $engine->apply(
    '{">": [{"var": "score"}, 50]}',
    '{"score": 75}'
);
echo $result; // "true"
```

</div>

The module-level helpers (`eval_str` in Rust, `apply` in Node.js, Python and Go) run on a default engine. Java, C# and PHP have no module-level function, so a default `Engine` and its `apply` method give the same one-shot. Use them for scripts, tutorials and code that needs no custom operators or configuration.

## When you need an Engine

Build an `Engine` when you need custom operators, a non-default configuration, templating mode, or a long-lived `Session` that reuses memory in a hot loop.

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
session.reset(); // Reset between evaluations to keep memory flat
```

```javascript
import { Engine } from '@goplasmatic/datalogic-node';

// 1. Create an engine
const engine = new Engine();

// 2. Compile once (returns a reusable Rule)
const rule = engine.compile({ '>': [{ var: 'score' }, 50] });

// 3. Evaluate via a session (reuses the arena across calls)
const sess = engine.session();
const result = sess.evaluate(rule, { score: 75 });
console.log(result); // true
// browser/edge: same API via @goplasmatic/datalogic-wasm, see the WASM chapter
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

```java
import com.goplasmatic.datalogic.Engine;
import com.goplasmatic.datalogic.Rule;
import com.goplasmatic.datalogic.Session;

// 1. Create an engine, 2. compile once, 3. evaluate via a session;
// try-with-resources frees the native handles
try (Engine engine = new Engine();
     Rule rule = engine.compile("{\">\": [{\"var\": \"score\"}, 50]}");
     Session session = engine.openSession()) {
    String result = session.evaluate(rule, "{\"score\": 75}");
    System.out.println(result); // "true"
}
```

```csharp
using Goplasmatic.Datalogic;

// 1. Create an engine
using var engine = new Engine();

// 2. Compile once
using var rule = engine.Compile("""{">": [{"var": "score"}, 50]}""");

// 3. Evaluate via a session (arena reuse across calls)
using var session = engine.OpenSession();
var result = session.Evaluate(rule, """{"score": 75}""");
Console.WriteLine(result); // "true"
```

```php
use Goplasmatic\Datalogic\Engine;

// 1. Create an engine
$engine = new Engine();

// 2. Compile once
$rule = $engine->compile('{">": [{"var": "score"}, 50]}');

// 3. Evaluate via a session (arena reuse across calls)
$session = $engine->openSession();
$result = $session->evaluate($rule, '{"score": 75}');
echo $result; // "true"
```

</div>

The [Rust chapter](../rust/overview.md) and each language's chapter cover engine configuration and sessions in depth.

## Working with Variables

Access data using the `var` operator:

```json
// Simple variable access
{ "var": "name" }
// Data: { "name": "Alice" }
// Result: "Alice"

// Nested access with dot notation
{ "var": "user.address.city" }
// Data: { "user": { "address": { "city": "New York" } } }
// Result: "New York"

// Default value for missing keys
{ "var": ["missing_key", "default_value"] }
// Data: {}
// Result: "default_value"
```

**Try it:**

<div class="playground-widget" data-logic='{"var": "user.address.city"}' data-data='{"user": {"address": {"city": "New York"}}}'>
</div>

## Conditional Logic

Use `if` for branching:

```json
{ "if": [{ ">=": [{ "var": "age" }, 18] }, "adult", "minor"] }

// Data: { "age": 25 }
// Result: "adult"

// Data: { "age": 15 }
// Result: "minor"
```

**Try it:**

<div class="playground-widget" data-logic='{"if": [{">=": [{"var": "age"}, 18]}, "adult", "minor"]}' data-data='{"age": 25}'>
</div>

## Combining Conditions

Use `and` and `or` to combine conditions:

```json
// AND: all conditions must be true
{ "and": [
    { ">=": [{ "var": "age" }, 18] },
    { "==": [{ "var": "verified" }, true] }
] }
// Data: { "age": 21, "verified": true }
// Result: true

// OR: at least one condition must be true
{ "or": [
    { "==": [{ "var": "role" }, "admin"] },
    { "==": [{ "var": "role" }, "moderator"] }
] }
// Data: { "role": "admin" }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "verified"}, true]}]}' data-data='{"age": 21, "verified": true}'>
</div>

## Array Operations

Filter, map, and reduce arrays:

```json
// filter: keep elements matching a condition ("" is the current element)
{ "filter": [{ "var": "numbers" }, { ">": [{ "var": "" }, 5] }] }
// Data: { "numbers": [1, 3, 5, 7, 9] }
// Result: [7, 9]

// map: transform each element
{ "map": [{ "var": "numbers" }, { "*": [{ "var": "" }, 2] }] }
// Data: { "numbers": [1, 2, 3] }
// Result: [2, 4, 6]
```

**Try it:**

<div class="playground-widget" data-logic='{"filter": [{"var": "numbers"}, {">": [{"var": ""}, 5]}]}' data-data='{"numbers": [1, 3, 5, 7, 9]}'>
</div>

## Error Handling

A failed evaluation is a structured error: a stable `type` tag, a message, the innermost operator that failed, and a breadcrumb of compiled-node ids leading to it:

```json
{ "+": ["text", 1] }
// Data: {}
// Error: { "type": "Thrown", "message": "Thrown: {\"type\":\"NaN\"}", "thrown": { "type": "NaN" }, "operator": "+", "node_ids": [3] }
```

The tag here is `Thrown`; its `thrown` payload is the JSONLogic-style `{"type": "NaN"}` value that arithmetic raises on a non-numeric string.

To recover from a runtime error inside the rule, wrap the expression in `try` (in the Rust crate, enable the `error-handling` feature; every language binding has it):

```json
{ "try": [{ "/": [10, { "var": "divisor" }] }, 0] }
// Data: { "divisor": 0 }
// Result: 0 (the division throws, so the fallback is returned)
```

**Try it:**

<div class="playground-widget" data-logic='{"try": [{"/": [10, {"var": "divisor"}]}, 0]}' data-data='{"divisor": 0}'>
</div>

Each binding's chapter covers how an uncaught error reaches your code (Rust `Result`, JavaScript and Python exceptions, Go `error` values, Java, C# and PHP exceptions): [Node.js](../nodejs/overview.md), [browser WASM](../javascript/api-reference.md), [Python](../python/api-gil.md), [Go](../go/quick-start.md), [Java](../jvm.md), [.NET](../dotnet.md), [PHP](../php.md).

`compile` accepts an unknown operator and leaves it to fail when evaluation reaches it. To catch such mistakes first, run `check` on the rule (`Engine::check` in Rust): it lists unknown operators with a "did you mean" suggestion and wrong argument counts, each located by a JSON Pointer ([Rule Analysis](../advanced/rule-analysis.md#checking-a-rule-enginecheck)). To make a misspelled data path fail instead of reading as `null`, set `MissingVar::Error` (`"missing_var": "error"` in a binding's config; see [Configuration](../advanced/configuration.md#missing-variables)).

## Next Steps

- [Basic Concepts](basic-concepts.md): how rules, compilation and evaluation fit together
- [Operators](../operators/overview.md): every operator with runnable examples
- [Use Cases & Examples](../use-cases/examples.md): complete rule patterns for real workloads
- Language chapters: [Rust](../rust/overview.md), [Node.js](../nodejs/overview.md), [JavaScript in the browser (WASM)](../javascript/installation.md), [Python](../python/installation.md), [Go](../go/installation.md), [Java / Kotlin](../jvm.md), [.NET](../dotnet.md), [PHP](../php.md)
