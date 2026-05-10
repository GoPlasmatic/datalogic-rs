# Migration Guide

This guide covers migrating between major versions of datalogic-rs.

## v4 to v5 Migration

### Overview

v5 is a breaking release that:

- Renames the public surface (`DataLogic` → `Engine`,
  `CompiledLogic` → `Logic`, `Operator` → `CustomOperator`).
- Makes one-shot evaluation **string-based** (`evaluate_str`) — the
  default build no longer pulls in `serde_json`.
- Switches custom operators to a **pre-evaluated, arena-resident** model
  (no more `evaluator.evaluate(args[i], ctx)` calls).
- Replaces ad-hoc constructors (`with_config`,
  `with_preserve_structure`, `with_config_and_structure`) with an
  **`EngineBuilder`**.
- Makes operator registration **builder-only** — engines are immutable
  after `build()`.
- Removes the `preserve` operator (templating moves into
  `preserve_structure` mode under `feature = "preserve"`).
- Reshapes `Error` into a struct with `kind` / `operator` / `path` and a
  stable JSON wire format.
- Edition 2024 + `#![forbid(unsafe_code)]`.

The on-the-wire JSONLogic spec is unchanged — your rules and data still
look the same. What changes is the Rust API around them.

### When to Migrate

**Migrate to v5 if:**

- You want a serde_json-free build, lower binary size, or fewer
  transitive deps.
- You want the arena evaluation path (`Engine::evaluate` returning
  `&DataValue<'a>`) for hot-path workloads.
- You want structured errors (`kind`, `operator`, `path`) and stable
  JSON serialisation.
- You want operator registration that the type system can statically
  freeze (no shared-mutable engines).

**Stay on v4 if:**

- You're shipping today and the v4 API is working — there is no rush.
- You depend on the old custom-operator semantics (lazy / unevaluated
  args via the `Evaluator` trait); v5 removed `Evaluator`, and emulating
  the old laziness inside a `CustomOperator` is non-trivial.

### Quick Migration Path (LegacyApi shim)

For the fastest possible upgrade, enable the `compat` feature and import
the `LegacyApi` trait. Every v4 method on `DataLogic` is reachable as a
`#[deprecated]` shim — your v4 code keeps compiling, and the compiler
points you at the v5 replacement per call site.

```toml
[dependencies]
datalogic-rs = { version = "5.0", features = ["compat"] }
```

```rust
use datalogic_rs::compat::LegacyApi;

let engine = datalogic_rs::Engine::with_config(my_config);  // shim — deprecated
engine.evaluate_json(rule, data)?;                           // shim — deprecated
```

Search for `compat::LegacyApi` to find every file that still uses the
v4 surface.

### Type Renames

| v4 | v5 |
|----|----|
| `DataLogic` | `Engine` |
| `CompiledLogic` | `Logic` |
| `Operator` (trait) | `CustomOperator` |
| `Evaluator` (trait) | _Removed — args are pre-evaluated_ |
| `ArenaValue<'a>` | `DataValue<'a>` |
| `ArenaContextStack<'a>` | `operator::EvalContext<'_, 'a>` |
| `ArenaOperator` | `CustomOperator` (with method renamed `evaluate_arena` → `evaluate`) |
| `Arc<CompiledLogic>` (auto-wrapped) | `Logic` (wrap in `Arc` yourself) |

`CompiledNode`, `OpCode`, `MetadataHint`, `PathSegment`, and `ReduceHint`
were public in v4 but are compile-internal in v5. If you reached into the
compiled tree, that path was already broken by the arena rewrite — there is
no shim. Translate failing-evaluation paths via `Logic::resolve_path` /
`Error::resolved_path` into the public `PathStep` type instead.

### Engine Construction

```rust
// v4
use datalogic_rs::DataLogic;
let engine = DataLogic::default();
let engine = DataLogic::new();
let engine = DataLogic::with_config(config);
let engine = DataLogic::with_preserve_structure();
let engine = DataLogic::with_config_and_structure(config, true);

// v5
use datalogic_rs::Engine;
let engine = Engine::default();
let engine = Engine::new();
let engine = Engine::builder().config(config).build();
let engine = Engine::builder().preserve_structure(true).build();
let engine = Engine::builder()
    .config(config)
    .preserve_structure(true)
    .build();
```

