# FAQ

## General

### What is JSONLogic?

JSONLogic is a way to write portable, safe logic rules as JSON. The
specification is available at [jsonlogic.com](https://jsonlogic.com).

### Why use datalogic-rs instead of the reference implementation?

- **Performance**: about 84x faster than json-logic-js across the shared
  benchmark suites (see [Performance](performance.md))
- **One engine in eight runtimes**: Rust, Node.js, browser WASM, Python,
  Go, JVM, .NET and PHP all run the same core
- **Thread safety**: `Logic` is `Send + Sync`; wrap it in `Arc` to share it
- **Extended operators**: datetime, string, array and object helpers,
  `try` / `throw`, flagd feature-flag operators
- **Rule checking**: `check` reports unknown operators and wrong argument
  counts before a rule runs
- **No `unsafe`**: the crate is built with `#![forbid(unsafe_code)]`

### Is datalogic-rs fully compatible with JSONLogic?

It passes the official JSONLogic test suite and adds operators beyond
the specification. A few defaults differ from json-logic-js (an empty
object is falsy, cross-type `==` raises an error); see
[Coming from json-logic-js](coming-from-json-logic-js.md#behavioral-differences-to-know).

---

## Rust Usage

### Should I use v4 or v5?

**Use v5** (5.8.1 is current) for new projects. The default build does
not pull in `serde_json`, and you can evaluate into your own arena. v5
has no compatibility shim for v4 code, so plan a single cutover; the
[Migration Guide](migration.md) and the repo-root `MIGRATION.md` have the
details.

### How do I share compiled rules across threads?

`Logic` is `Send + Sync`. Wrap it in `Arc` to share:

```rust
use datalogic_rs::Engine;
use std::sync::Arc;

let engine = Arc::new(Engine::new());
let compiled = engine.compile_arc(rule).unwrap();

let compiled_clone = Arc::clone(&compiled);
std::thread::spawn(move || {
    let mut session = engine.session();
    session.eval_str(&compiled_clone, data)
});
```

A `Session` borrows its engine. To store a session in a struct, send it
to another thread, or hold it across an `.await`, use `SharedSession`,
which holds the engine by `Arc`.

### How do I find out which data fields a rule reads?

Ask the compiled rule for its facts (`facts()` in every binding):

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let logic = engine
    .compile(r#"{"and": [{">": [{"var": "age"}, 18]}, {"==": [{"var": "country"}, "US"]}]}"#)
    .unwrap();
let facts = logic.facts();
let reads: Vec<String> = facts.reads().iter().map(|p| p.segments().join(".")).collect();
assert_eq!(reads, ["age", "country"]);
assert!(facts.reads_complete()); // no computed path, no custom operator
```

The facts also list the operators the rule uses and whether its result
depends only on its data. See [Rule Analysis](advanced/rule-analysis.md#what-a-rule-reads-logicfacts).

### Why are custom operator arguments pre-evaluated in v5?

With pre-evaluated arguments, a custom operator works like a built-in:
the engine evaluates the arguments, hands you `&DataValue<'a>` borrows,
and you return another arena allocation. This drops the boundary
conversion the v4 `Operator` trait paid on every call and the separate
`Evaluator` trait.

A custom operator therefore cannot short-circuit. Lazy evaluation belongs
to built-ins such as `and`, `or`, `if` and `??`.

### What's the difference between `eval`, `eval_str`, `eval_into`, and `evaluate`?

| Method | Input | Output | Notes |
|--------|-------|--------|-------|
| `datalogic_rs::eval_str` (and `eval` / `eval_into`) | `R: IntoLogic`, `D: OwnedInput` | `String` (or `OwnedDataValue` / `T`) | Module-level helper backed by a default engine. Use it when you need no custom operators or configuration. |
| `Engine::eval_str` (and `eval` / `eval_into` / `eval_as`) | `R: IntoLogic`, `D: OwnedInput` | `String` (or `OwnedDataValue` / `T`) | One-shot through a configured engine, with a fresh arena per call. |
| `Engine::evaluate` | `&Logic`, any `EvalInput`, `&Bump` | `&'a DataValue<'a>` | Hot path. You own the arena; the result borrows from it. |
| `Session::eval_str` (and `eval` / `eval_into` / `eval_as`) | `&Logic`, `D: EvalInput` | `String` (or `OwnedDataValue` / `T`) | Reuses the session's arena across calls. You call `session.reset()` between batches. |
| `Session::eval_borrowed` | `&Logic`, `D: EvalInput` | `&'a DataValue<'a>` | Zero-copy result; valid until the next `&mut self` call. |

The typed `eval_into::<T>` paths (and the `serde_json::Value` boundary
on `EvalInput` / `IntoLogic`) require `feature = "serde_json"`.

---

## JavaScript / WASM Usage

### Do I need to call `init()` in Node.js?

No. Under Node.js the package resolves to its `nodejs` build, which
instantiates the module on load:

```javascript
const { Engine } = require('@goplasmatic/datalogic-wasm');
new Engine().evalStr('{"==": [1, 1]}', '{}'); // "true"
```

On a Node.js server, the native `@goplasmatic/datalogic-node` package is
faster and takes JS objects.

### Why do I need to JSON.stringify my data?

The WASM binding takes and returns JSON text, so rules, data and results
cross the JS/WASM boundary as strings:

```javascript
const engine = new Engine();
const rule = engine.compile(JSON.stringify(logic));
const value = JSON.parse(rule.evaluate(JSON.stringify(data)));
```

### How do I use this with TypeScript?

The package includes type definitions:

```typescript
import init, { Engine } from '@goplasmatic/datalogic-wasm';

await init();
const engine = new Engine();
const result: string = engine.evalStr('{"==": [1, 1]}', '{}');
```

The free `evaluate` function and the `CompiledRule` class are deprecated
in 5.8.0 and removed in 6.0; use `Engine`.

---

## React UI

### Why does the editor need explicit dimensions?

The editor draws with React Flow, which needs a container with a defined
height to lay out nodes and the viewport.

```tsx
<div style={{ height: '500px' }}>
  <DataLogicEditor value={expression} />
</div>
```

### Can I use this with Next.js?

Yes. With the App Router, render it from a client component:

```tsx
'use client';

import '@goplasmatic/datalogic-ui/styles.css';
import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

export function Editor({ expression }) {
  return <DataLogicEditor value={expression} />;
}
```

---

## Operators

### How do I access array elements by index?

Use the `var` operator with numeric path segments:

```json
{"var": "items.0.name"}
```

### What's the difference between `==` and `===`?

- `==`: Loose equality (with type coercion, like JavaScript)
- `===`: Strict equality (no type coercion)

```json
{"==": [1, "1"]}   // true
{"===": [1, "1"]}  // false
```

### How do I handle missing data?

Test for it with `missing` or `missing_some`:

```json
{"if": [
    {"missing": ["user.email"]},
    "Email required",
    "Valid"
]}
```

Or give `var` a default:

```json
{"var": ["user.email", "no-email@example.com"]}
```

`or` also works as a fallback, because it returns the deciding operand
rather than a boolean:

```json
{"or": [{"var": "nickname"}, "Anonymous"]}
// Data: {"nickname": "Ada"}  -> "Ada"
// Data: {}                   -> "Anonymous"
```

### What happened to the `preserve` operator?

v5 removed it. Literal scalars and arrays pass through inline, and
templated objects belong in templating mode: `Engine::compile_template`
for one rule, or `Engine::builder().with_templating(true).build()` for the
engine (both need `feature = "templating"`).

---

## Configuration

### How do I handle NaN in arithmetic?

Set `NanHandling` on the configuration:

```rust
use datalogic_rs::{Engine, EvaluationConfig, NanHandling};

let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::IgnoreValue);
let engine = Engine::builder().with_config(config).build();
```

Options: `ThrowError` (default), `CoerceToZero` (substitutes 0), `IgnoreValue` (skips the value), `ReturnNull`.

### How do I make a misspelled data path fail instead of returning `null`?

Set `MissingVar::Error` (`"missing_var": "error"` in a binding's config).
A `var` / `val` read that finds nothing then raises `VariableNotFound`
naming the path:

```rust
use datalogic_rs::{Engine, EvaluationConfig, MissingVar};

let config = EvaluationConfig::default().with_missing_var(MissingVar::Error);
let engine = Engine::builder().with_config(config).build();
let err = engine
    .eval_str(r#"{">": [{"var": "user.agee"}, 18]}"#, r#"{"user": {"age": 30}}"#)
    .unwrap_err();
assert_eq!(err.tag(), "VariableNotFound");
```

A `var` with a default, a field that is present and `null`, `missing`,
`missing_some` and `exists` behave as before, and `try` catches the error.
See [Missing Variables](advanced/configuration.md#missing-variables).

### How do I change division by zero behavior?

```rust
use datalogic_rs::{EvaluationConfig, DivisionByZeroHandling};

let config = EvaluationConfig::default()
    .with_division_by_zero(DivisionByZeroHandling::ReturnNull);
```

Options: `ReturnSaturated` (default, `f64::MAX/MIN` with the dividend's
sign), `ThrowError`, `ReturnNull`, `ReturnInfinity`.

The setting applies to the float path only: an integer dividend over an
integer zero (`{"/": [10, 0]}`) raises `Thrown { type: "NaN" }` under
every setting. `{"/": [10.5, 0]}` takes the configured path. See
[Division by Zero](advanced/configuration.md#division-by-zero) for the
comparison table.

---

## Troubleshooting

### "Invalid operator" error

Outside templating mode, an unknown operator key compiles and raises
`InvalidOperator` when evaluation reaches it. `check` finds it before
the rule runs and suggests the operator one edit away (see
[Rule Analysis](advanced/rule-analysis.md)). Then either:

1. Fix the operator name (operators are case-sensitive)
2. Register a custom operator on the builder
3. Compile the rule as a template (`Engine::compile_template`, `feature = "templating"`) if the key is meant as an output field

### My template key runs as an operator instead of being emitted

This is the inverse problem, and it raises no error, only a wrong result.
In templating mode a single-key object whose key names an operator is an
operator call, so `{"type": {"var": "x"}}` runs the `type` operator rather
than emitting a `type` field. Every built-in name is affected (87 names,
counting the aliases `var`, `?:` and `match`, in a build with every
operator family), plus any custom operator you registered.

Turn on the key escape and prefix the key:
`Engine::builder().with_templating(true).with_template_key_escape('$')`,
then write `{"$type": ...}` to emit `type` and `{"$$type": ...}` to emit
a literal `$type`. It is off by default, so existing templates keep their
meaning. `check` in template mode also warns about an output key one edit
away from an operator name. Details in
[Structured Objects](./advanced/structured-objects.md#emitting-keys-that-are-operator-names)
and [Troubleshooting](./troubleshooting.md#a-template-key-runs-as-an-operator-instead-of-being-emitted).

### Performance issues with large expressions

1. Use a `Session` for repeated calls, so they reuse one arena
2. For the hottest path, call `Engine::evaluate` with a `bumpalo::Bump`
   you manage
3. Use `feature = "trace"` to see which sub-expressions run and how often
   (the trace records iteration counts, not timings), and a sampling
   profiler such as perf or Instruments for timing (see
   [Performance](performance.md#profiling))

### WASM initialization fails

With the web build, `await init()` before you construct an `Engine`:

```javascript
await init();
const engine = new Engine();
```

For more troubleshooting, see the [Troubleshooting Guide](troubleshooting.md).
