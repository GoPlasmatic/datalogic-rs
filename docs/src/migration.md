# Migration Guide

This page covers the behaviour changes in 5.8.0 and gives an overview of
the move from v4 to v5. The full v4 to v5 cookbook (every renamed call,
cargo-feature swap and error-handling update) and the notes for earlier
5.x releases live in [`MIGRATION.md`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md)
at the repo root, which is authoritative.

## Upgrading to 5.8

5.8.0 adds API (rule checking, rule facts, the operator catalogue,
per-compile templating, `MissingVar`, `Roots`, `SharedSession` and more)
without breaking source compatibility. A rule's result can change in the
cases below, most of them bug fixes. The
[CHANGELOG](https://github.com/GoPlasmatic/datalogic-rs/blob/main/CHANGELOG.md)
has every item with examples.

**Errors**

- `Error::operator()`, and `"operator"` in a serialized error, name the
  innermost failing operator, custom operators included. Before 5.8 it
  was the root operator unless a deeper site set one, and traced runs
  always reported the root.
- `format_date` with a format specifier chrono does not know (`"%Q"`) or a
  trailing `%` is an `InvalidArguments` error instead of a panic.

**Data and comparison**

- Only a single-key `{"datetime": ..}` or `{"timestamp": ..}` object is a
  datetime. A record that has such a field next to others is ordinary data
  for `==`, `===`, `in`, `distinct` and `type`.
- Integers above 2^53 compare, divide and sort exactly in `==`, `===`,
  `<` and the other comparisons, `/`, `%` and `sort`; they are no longer
  rounded or judged equal.
- `sort` by a field reads a missing field as `null`, so it ties with a
  `null` field.
- `filter` with `===` / `!==` against `null` treats a missing field as
  `null`, as the other iterators did.

**Arithmetic and strings**

- The strings `"NaN"`, `"inf"` and `"infinity"`, and numeric literals that
  overflow, no longer coerce to non-finite numbers: arithmetic applies NaN
  handling to them, `==` and `<` raise an error, and `abs`, `ceil` and
  `floor` raise `InvalidArguments`.
- `NanHandling::CoerceToZero` substitutes 0 instead of skipping the value:
  `{"*": [2, "x", 3]}` is `0`, not `6`.
- Variadic `+` and `*` coerce numeric strings as the two-argument forms do.
- `ceil` and `floor` return a float for a result outside the `i64` range
  instead of saturating.
- `lower` and `upper` apply context-sensitive Unicode case rules
  (`"ΟΔΟΣ"` lowers to `"οδος"`).
- `type` classifies strings with the datetime and duration parsers:
  `"password1"` is `"string"` (it was `"duration"`).
- A rule that passes an expression where an operator distinguishes a
  literal argument (a single array for `+`, `*`, `max` and `min`, a `var`
  path built with `cat`) gives the same result with or without constant
  folding: `{"+": [{"if": [true, [1, 2], 0]}]}` is `3` with folding on
  or off.
- A rule compiled on one engine and evaluated on an engine with different
  number coercion, NaN, division, loose-equality or truthiness settings
  follows the evaluating engine's settings; before 5.8, values folded at
  compile time kept the compiling engine's.
- `Logic::to_json()` writes a rule that reads back as the same rule: a
  single array argument stays wrapped (`{"max": [[1, 2]]}`), a dotted
  `val` path stays in `val` form, and strings are escaped.

**Limits**

- A `reduce` accumulator may nest at most 1,024 levels; deeper is an
  `InvalidArguments` error instead of a stack overflow.
- Tensor constructors (`zeros`, `full`, `scatter`, `rle_expand`,
  `one_hot`, `pad`) refuse more than 2^28 elements.
- `fractional` clamps bucket weights to `i32::MAX`, as flagd does.
- Operation counts from `evaluate_metered` rise for iterator shapes that
  took a fast path, to match the general path.

**Deprecations** (removed in 6.0)

- WASM: the `CompiledRule` class and the free `evaluate` and
  `evaluateWithTrace` functions. Build an `Engine` and use `compile`,
  `evalStr` and `evaluateWithTrace` on it.
- Node and WASM: `evaluateNumber`. Use `evaluateFloat` (or `evaluateInt`).

If you install the Node package: 5.1.1 through 5.7.1 published
`@goplasmatic/datalogic-node` without its `index.js` loader, so
`require('@goplasmatic/datalogic-node')` failed. 5.8.0 ships it again.

## v4 to v5 Migration

### v5 is a hard cliff

v5 has **no compatibility shim**. The pre-release `compat` feature and
the `LegacyApi` trait are gone. Plan a single cutover: update Cargo.toml,
run a find-and-replace pass, and re-run your test suite.

Rules and data keep the same JSONLogic format; the changes are in the Rust
API.

### What changed at a glance

- **Type renames.** `DataLogic` → [`Engine`], `CompiledLogic` →
  [`Logic`], `Operator` → [`CustomOperator`], `ArenaValue` →
  [`DataValue`], `ArenaContextStack` →
  [`operator::EvalContext`](https://docs.rs/datalogic-rs/latest/datalogic_rs/operator/struct.EvalContext.html).
  `Evaluator` is gone (args arrive pre-evaluated).
- **Method renames.** The `evaluate_*` methods become `eval_*`. `evaluate_str`
  → `eval_str`, `evaluate_borrowed` → `eval_borrowed`. The
  `serde_json::Value`-shaped variants (`evaluate_json_value`,
  `evaluate_owned`, `evaluate_ref`, …) collapse into one typed entry
  point: `engine.eval_into::<T, _, _>(rule, data)` (or
  `datalogic_rs::eval_into::<T, _, _>(...)` at the module level), gated
  on `feature = "serde_json"`.
- **Builder construction.** `DataLogic::with_config(c)` /
  `with_preserve_structure()` / `with_config_and_structure(c, s)` all
  collapse into [`Engine::builder()`] with `.with_config(c)` and
  `.with_templating(s)` setters.
- **Compilation accepts more shapes.** `engine.compile(rule)` takes any
  [`IntoLogic`]: `&str`, `&String`, `&OwnedDataValue`, `OwnedDataValue`,
  `&serde_json::Value` (gated on `serde_json`).
- **Module-level helpers for one-shot calls.** `datalogic_rs::eval`,
  `datalogic_rs::eval_str`, `datalogic_rs::eval_into` and
  `datalogic_rs::compile` use a shared default engine, so a one-shot
  call needs no `Engine`.
- **Sessions are explicit.** Reusable arenas live on
  [`Session`] (`engine.session()`); the session never auto-resets,
  so you call `session.reset()` between batches.
- **Trace surface is a session.** `engine.trace().eval_str(rule, data)`
  returns a [`TracedRun<R>`] with `result: Result<R, Error>` plus
  `steps` and `expression_tree`. Available on `feature = "trace"`.
  The old `TracedResult` type is gone; successful and failed runs
  share the same `TracedRun<R>` shape.
- **Custom operators take pre-evaluated args.** Implementations get
  `args: &[&'a DataValue<'a>]`, a `&mut EvalContext<'_, 'a>`, and a
  `&'a bumpalo::Bump`; they return `&'a DataValue<'a>`.
- **Operator registration is builder-only.** `Engine` is immutable
  after `build()`. Register every custom operator on the
  `EngineBuilder` before calling `.build()`.
- **Error is structured.** `Error` is a struct with `kind`,
  `operator()`, `node_ids()`, `tag()` and `code()`, plus a stable JSON
  wire format.
  Construct via `Error::invalid_arguments(...)`, `Error::type_error(...)`,
  `Error::custom_message(...)`, `Error::wrap(...)`.
- **`preserve` operator removed.** Literal scalars and arrays pass
  through inline; templated objects belong in templating mode
  (`Engine::compile_template` for one rule, or
  `Engine::builder().with_templating(true).build()` for the engine; both
  need `feature = "templating"`).
- **Edition 2024 + `#![forbid(unsafe_code)]`.**

### Feature-flag rename

The pre-release `compat` feature is gone. The replacement is
purpose-named:

| v4 / pre-release feature | v5 feature | What it enables |
|---|---|---|
| `compat` (mixed interop + shims) | `serde_json` | `&serde_json::Value` interop and the typed `eval_into::<T>` paths |
| `preserve` | `templating` | Templating mode and `Engine::builder().with_templating(true)` |
| `trace` | `trace` | `engine.trace()` (transitively enables `serde_json`) |

### Quick before/after sketch

```rust
// v4
use datalogic_rs::DataLogic;
let mut engine = DataLogic::with_config(my_config);
engine.add_operator("double".to_string(), Box::new(MyOp));
let compiled = engine.compile(&rule_value)?;
let result: Value = engine.evaluate_owned(&compiled, data)?;
```

```rust
// v5
use datalogic_rs::Engine;
let engine = Engine::builder()
    .with_config(my_config)
    .add_operator("double", MyOp)
    .build();
let compiled = engine.compile(&rule_value)?;               // accepts &Value via `serde_json`
let mut session = engine.session();                        // reuse one arena for the compiled logic
let result = session.eval(&compiled, &data_value)?;        // OwnedDataValue
let result_str = session.eval_str(&compiled, data_str)?;   // String (JSON)
let v: serde_json::Value = session.eval_into(&compiled, &data_value)?;  // typed
```

### Custom operators

```rust
// v5 (final)
use datalogic_rs::{CustomOperator, DataValue, Engine, Result};
use datalogic_rs::operator::EvalContext;
use bumpalo::Bump;

struct DoubleOperator;
impl CustomOperator for DoubleOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        // args are already evaluated; no Evaluator call.
        let n = args.first()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}

let engine = Engine::builder()
    .add_operator("double", DoubleOperator)
    .build();
```

### Where to look next

- The repo-root [`MIGRATION.md`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md)
  has the per-call cookbook.
- The [API Reference](rust/api-reference.md) covers the v5 methods.
- The [Quick Start](getting-started/quick-start.md) shows the
  module-level helpers.

[`Engine`]: rust/api-reference.md
[`Logic`]: rust/api-reference.md
[`CustomOperator`]: advanced/custom-operators.md
[`DataValue`]: rust/api-reference.md
[`Session`]: rust/api-reference.md
[`TracedRun<R>`]: rust/api-reference.md
[`IntoLogic`]: rust/api-reference.md
[`Engine::builder()`]: rust/api-reference.md

---

## v3 to v4 Migration

If you are moving from v3 straight to v5, skip the v3 to v4 step: nothing
in it applies to this codebase. Read the
[v4 to v5 section](#v4-to-v5-migration) above and the repo-root
`MIGRATION.md`.

### Getting Help

If you hit a problem during migration:

1. Check the [API Reference](rust/api-reference.md)
2. Review the [examples](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs/examples)
3. Open an issue on [GitHub](https://github.com/GoPlasmatic/datalogic-rs/issues)
