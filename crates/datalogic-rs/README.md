# datalogic-rs

[![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
[![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A fast, type-safe Rust implementation of [JSONLogic](http://jsonlogic.com)
for evaluating logical rules as JSON. Compile a rule once, evaluate it
millions of times across threads with zero overhead — same engine powers
a **rule engine**, a **JSON template engine**, or a **safe expression
evaluator**.

This is the **Rust core** of the
[`datalogic-rs` monorepo](https://github.com/GoPlasmatic/datalogic-rs).
The repo also ships [WASM](https://www.npmjs.com/package/@goplasmatic/datalogic),
[Python](https://pypi.org/project/datalogic-py/), Go, and
[React](https://www.npmjs.com/package/@goplasmatic/datalogic-ui)
bindings — same rules, same semantics. For the cross-runtime overview
and the per-binding READMEs, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

## Install

```bash
cargo add datalogic-rs
```

The default build is `serde_json`-free and ships only the JSONLogic
baseline operators. Opt in to feature flags as needed — see the
[feature flag reference](#feature-flags) below.

## Hello, JSONLogic

```rust
let result = datalogic_rs::eval_str(r#"{"+": [1, 2, 3]}"#, r#"{}"#).unwrap();
assert_eq!(result, "6");
```

That's it. `eval_str` parses the rule, parses the data, evaluates, and
hands you back a JSON string. The free functions on the crate root
wrap a shared default `Engine` — explicit construction lets you add
custom operators, change config, or amortise compilation. The rest of
this README walks through **when to use which API**.

## Choosing your API: five tiers, one engine

The crate exposes five evaluation tiers in increasing order of control.
Pick by use case, not by curiosity — most callers want **Tier 0** for
ad-hoc work or **Tier 2** for repeated evaluation.

| Tier | Entry point                                              | Arena owner             | Returns                              | Use when                                              |
|------|----------------------------------------------------------|-------------------------|--------------------------------------|-------------------------------------------------------|
| **0** | `datalogic_rs::eval_str` / `eval` / `eval_into` / `compile` | lazy static `Engine`   | `String` / `OwnedDataValue` / `T` / `Logic` | One-shot scripts, ad-hoc evaluation, no custom config |
| **1** | `Engine::eval_str` / `eval` / `eval_into`                 | per-call `Bump`         | `String` / `OwnedDataValue` / `T`    | You need custom operators, config, or templating mode |
| **2** | `Engine::session()` → `Session::eval*`                    | session-owned `Bump`    | owned **or** `&DataValue<'a>` borrow | Hot loops, services, batch jobs                       |
| **3** | `Engine::evaluate(&Logic, data, &Bump)`                   | caller-owned `Bump`     | `&'a DataValue<'a>`                  | Zero-copy result pipelines, custom pool strategies    |
| **4** | `Engine::trace()` → `TracedSession::*`                    | session-owned + buffer  | `TracedRun<R>` (result + steps)      | Debugging, visualisation, instrumentation             |

### Tier 0 — Module-level one-shot

The free functions wrap a static default `Engine`. No construction,
no configuration. Three result shapes:

```rust
use datalogic_rs::{compile, eval, eval_str};

// JSON string in, JSON string out
let s = eval_str(r#"{">": [{"var": "x"}, 10]}"#, r#"{"x": 42}"#).unwrap();
assert_eq!(s, "true");

// JSON string in, OwnedDataValue out
let v = eval(r#"{"+": [1, 2]}"#, r#"{}"#).unwrap();
assert_eq!(v.as_i64(), Some(3));

// Compile once at the module level when the rule is fixed
let logic = compile(r#"{"==": [{"var": "status"}, "active"]}"#).unwrap();
```

With the `serde_json` feature, `eval_into::<T>` returns any
`T: DeserializeOwned`:

```rust
// Cargo.toml: datalogic-rs = { version = "5", features = ["serde_json"] }
let n: i64 = datalogic_rs::eval_into(r#"{"+": [1, 2, 3]}"#, r#"{}"#).unwrap();
assert_eq!(n, 6);
```

### Tier 1 — Engine one-shot

Construct an `Engine` when you need anything beyond defaults: custom
operators, a non-default `EvaluationConfig`, or templating mode.

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let result = engine.eval_str(
    r#"{"==": [{"var": "status"}, "active"]}"#,
    r#"{"status": "active"}"#,
).unwrap();
assert_eq!(result, "true");
```

Use `Engine::builder()` to register operators and tweak behaviour:

```rust
use datalogic_rs::{Engine, EvaluationConfig};

let engine = Engine::builder()
    .with_config(EvaluationConfig::safe_arithmetic())
    .with_templating(true)              // requires `templating` feature
    .build();
```

### Tier 2 — Session (the right default for repeated evaluation)

`Session` owns a reusable `bumpalo::Bump` and resets it between calls,
so peak memory tracks the largest single evaluation, not the sum.

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
let mut session = engine.session();

for x in 0..1_000 {
    let payload = format!(r#"{{"x": {x}}}"#);
    let result = session.eval_str(&compiled, &payload).unwrap();
    // ...consume `result`...
    session.reset();          // O(1); keeps chunks for the next iteration
}
```

`Session::eval_borrowed` returns a `&'a DataValue<'a>` borrow into the
session's own arena — skips the owned deep-clone when the result is
consumed before the next session call. For pre-sizing the arena after
a warm-up pass, use `session.allocated_bytes()` +
`session.reset_with_capacity(bytes)`.

**Tokio idiom:** `Arc<Engine>` shared across worker threads (it's
`Send + Sync`), one `Session` per task (it's `Send` but `!Sync`, moves
with the task across `.await` points).

Full pattern: [`examples/compile_once_evaluate_many.rs`](./examples/compile_once_evaluate_many.rs).

### Tier 3 — Zero-copy `evaluate(&Bump)`

When the result borrow can stay scoped to a caller-managed arena,
skip the owned deep-clone and use `Engine::evaluate` directly. The
caller owns the `Bump`; the library never resets it.

```rust
use bumpalo::Bump;
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"==": [{"var": "status"}, "active"]}"#).unwrap();

let arena = Bump::new();
let result = engine.evaluate(&compiled, r#"{"status": "active"}"#, &arena).unwrap();
assert_eq!(result.as_bool(), Some(true));
```

Reach for this tier when you have a pool-managed arena, are
pipelining values across stages without crossing the value boundary,
or want maximum control over when memory is reclaimed.

### Tier 4 — Traced evaluation (`trace` feature)

Enable the `trace` feature, then ask the engine for a `TracedSession`.
Each call records the expression tree + per-node execution steps.

```rust
// Cargo.toml: datalogic-rs = { version = "5", features = ["trace"] }
use datalogic_rs::Engine;

let engine = Engine::new();
let traced = engine.trace().eval_str(
    r#"{"and": [true, {"var": "x"}]}"#,
    r#"{"x": true}"#,
);
let result_string = traced.result.unwrap();
// `traced.steps` is the per-node execution trace
// (drop into the React debugger or process programmatically).
```

Full pattern: [`examples/tracing.rs`](./examples/tracing.rs).

## Input shapes

`Engine::evaluate` and `Session::eval_borrowed` accept any input the
caller is likely to have on hand, via the sealed [`EvalInput`] trait.
Per-call cost differs:

| Shape                                     | Cost per call                       |
|-------------------------------------------|-------------------------------------|
| `&str` (JSON literal)                     | parse + arena alloc                 |
| `&serde_json::Value` (`serde_json` feature) | deep-convert into the arena       |
| `&OwnedDataValue`                         | deep-borrow into the arena          |
| `DataValue<'a>` (by value)                | one arena alloc for the top node    |
| `&'a DataValue<'a>` (by reference)        | **zero** — pass-through             |

For the same-input-many-rules case, or when upstream stages already
produced an arena value, prefer the `&'a DataValue<'a>` path — it's
genuinely allocation-free.

The Tier 0 / Tier 1 one-shot methods (`eval`, `eval_str`,
`eval_into`) accept a similar set via the [`OwnedInput`] trait, which
omits the `DataValue<'a>` shapes (no caller arena to borrow from).

Runnable example: [`examples/zero_copy_input.rs`](./examples/zero_copy_input.rs).

[`EvalInput`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/trait.EvalInput.html
[`OwnedInput`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/trait.OwnedInput.html

## Working with `DataValue`

Evaluation returns `&'a DataValue<'a>` — an arena-allocated, borrowed
JSON-shaped value tree. The type lives in the sibling `datavalue`
crate (re-exported at the root and as `datalogic_rs::datavalue`).
Most callers only need a handful of accessors:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"var": "user.score"}"#).unwrap();
let mut session = engine.session();
let result = session.eval_borrowed(&compiled, r#"{"user": {"score": 42}}"#).unwrap();

assert_eq!(result.as_i64(), Some(42));
// Other accessors: .as_f64(), .as_str(), .as_bool(), .as_array(), .as_object().
```

Conversion to other shapes:

- **To a JSON string:** `value.to_string()` — `DataValue` and
  `OwnedDataValue` both implement `Display`.
- **To `serde_json::Value`** (requires `serde_json`): use
  `eval_into::<serde_json::Value>(...)`.
- **To a typed Rust struct** (requires `serde_json`): use
  `eval_into::<T>(...)` where `T: DeserializeOwned`.
- **Owned vs borrowed:** `DataValue<'a>` borrows from a `Bump`;
  `OwnedDataValue` is the heap-owned counterpart for crossing arena
  lifetimes. Convert via `.to_owned()` (borrowed → owned) and
  `.to_arena(&bump)` (owned → borrowed).

## Public types at a glance

| Type                           | Role                                                        |
|--------------------------------|-------------------------------------------------------------|
| [`Engine`]                     | Immutable evaluation engine; entry point for every tier     |
| [`EngineBuilder`]              | Builder for engines with custom config, operators, modes    |
| [`Logic`]                      | Compiled, thread-safe rule snapshot                         |
| [`Session`]                    | Arena-reusing handle for hot loops; caller resets           |
| [`DataValue`]                  | Arena-borrowed JSON-shaped value (returned from `evaluate`) |
| `OwnedDataValue`               | Heap-owned counterpart of `DataValue` (via `datavalue`)     |
| [`EvaluationConfig`]           | Behaviour knobs: NaN, division by zero, truthiness, coercion |
| [`CustomOperator`]             | Trait you implement to extend the engine                    |
| `operator::EvalContext`        | Opaque engine context passed to `CustomOperator::evaluate`  |
| [`Error`] / [`ErrorKind`]      | Unified error type with operator + node-id breadcrumbs      |
| [`TracedRun`] / [`TracedSession`] | Tracing types (`trace` feature)                          |

[`Engine`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.Engine.html
[`EngineBuilder`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EngineBuilder.html
[`Logic`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.Logic.html
[`Session`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.Session.html
[`DataValue`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/enum.DataValue.html
[`EvaluationConfig`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html
[`CustomOperator`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/trait.CustomOperator.html
[`Error`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.Error.html
[`ErrorKind`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/enum.ErrorKind.html
[`TracedRun`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.TracedRun.html
[`TracedSession`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.TracedSession.html

## Custom operators

Register custom operators on `Engine::builder()` and call them from
rules just like the built-ins. Args arrive **pre-evaluated** as
arena-resident `&DataValue<'a>` borrows; you allocate the result back
into the arena.

```rust
use bumpalo::Bump;
use datalogic_rs::{CustomOperator, DataValue, Engine, Result, operator::EvalContext};

struct Double;
impl CustomOperator for Double {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}

let engine = Engine::builder().add_operator("double", Double).build();
let result = engine.eval_str(r#"{"double": 21}"#, "null").unwrap();
assert_eq!(result, "42");
```

Runnable example: [`examples/custom_operator.rs`](./examples/custom_operator.rs).
Full guide: [Custom Operators](https://goplasmatic.github.io/datalogic-rs/advanced/custom-operators.html).

The `CustomOperator` trait is the headline extension point and is
**stable for the 5.x series** — no required-method additions, no
signature changes.

## Configuration

`EvaluationConfig` controls edge-case behaviour:

- **NaN handling** (`NanHandling`) — what happens when arithmetic
  receives a non-number
- **Division by zero** (`DivisionByZeroHandling`)
- **Truthiness** (`TruthyEvaluator`) — JavaScript, Python, strict
  boolean, or a custom closure
- **Numeric coercion** (`NumericCoercionConfig`) — null-to-zero,
  bool-to-number, empty-string-to-zero, etc.
- **Recursion depth** — guards against pathological inputs

Presets like `EvaluationConfig::safe_arithmetic()` and
`EvaluationConfig::strict()` cover common postures. See the
[Configuration guide](https://goplasmatic.github.io/datalogic-rs/advanced/configuration.html)
and the runnable [`examples/configuration.rs`](./examples/configuration.rs).

## Templating mode

With `Engine::builder().with_templating(true)` (requires the
`templating` feature), multi-key objects in a compiled rule become
output-shaping templates — keys flow through to the output and
operator values become computed fields.

```rust
// Cargo.toml: datalogic-rs = { version = "5", features = ["templating"] }
use datalogic_rs::Engine;

let engine = Engine::builder().with_templating(true).build();
let result = engine.eval_str(
    r#"{"greeting": {"cat": ["Hello ", {"var": "name"}]},
        "isAdult": {">=": [{"var": "age"}, 18]}}"#,
    r#"{"name": "Jane", "age": 25}"#,
).unwrap();
// {"greeting":"Hello Jane","isAdult":true}
```

The 4.x JSONLogic `preserve` *operator* was removed in v5: literal
scalars / arrays work inline already; templated objects belong in
templating mode. Runnable example:
[`examples/structured_objects.rs`](./examples/structured_objects.rs).

## Error model

Every fallible path returns `Result<T, Error>`. `Error` carries:

- `kind: ErrorKind` — the discriminant (`ParseError`, `Thrown`,
  `VariableNotFound`, `TypeError`, `ArithmeticError`, `Custom`, …)
- `operator: Option<&'static str>` — the outermost failing operator
- `node_ids: Vec<u32>` — breadcrumbs from the compiled tree; resolve
  to a JSON path via `Error::resolve_path(&logic)` which returns a
  `Vec<PathStep>` you can print or serialise

```rust
use datalogic_rs::{Engine, ErrorKind};

let engine = Engine::new();
let err = engine.eval_str(r#"{"var": "missing"}"#, r#"{}"#);
// Default config: variable misses return null, not an error.
// Switch to a strict config to surface them as `VariableNotFound`.
```

Runnable example: [`examples/error_handling.rs`](./examples/error_handling.rs).

## Thread safety

| Type                           | `Send` | `Sync` | Notes                                                              |
|--------------------------------|:------:|:------:|--------------------------------------------------------------------|
| `Engine`                       | ✅     | ✅     | Construct once, share via `Arc`                                    |
| `Logic`                        | ✅     | ✅     | Compiled rules; use `Engine::compile_arc` for cross-thread sharing |
| `Session`                      | ✅     | ❌     | Owns a `Bump`; open one per thread / per task                      |
| `CustomOperator` implementors  | ✅     | ✅     | Required by the trait bound                                        |

Runnable example: [`examples/thread_safety.rs`](./examples/thread_safety.rs).

## Feature flags

| Feature           | Effect                                                                    |
|-------------------|---------------------------------------------------------------------------|
| `serde_json`      | `&serde_json::Value` interop and `eval_into::<T>` typed deserialisation   |
| `templating`      | Structure-preservation (templating) mode                                  |
| `datetime`        | Date / time operators (pulls in `chrono`)                                 |
| `trace`           | Execution-step recording for the debugger (implies `serde_json`)          |
| `error-handling`  | `try` / `throw` operators                                                 |
| `ext-string`, `ext-array`, `ext-control`, `ext-math` | Optional operator families             |

The default build is `serde_json`-free; opt in via
`features = ["serde_json"]` when you need the value boundary.

## Performance

Compiled rules dispatch through a single `OpCode` enum (no string
lookups), values live in a `bumpalo::Bump` arena (no per-result heap
allocation), and read-through operators like `var` borrow zero-copy
from the caller's input. Geomean ~9.7 ns/op across 44 operator suites
on Apple M2 Pro — see the cross-library comparison in
[`tools/benchmark/BENCHMARK.md`](../../tools/benchmark/BENCHMARK.md).

## Migrating from v4

v5 is a breaking release with a hard cliff — no `compat` feature, no
deprecated method shims. Headline renames: `DataLogic` → `Engine`,
`evaluate_json` → `eval_str` / `eval_into::<T>`, `Operator` →
`CustomOperator`, `with_config(...)` →
`Engine::builder().with_config(...).build()`. See
[`MIGRATION.md`](../../MIGRATION.md) for the full v4 → v5 cookbook and
[`CHANGELOG.md`](CHANGELOG.md) for the chronological breakage list.

## Learn more

- [Repo README](https://github.com/GoPlasmatic/datalogic-rs#readme) — cross-runtime overview, per-binding READMEs
- [Documentation site](https://goplasmatic.github.io/datalogic-rs/) — long-form guide, operator reference, advanced topics
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/) — try rules live in the visual debugger
- [`docs.rs/datalogic-rs`](https://docs.rs/datalogic-rs) — Rust API reference
- [`examples/README.md`](./examples/README.md) — index of runnable examples
- [`tests/README.md`](./tests/README.md) — JSONLogic suite format

## License

Apache 2.0 — see [LICENSE](../../LICENSE).
