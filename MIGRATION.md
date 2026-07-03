# Migrating to datalogic-rs v5

v5 is a clean break from v4. There is **no `compat` feature**, no
`LegacyApi` trait, and no deprecated method shims inside the v5 crate —
v4 callers rewrite their call sites following this guide. The
rewrites are mechanical and 1:1; this document is the authoritative
cookbook.

If you are still on v4 and not ready to migrate, stay on the latest
4.x release. v5 does not support a side-by-side mode.

## v4 → v5 in 60 seconds

Most call-site changes are mechanical 1:1 renames. The deep-dive is
below; this checklist covers the 90% case.

**Rust callers**

- Cargo feature: `compat` → `serde_json` ([details](#cargotoml))
- Engine construction is builder-only:
  `Engine::with_config(c)` → `Engine::builder().with_config(c).build()`
  ([details](#engine-construction))
- Templating: `with_preserve_structure()` →
  `builder().with_templating(true).build()`
  ([details](#engine-construction))
- One-shot: `engine.evaluate_json(rule, data) -> Value` →
  `engine.eval_str(rule, data) -> String` (JSON in/out) or
  `engine.eval_into::<T>(rule, data)` (typed)
  ([details](#evaluation))
- Compile from `&Value`: `engine.compile_serde_value(&v)` →
  `engine.compile(&v)` ([details](#compilation))
- Custom operators: `ArenaOperator` → `CustomOperator`,
  `&mut ContextStack` → `&mut EvalContext`
  ([details](#custom-operators))
- Trace: `engine.evaluate_json_with_trace(...)` →
  `engine.trace().eval_str(...)` returning `TracedRun<R>`
  ([details](#trace))

**JS / TS callers**

- npm install: `@goplasmatic/datalogic` →
  `@goplasmatic/datalogic-wasm` (browser/edge) or
  `@goplasmatic/datalogic-node` (new in v5, Node-native via napi-rs)
  ([details](#npm-package-rename-jsts-consumers-only))
- Templating flag rename: `preserve_structure` → `templating` — same
  semantics ([details](#javascript--npm-consumers))

If a v4 surface isn't covered here, search this document or
[file an issue](#if-you-get-stuck).

## Contents

- [npm package rename (JS/TS consumers only)](#npm-package-rename-jsts-consumers-only)
- [What changed at a glance](#what-changed-at-a-glance)
- [Cargo.toml](#cargotoml)
- [Method-by-method translation](#method-by-method-translation)
  - [Engine construction](#engine-construction)
  - [Compilation](#compilation)
  - [Evaluation](#evaluation)
  - [Trace](#trace)
  - [Custom operators](#custom-operators)
- [New v5 capabilities (not in v4)](#new-v5-capabilities-not-in-v4)
- [Common patterns side-by-side](#common-patterns-side-by-side)
- [Recipe: structural-error consumers](#recipe-structural-error-consumers)
- [JavaScript / npm consumers](#javascript--npm-consumers)
- [Things that did NOT change](#things-that-did-not-change)
- [If you get stuck](#if-you-get-stuck)

## npm package rename (JS/TS consumers only)

The WASM npm package was renamed to align with the `datalogic-<lang>`
convention used by every other binding:

| v4 | v5 |
|---|---|
| `@goplasmatic/datalogic` (WASM) | **`@goplasmatic/datalogic-wasm`** |
| _(new in v5)_ | **`@goplasmatic/datalogic-node`** — native Node.js binding via napi-rs |
| `@goplasmatic/datalogic-ui` | `@goplasmatic/datalogic-ui` (unchanged) |

If you are a JS consumer:

- **Browsers, Deno, Bun, Cloudflare Workers** → switch
  `npm install @goplasmatic/datalogic` to `npm install @goplasmatic/datalogic-wasm`.
- **Node.js services** → install `@goplasmatic/datalogic-node` instead;
  it's the new native build (per-platform `.node` prebuild) and is
  materially faster than the WASM path for Node. The WASM package
  still works under Node if you'd rather have a single artifact across
  Node + browser.
- **React UI consumers** → no change. `@goplasmatic/datalogic-ui`
  bundles its own WASM internally.

The legacy `@goplasmatic/datalogic` name is not republished at v5; v4.x
versions remain installable for consumers not ready to move.

## What changed at a glance

| Concern                     | v4                                                   | v5                                                              |
|-----------------------------|------------------------------------------------------|-----------------------------------------------------------------|
| Feature flag for serde_json | `compat`                                             | `serde_json`                                                    |
| One-shot evaluation         | `engine.evaluate_json(rule, data) -> Value`          | `engine.eval_str(rule, data) -> String` *or* `eval_into::<T>`    |
| Value-boundary evaluation   | `engine.evaluate_owned(&logic, value) -> Value`      | `engine.eval_into::<serde_json::Value, _, _>(rule, &value) -> Value` |
| Compile from `&Value`       | `engine.compile_serde_value(&value)`                 | `engine.compile(&value)` *(via `IntoLogic`, requires `serde_json`)* |
| Construct with config       | `Engine::with_config(cfg)`                           | `Engine::builder().with_config(cfg).build()`                    |
| Templating                  | `Engine::with_preserve_structure()`                  | `Engine::builder().with_templating(true).build()`               |
| Trace one-shot              | `engine.evaluate_json_with_trace(rule, data)`        | `engine.trace().eval_str(rule, data) -> TracedRun<String>`      |
| Custom-op context type      | `&mut ContextStack<'a>`                              | `&mut EvalContext<'_, 'a>`                                      |
| `Arc<Logic>` shortcut       | (manual `Arc::new(...)`)                             | `engine.compile_arc(rule) -> Arc<Logic>`                        |
| Module-level conveniences   | none                                                 | `datalogic::eval` / `eval_str` / `eval_into` / `compile`        |

## Cargo.toml

Rename the feature you depend on:

```toml
# v4
[dependencies]
datalogic-rs = { version = "4", features = ["compat"] }

# v5
[dependencies]
datalogic-rs = { version = "5", features = ["serde_json"] }
```

If you only used the JSONLogic baseline (no `serde_json::Value`,
no typed serde input/output), drop the feature entirely:

```toml
datalogic-rs = "5"
```

The v4 `wasm` feature (JS-host clock for `now` when targeting
`wasm32-unknown-unknown`) is called `wasm-clock` in v5, and the `now`
operator itself now also needs `datetime`:

```toml
# v4
datalogic-rs = { version = "4", features = ["wasm"] }

# v5
datalogic-rs = { version = "5", features = ["datetime", "wasm-clock"] }
```

As in v4, leave it off when the module runs in a non-JS wasm runtime
(wasmtime, wazero, Chicory): the JS clock imports would fail to
instantiate there
([#47](https://github.com/GoPlasmatic/datalogic-rs/issues/47)).

## Method-by-method translation

### Engine construction

| v4                                            | v5                                                            |
|-----------------------------------------------|---------------------------------------------------------------|
| `Engine::new()`                               | `Engine::new()` *(unchanged)*                                 |
| `Engine::with_config(cfg)`                    | `Engine::builder().with_config(cfg).build()`                  |
| `Engine::with_preserve_structure()`           | `Engine::builder().with_templating(true).build()`             |
| `Engine::with_config_and_structure(c, s)`     | `Engine::builder().with_config(c).with_templating(s).build()` |

### Compilation

| v4                                       | v5                                                                |
|------------------------------------------|-------------------------------------------------------------------|
| `engine.compile(&Value)`                 | `engine.compile(&value)` *(`IntoLogic` accepts the same shape)*   |
| `engine.compile_serde_value(&Value)`     | `engine.compile(&value)` *(collapsed)*                            |
| `Arc::new(engine.compile(...)?)`         | `engine.compile_arc(...)?`                                        |

`compile` now accepts `&str`, `&String`, `&OwnedDataValue`,
`OwnedDataValue`, and `&serde_json::Value`. A typed `&T: Serialize`
input goes via `serde_json::to_value(&t)?` first.

### Evaluation

| v4                                                      | v5                                                                                          |
|---------------------------------------------------------|---------------------------------------------------------------------------------------------|
| `engine.evaluate(&logic, Arc<Value>)` → `Value`         | `engine.session().eval_into::<serde_json::Value, _, _>(&compiled, &*arc)` (compiled logic evaluates through a session) |
| `engine.evaluate_owned(&logic, value)`                  | `let v: serde_json::Value = engine.session().eval_into(&compiled, &value)?`                 |
| `engine.evaluate_json(rule_str, data_str)` → `Value`    | `let v: serde_json::Value = engine.eval_into(rule_str, data_str)?` *or* `engine.eval_str(...)` for String result |

The v5 `eval_into::<T>` has three generic parameters (`T`, `R`, `D`).
You can either annotate the binding (`let v: T = ...`) and let
inference fill in `R`/`D`, or use turbofish placeholders:
`engine.eval_into::<T, _, _>(rule, data)`.

### Trace

| v4                                                            | v5                                                              |
|---------------------------------------------------------------|-----------------------------------------------------------------|
| `engine.evaluate_json_with_trace(rule, data) -> TracedResult` | `engine.trace().eval_str(rule, data) -> TracedRun<String>` — the outer Result collapses into `TracedRun.result` |

`TracedResult` is gone. `TracedRun<R>` shape:

```rust
pub struct TracedRun<R> {
    pub result: Result<R, Error>,        // success and failure share one field
    pub steps: Vec<ExecutionStep>,
    pub expression_tree: ExpressionNode,
}
```

Migration of field accesses:

```rust
// v4
let result = engine.evaluate_json_with_trace(rule, data).unwrap();
assert!(result.error.is_none());
assert_eq!(result.result, json!(true));

// v5 — string result
let run = engine.trace().eval_str(rule, data);
assert_eq!(run.result.unwrap(), "true");

// v5 — typed value result
let run: TracedRun<serde_json::Value> = engine.trace().eval_into(rule, data);
assert_eq!(run.result.unwrap(), json!(true));

// v5 — error case
let run = engine.trace().eval_str(bad_rule, data);
let err = run.result.unwrap_err();
assert_eq!(err.operator(), Some("throw"));
```

### Custom operators

The trait stays — the **method body** is unchanged; only the context
parameter type changes:

```rust
// v4
use datalogic_rs::compat::{ArenaOperator, ArenaContextStack};

impl ArenaOperator for Double {
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut ArenaContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args[0].as_f64().unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}

// v5
use datalogic_rs::{CustomOperator, operator::EvalContext};

impl CustomOperator for Double {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args[0].as_f64().unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}
```

`EvalContext` exposes the same observations that were public on
`ContextStack` (`root_input`, `depth`). Code that reached into
`ContextStack`'s private internals was already unsupported and has no
v5 path — open an issue if you have a use case.

Registration is unchanged: `Engine::builder().add_operator("double", Double).build()`.

## New v5 capabilities (not in v4)

You don't need these to migrate, but they're worth knowing:

- **Module-level helpers.** `datalogic::eval`, `eval_str`, `eval_into`,
  `compile` — backed by a default engine, no construction required.
- **Owned and typed result paths.** `engine.eval(...) -> OwnedDataValue`
  for raw owned, `engine.eval_into::<MyStruct, _, _>(...)` for typed.
- **`compile_arc`** for the cross-thread sharing pattern.
- **`with_constant_folding(false)`** on the builder for callers that
  walk the compiled tree (debuggers, alternate evaluators).
- **`TracedSession` mirrors `Session` 1:1** — `eval`, `eval_str`,
  `eval_into`, `eval_borrowed` all return `TracedRun<R>`.

## Common patterns side-by-side

### One-shot evaluation

```rust
// v4
let engine = Engine::new();
let result: serde_json::Value = engine.evaluate_json(rule, data)?;

// v5 — JSON in/out
let result: String = datalogic_rs::eval_str(rule, data)?;

// v5 — typed in/out
#[derive(Deserialize)]
struct Decision { passed: bool }
let result: Decision = datalogic_rs::eval_into(rule, data)?;
```

### Compile once, evaluate many

```rust
// v4
let engine = Engine::new();
let compiled = engine.compile(&rule_value)?;     // Arc<Logic>
for record in stream {
    let r = engine.evaluate_owned(&compiled, record)?;
    // ...
}

// v5
let engine = Engine::new();
let compiled = engine.compile(rule_str)?;        // Logic
let mut session = engine.session();
for record in stream {
    let r: serde_json::Value =
        session.eval_into(&compiled, &record)?;
    session.reset();
    // ...
}
```

### Cross-thread sharing

```rust
// v4
let engine = Engine::new();
let compiled = engine.compile(&rule)?;           // already Arc<Logic>
let c2 = Arc::clone(&compiled);
std::thread::spawn(move || { /* use c2 */ });

// v5
let engine = Engine::new();
let compiled = engine.compile_arc(rule_str)?;    // Arc<Logic>
let c2 = Arc::clone(&compiled);
std::thread::spawn(move || { /* use c2 */ });
```

### Hot loop with zero-copy results

```rust
// v5 only — v4 had no exposed arena tier
use bumpalo::Bump;

let engine = Engine::new();
let compiled = engine.compile(rule_str)?;
let mut arena = Bump::new();
for input in stream {
    let result = engine.evaluate(&compiled, input, &arena)?;
    // ... use `result: &DataValue<'_>` while `arena` is alive
    arena.reset();
}
```

## Recipe: structural-error consumers

`Error` already carried `operator`/`path` in late 4.x; v5 makes that
the only path:

```rust
// v5
match engine.eval_str(rule, data) {
    Ok(json) => println!("ok: {json}"),
    Err(e) => {
        eprintln!("op: {:?}", e.operator());
        eprintln!("kind: {:?}", &e.kind);
        // For a JSONLogic-style path of node ids:
        eprintln!("node ids (leaf→root): {:?}", e.node_ids());
        // Resolve to structured PathSteps (root→leaf):
        if let Ok(compiled) = engine.compile(rule) {
            eprintln!("path: {:#?}", e.resolve_path(&compiled));
        }
    }
}
```

## JavaScript / npm consumers

The `@goplasmatic/datalogic-wasm` (WASM) and `@goplasmatic/datalogic-ui`
(React) packages share the v5 cutover. Two surface renames mirror the
Rust core:

| v4 JS surface                                | v5 JS surface                                |
|----------------------------------------------|----------------------------------------------|
| `evaluate(logic, data, preserve_structure)`  | `evaluate(logic, data, templating)`          |
| `new CompiledRule(logic, preserve_structure)`| `new CompiledRule(logic, templating)`        |
| `<DataLogicEditor preserveStructure={…} />`  | `<DataLogicEditor templating={…} />`         |
| `onPreserveStructureChange={…}`              | `onTemplatingChange={…}`                     |

The semantic of the flag is unchanged — `true` enables templating mode
where multi-key objects compile to output-shaping templates with
embedded JSONLogic.

## Things that did NOT change

- Operator semantics (every JSONLogic operator behaves the same).
- `EvaluationConfig` field set, defaults, presets (`safe_arithmetic`,
  `strict`).
- `DataValue` / `OwnedDataValue` shape and accessors.
- `Error::kind()` variants, error-recovery behaviour, structured
  error fields.
- `PathStep` shape.
- The two-phase compile/evaluate model and arena-allocated dispatch.

## If you get stuck

- The renames are 1:1 — search-and-replace covers ~90% of call sites.
- For typed `eval_into`, prefer annotating the result binding
  (`let result: MyType = engine.eval_into(rule, data)?`) over
  turbofishing all three generic parameters.
- If a v4 method shape has no listed translation, file an issue —
  we'll add it here.