> `preserve_structure(...)` requires the `preserve` feature.

### Compilation

```rust
// v4 — accepts &serde_json::Value
let compiled = engine.compile(&rule_value)?;
// compiled is Arc<CompiledLogic>

// v5 — accepts &str
let compiled = engine.compile(rule_str)?;
// compiled is Logic. Wrap in Arc to share across threads:
let shared = std::sync::Arc::new(compiled);
```

If your rule is already a `serde_json::Value`, enable the `compat` feature
and call `compile_serde_value`:

```rust
#[cfg(feature = "compat")]
use datalogic_rs::compat::LegacyApi;
let compiled = engine.compile_serde_value(&rule_value)?;
```

Or convert directly:

```rust
let compiled = engine.compile(&rule_value.to_string())?;
```

### Evaluation

```rust
// v4
let result: Value = engine.evaluate(&compiled, &data)?;
let result: Value = engine.evaluate_owned(&compiled, data)?;
let result: Value = engine.evaluate_json(rule_str, data_str)?;

// v5 — pick the entry point based on what you have
let result: String = engine.evaluate_str(rule_str, data_str)?;     // one-shot

#[cfg(feature = "compat")]
let result: serde_json::Value = engine.evaluate_json_value(&rule_value, &data_value)?;

// Reusable arena (recommended for repeated calls)
let mut session = engine.session();
let result: String = session.evaluate_str(&compiled, data_str)?;
let result: OwnedDataValue = session.evaluate(&compiled, data)?;

// Hot path — caller owns the arena
use bumpalo::Bump;
let arena = Bump::new();
let result: &DataValue<'_> = engine.evaluate(&compiled, data, &arena)?;
```

`Engine::evaluate` accepts any `EvalInput` — `&'a DataValue<'a>`,
`DataValue<'a>`, `&'a str`, `&OwnedDataValue`, or
`&serde_json::Value` (`compat`).

### Custom Operators

The trait was renamed `Operator` → `CustomOperator`, args are now
pre-evaluated arena borrows, results are arena-allocated, and the
`Evaluator` trait is gone.

```rust
// v4
use datalogic_rs::{Operator, ContextStack, Evaluator, Result, Error};
use serde_json::{json, Value};

struct DoubleOperator;
impl Operator for DoubleOperator {
    fn evaluate(
        &self,
        args: &[Value],
        ctx: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        let v = evaluator.evaluate(&args[0], ctx)?;
        let n = v.as_f64().ok_or_else(|| Error::InvalidArguments("expected number".into()))?;
        Ok(json!(n * 2.0))
    }
}

// v5
use bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Error, Result};

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
            .ok_or_else(|| Error::invalid_arguments("expected number"))?;
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}
```

If you already had v4 operators using the older `ArenaOperator` / 
`evaluate_arena` names, the `compat` feature provides a `#[deprecated]`
trait alias that automatically forwards to the v5 trait — rename
`evaluate_arena` to `evaluate` to migrate fully.

### Operator Registration

```rust
// v4 — mutating method
let mut engine = DataLogic::new();
engine.add_operator("double".to_string(), Box::new(DoubleOperator));

// v5 — builder only
let engine = Engine::builder()
    .add_operator("double", DoubleOperator)
    .build();
```

If you already hold a `Box<dyn CustomOperator>`:

```rust
let engine = Engine::builder()
    .add_operator_boxed("double", boxed_op)
    .build();
```

### Error Handling

`Error` was a flat enum in v4. In v5 it is a struct:

```rust
pub struct Error {
    pub kind: ErrorKind,
    pub operator: Option<String>,
    pub path: Vec<u32>,
}
```

