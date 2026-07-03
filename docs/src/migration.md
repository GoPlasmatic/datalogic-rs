# Migration Guide

This page is a quick conceptual overview. The full v4 → v5 cookbook —
every renamed call, every cargo-feature swap, every error-handling
update — lives in [`MIGRATION.md`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md)
at the repo root. Treat that file as authoritative.

## v4 to v5 Migration

### v5 is a hard cliff

v5 has **no compatibility shim**. The pre-release `compat` feature and
the `LegacyApi` trait are gone — there is no transitional crate
configuration. Plan a single cutover: update Cargo.toml, run a
find-and-replace pass, and re-run your test suite.

The on-the-wire JSONLogic spec is unchanged — your rules and data still
look the same. Everything that changes is on the Rust side.

### What changed at a glance

- **Type renames.** `DataLogic` → [`Engine`], `CompiledLogic` →
  [`Logic`], `Operator` → [`CustomOperator`], `ArenaValue` →
  [`DataValue`], `ArenaContextStack` →
  [`operator::EvalContext`](https://docs.rs/datalogic-rs/latest/datalogic_rs/operator/struct.EvalContext.html).
  `Evaluator` is gone (args arrive pre-evaluated).
- **Method renames.** Every `evaluate_*` is now `eval_*`. `evaluate_str`
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
  `datalogic_rs::eval_str`, `datalogic_rs::eval_into`, and
  `datalogic_rs::compile` use a shared default engine — no need to
  construct an `Engine` for the simple cases.
- **Sessions are explicit.** Reusable arenas live on
  [`Session`] (`engine.session()`); the session never auto-resets,
  so callers call `session.reset()` between batches.
- **Trace surface is a session.** `engine.trace().eval_str(rule, data)`
  returns a [`TracedRun<R>`] with `result: Result<R, Error>` plus
  `steps` and `expression_tree`. Available on `feature = "trace"`.
  The old `TracedResult` type is gone — successful and failed runs
  share the same `TracedRun<R>` shape.
- **Custom operators take pre-evaluated args.** Implementations get
  `args: &[&'a DataValue<'a>]`, a `&mut EvalContext<'_, 'a>`, and a
  `&'a bumpalo::Bump`; they return `&'a DataValue<'a>`.
- **Operator registration is builder-only.** `Engine` is immutable
  after `build()`. Register every custom operator on the
  `EngineBuilder` before calling `.build()`.
- **Error is structured.** `Error` is a struct with `kind`,
  `operator()`, `node_ids()`, `tag()`, plus a stable JSON wire format.
  Construct via `Error::invalid_arguments(...)`, `Error::type_error(...)`,
  `Error::custom_message(...)`, `Error::wrap(...)`.
- **`preserve` operator removed.** Literal scalars and arrays already
  pass through inline; templated objects belong in templating mode
  (rebuild with `Engine::builder().with_templating(true).build()`,
  requires `feature = "templating"`).
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
        // args are already evaluated — no Evaluator call.
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
- [`rust/api-reference.md`](rust/api-reference.md) covers every v5 method.
- [`getting-started/quick-start.md`](getting-started/quick-start.md)
  walks through the new module-level helpers.

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

If you're stepping from v3 directly to v5, the v3 → v4 jump is a
historical layer that no longer matches anything in this codebase. Read
the [v4-to-v5 section](#v4-to-v5-migration) above and the repo-root
`MIGRATION.md`; everything you need to land on v5 is covered there.

### Getting Help

If you encounter issues during migration:

1. Check the [API Reference](rust/api-reference.md)
2. Review the [examples](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs/examples)
3. Open an issue on [GitHub](https://github.com/GoPlasmatic/datalogic-rs/issues)