```rust
// v4
match engine.evaluate(&compiled, &data) {
    Ok(_) => {}
    Err(Error::InvalidOperator(op)) => { /* ... */ }
    Err(Error::InvalidArguments(msg)) => { /* ... */ }
    Err(_) => {}
}

// v5
use datalogic_rs::ErrorKind;
match engine.evaluate_str(rule, data) {
    Ok(_) => {}
    Err(err) => match &err.kind {
        ErrorKind::InvalidOperator(op) => {}
        ErrorKind::InvalidArguments(msg) => {}
        _ => {}
    },
}
// `err.kind_tag()` returns a stable string for cross-version matching.
// `err.operator` and `err.path` are populated automatically.
// `err.thrown_value()` accesses the `Thrown` payload.
```

Construct errors with the named shorthands:

```rust
Error::invalid_arguments("expected number")
Error::type_error("...")
Error::custom("...")
Error::wrap(some_io_error)   // any std::error::Error + Send + Sync + 'static
```

Errors serialise to a stable JSON shape:

```json
{
  "type": "InvalidArguments",
  "message": "expected number",
  "operator": "double",
  "path": [42, 7]
}
```

### Trace API

```rust
// v4
let trace = engine.evaluate_json_with_trace(logic, data)?;

// v5 (feature = "trace")
let run = engine.with_trace().evaluate_str(logic, data);
println!("{}", run.result.unwrap());
for step in &run.steps {
    // step.node_id, step.context, step.result, ...
}
```

The legacy `evaluate_json_with_trace` lives behind `compat::LegacyApi`.

### `preserve` Operator Removed

v4 had a `{"preserve": <value>}` operator. v5 removes it:

- Literal scalars and arrays already pass through inline.
- Templated objects belong in `preserve_structure` mode (rebuild with
  `Engine::builder().preserve_structure(true).build()`, requires
  `feature = "preserve"`).

### Truthiness Custom Callback

```rust
// v4
TruthyEvaluator::Custom(Arc::new(|v: &serde_json::Value| -> bool { ... }))

// v5
use datalogic_rs::datavalue::OwnedDataValue;
TruthyEvaluator::Custom(Arc::new(|v: &OwnedDataValue| -> bool { ... }))
```

### Feature Flags

v5 ships with `default = []` — no `serde_json` unless you opt in. Common
configurations:

```toml
# Pure v5, smallest deps
datalogic-rs = "5.0"

# Need serde_json::Value boundary or v4-compat shims
datalogic-rs = { version = "5.0", features = ["compat"] }

# Tracing (also implies compat for the legacy TracedResult shape)
datalogic-rs = { version = "5.0", features = ["trace"] }

# Templating
datalogic-rs = { version = "5.0", features = ["preserve"] }

# DateTime operators
datalogic-rs = { version = "5.0", features = ["datetime"] }
```

The `wasm` aggregate feature pulls in `datetime` + `trace` + `preserve`
for browser builds.

### LegacyApi and the `compat` feature

`features = ["compat"]` activates:

- `serde_json::Value` adapters: `Engine::evaluate_json_value`,
  `Engine::compile` (taking `&Value`) via `LegacyApi`, deep-convert helpers.
- The `LegacyApi` extension trait — bringing it into scope unlocks every
  4.x entry point (`evaluate_json`, `evaluate_owned`, `evaluate_ref`,
  `with_config`, `with_preserve_structure`, etc.) as `#[deprecated]` shims.
- The `ArenaValue` / `ArenaContextStack` / `ArenaOperator` deprecated
  aliases for the renamed types.
- `serde_json::Value` as an additional `EvalInput` shape for
  `Engine::evaluate` and `Session::evaluate`.

Every shimmed method is `#[deprecated(since = "5.0.0")]` — the compiler
will keep nudging you per call site. Plan to drop the feature in 5.1+.

### Migration Checklist

1. **Update Cargo.toml:**
   ```toml
   [dependencies]
   datalogic-rs = { version = "5.0", features = ["compat"] }  # transitional
   ```

2. **Rename types** with find-and-replace:
   - `DataLogic` → `Engine`
   - `CompiledLogic` → `Logic`
   - `Operator` (trait) → `CustomOperator`
   - `ArenaValue` → `DataValue`
   - `ArenaContextStack` → `operator::EvalContext`

3. **Replace constructors** with the builder:
   - `DataLogic::with_config(c)` → `Engine::builder().config(c).build()`
   - `DataLogic::with_preserve_structure()` → `Engine::builder().preserve_structure(true).build()`
   - `DataLogic::with_config_and_structure(c, p)` → `Engine::builder().config(c).preserve_structure(p).build()`

4. **Update evaluation calls:**
   - `engine.evaluate(&compiled, &data)` → `Session` / `Engine::evaluate` / `evaluate_json_value`
   - `engine.evaluate_owned(&compiled, data)` → same
   - `engine.evaluate_json(rule, data)` → `engine.evaluate_str(rule, data)` (returns `String`, not `serde_json::Value`)

5. **Migrate custom operators:**
   - Implement `CustomOperator` (not `Operator`)
   - Drop the `evaluator: &dyn Evaluator` parameter
   - Treat `args[i]` as already-evaluated `&DataValue<'a>` borrows
   - Allocate the result via `arena.alloc(...)`

6. **Move operator registration to the builder.** v5's `Engine` has no
   `add_operator` method — register before `build()`.

7. **Update error handling:**
   - Match on `err.kind` / `ErrorKind::*` instead of `Error::*`
   - Construct via `Error::invalid_arguments(...)` etc.
   - Drop `Error::Custom(string)` in favour of `Error::custom(...)` /
     `Error::wrap(...)`

8. **Remove uses of the `preserve` operator.** Rebuild the engine with
   `preserve_structure(true)` and rely on object-level templating.

9. **Drop the `compat` feature** once all `#[deprecated]` warnings are
   resolved. The `LegacyApi` trait disappears in 5.1.

10. **Test thoroughly.** The JSONLogic semantics are unchanged, but the
    evaluation entry points and error-construction paths are not — make
    sure your test suite exercises both error and success paths.

---

## v3 to v4 Migration

(Historical — only relevant if you're still on v3.)

### Overview

v4 redesigns the API for ergonomics and simplicity. The core JSONLogic
behavior is unchanged, but the Rust API is different.

**Key changes:**

- Simplified `DataLogic` engine API
- `CompiledLogic` automatically wrapped in `Arc`
- No more arena allocation (simpler lifetime management)
- New evaluation methods

### API Changes

#### Engine Creation

```rust
// v3
use datalogic_rs::DataLogic;
let engine = DataLogic::default();

// v4
use datalogic_rs::DataLogic;
let engine = DataLogic::new();
```

#### Compilation

```rust
// v3
let compiled = engine.compile(&logic)?; // not auto-wrapped

// v4
let compiled = engine.compile(&logic)?; // Arc<CompiledLogic>
```

#### Evaluation

```rust
// v3
let result = engine.evaluate(&compiled, &data)?;

// v4
let result = engine.evaluate_owned(&compiled, data)?;
let result = engine.evaluate(&compiled, &data)?;
```

#### Custom Operators

```rust
// v3
struct MyOperator;
impl Operator for MyOperator {
    fn evaluate(&self, args: &[Value], data: &Value, engine: &DataLogic) -> Result<Value> { /* ... */ }
}

// v4
use datalogic_rs::{Operator, ContextStack, Evaluator, Result};

struct MyOperator;
impl Operator for MyOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        let value = evaluator.evaluate(&args[0], context)?;
        // ...
    }
}
```

If you're stepping from v3 directly to v5, do the v3→v4 migration first
mentally (the trait shape) and then apply the v4→v5 section above (rename
types, switch to pre-evaluated args, drop `Evaluator`, move to the
builder).

### Getting Help

If you encounter issues during migration:

1. Check the [API Reference](api/reference.md)
2. Review the [examples](https://github.com/GoPlasmatic/datalogic-rs/tree/main/examples)
3. Open an issue on [GitHub](https://github.com/GoPlasmatic/datalogic-rs/issues)
