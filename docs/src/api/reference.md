# API Reference

Core types and methods in datalogic-rs v5.

## Engine

The main engine for compiling and evaluating JSONLogic rules.

### Creating an Engine

```rust
use datalogic_rs::{Engine, EvaluationConfig};

// Default engine.
let engine = Engine::new();

// Builder — set config, structure-preservation, register custom operators.
let engine = Engine::builder()
    .config(EvaluationConfig::strict())
    .preserve_structure(true)        // requires feature = "preserve"
    .add_operator("my_op", MyOperator)
    .build();
```

> v5 makes operator registration **builder-only**. The `Engine` produced by
> `build()` has a frozen operator set. The v4
> `with_config` / `with_preserve_structure` / `with_config_and_structure`
> constructors live on the [`compat::LegacyApi`](../migration.md#legacyapi-and-the-compat-feature)
> trait.

### Methods

#### `compile`

Compile a JSONLogic rule string into a reusable `Logic`.

```rust
pub fn compile(&self, logic: &str) -> Result<Logic>
```

The returned `Logic` is owned. Wrap in `Arc` to share across threads.

#### `evaluate`

Hot-path evaluation against arena-resident data. The caller owns the
`bumpalo::Bump` and the result borrows from it.

```rust
pub fn evaluate<'a, D: EvalInput<'a>>(
    &self,
    compiled: &'a Logic,
    data: D,
    arena: &'a bumpalo::Bump,
) -> Result<&'a DataValue<'a>>
```

`D` accepts any of: `&'a DataValue<'a>`, `DataValue<'a>`, `&'a str`,
`&OwnedDataValue`, or `&serde_json::Value` (under `feature = "compat"`).

```rust
use bumpalo::Bump;
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
let arena = Bump::new();
let result = engine.evaluate(&compiled, r#"{"x": 1}"#, &arena).unwrap();
assert_eq!(result.as_bool(), Some(true));
```

#### `evaluate_str`

One-shot string-in / string-out evaluation. Allocates a fresh
`bumpalo::Bump` internally.

```rust
pub fn evaluate_str(&self, logic: &str, data: &str) -> Result<String>
```

#### `evaluate_serde` (feature = "compat")

`serde_json::Value` boundary on both sides, mirroring `evaluate_str`.

```rust
#[cfg(feature = "compat")]
pub fn evaluate_serde(&self, logic: &serde_json::Value, data: &serde_json::Value) -> Result<serde_json::Value>
```

#### `session`

Open a `Session` that owns a reusable arena.

```rust
pub fn session(&self) -> Session<'_>
```

#### `with_trace` (feature = "trace")

Open a `TracedSession` whose `evaluate*` calls record execution steps.

```rust
#[cfg(feature = "trace")]
pub fn with_trace(&self) -> TracedSession<'_>
```

#### Introspection helpers

```rust
pub fn config(&self) -> &EvaluationConfig
pub fn preserve_structure(&self) -> bool
pub fn has_custom_operator(&self, name: &str) -> bool
pub fn operator_names(&self) -> impl Iterator<Item = &str>
```

---

## EngineBuilder

Fluent constructor for `Engine`. Returned by `Engine::builder()`.

```rust
EngineBuilder::new()
    .config(EvaluationConfig::default())
    .preserve_structure(true)              // feature = "preserve"
    .add_operator("name", MyOp)
    .add_operator_boxed("dyn", boxed_op)   // when you already have Box<dyn CustomOperator>
    .remove_operator("unwanted")
    .build();
```

---

## Logic

The compiled, reusable rule tree. Output of `Engine::compile`.

- `Send + Sync` — wrap in `Arc` to share across threads.
- Immutable after construction.
- `resolve_path(&self, path: &[u32]) -> Vec<PathStep>` — translate the
  breadcrumb of a structured `Error` into the source path of the failing
  node.

---

## Session

Reusable evaluation handle that owns a `bumpalo::Bump` and resets it
between calls. Construct via `Engine::session()`.

```rust
let mut session = engine.session();
let result_str: String = session.evaluate_str(&compiled, data_json)?;
let result_owned: datalogic_rs::datavalue::OwnedDataValue = session.evaluate(&compiled, data)?;

#[cfg(feature = "compat")]
let result_value: serde_json::Value = session.evaluate_serde(&compiled, &serde_data)?;
```

`Session::evaluate` accepts any `EvalInput<'_>`.

---

## EvalInput

Sealed input adapter trait used by `Engine::evaluate` and `Session::evaluate`.

| Implementor | Cost |
|-------------|------|
| `&'a DataValue<'a>` | Pass-through. |
| `DataValue<'a>` | One arena alloc. |
| `&'a str` | JSON parse via `DataValue::from_str`. |
| `&OwnedDataValue` | Deep-borrow into the arena. |
| `&serde_json::Value` (`compat`) | Deep-convert into the arena. |

The trait is sealed — external crates cannot add new shapes.

---

## DataValue / OwnedDataValue

`DataValue<'a>` is the arena-resident value tree:

```rust
enum DataValue<'a> {
    Null,
    Bool(bool),
    Number(NumberRepr),
    String(&'a str),
    Array(&'a [DataValue<'a>]),
    Object(&'a [(&'a str, DataValue<'a>)]),
    DateTime(...),  // feature = "datetime"
    Duration(...),  // feature = "datetime"
    InputRef(...),  // borrow-through into caller input
}
```

Both `DataValue` and `OwnedDataValue` are re-exported from the
[`datavalue`](https://docs.rs/datavalue-rs) crate. Use `arena.alloc(...)` to
return values from custom operators; use `OwnedDataValue` when you need a
heap-allocated owned tree (e.g. as `Session::evaluate`'s return).

---

## EvaluationConfig

Configuration for evaluation behavior. All fields are public — set them
with struct update syntax:

```rust
EvaluationConfig {
    arithmetic_nan_handling: NanHandling::ThrowError,
    division_by_zero: DivisionByZeroHandling::ReturnBounds,
    loose_equality_errors: true,
    truthy_evaluator: TruthyEvaluator::JavaScript,
    numeric_coercion: NumericCoercionConfig::default(),
}
```

Presets:

```rust
EvaluationConfig::default();
EvaluationConfig::safe_arithmetic();
EvaluationConfig::strict();
```

### NanHandling

```rust
pub enum NanHandling {
    ThrowError,    // default
    IgnoreValue,
    CoerceToZero,
    ReturnNull,
}
```

### DivisionByZeroHandling

```rust
pub enum DivisionByZeroHandling {
    ReturnBounds,    // default — f64::MAX / MIN
    ThrowError,
    ReturnNull,
    ReturnInfinity,
}
```

### TruthyEvaluator

```rust
pub enum TruthyEvaluator {
    JavaScript,    // default
    Python,
    StrictBoolean,
    Custom(Arc<dyn Fn(&OwnedDataValue) -> bool + Send + Sync>),
}
```

> **v5 change:** the `Custom` callback receives an `&OwnedDataValue` (not
> `&serde_json::Value`).

---

## CustomOperator Trait

```rust
pub trait CustomOperator: Send + Sync {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        ctx: &mut operator::EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>>;
}
```

| Parameter | Notes |
|-----------|-------|
| `args` | **Pre-evaluated** arguments. The engine has already recursed into each arg's expression tree. |
| `ctx` | Opaque view into the engine's evaluation context. Untouched by most operators. |
| `arena` | Allocator for the current call. Use `arena.alloc(...)` for `DataValue` and `arena.alloc_str(...)` for strings. |

> **v4 → v5:** the trait was renamed from `Operator` to `CustomOperator`.
> Args are now pre-evaluated and arena-resident; the `Evaluator` trait was
> removed. There is no longer a value-mode entry point.

---

## EvalContext

`operator::EvalContext<'_, 'a>` is an opaque view into the engine's
evaluation context, passed to `CustomOperator::evaluate`. Most custom
operators don't need to inspect it; the read-only accessors
`root_input()` (the input passed to `Engine::evaluate`) and `depth()`
(number of iteration frames currently pushed) cover the rare cases where
behaviour depends on the surrounding context. The internal stack layout
is hidden so it can evolve without breaking the trait contract.

---

## Error

Structured error type:

```rust
pub struct Error {
    pub kind: ErrorKind,
    pub operator: Option<String>,
    pub path: Vec<u32>,
}

pub enum ErrorKind {
    InvalidOperator(String),
    InvalidArguments(String),
    VariableNotFound(String),
    InvalidContextLevel(isize),
    TypeError(String),
    ArithmeticError(String),
    Custom(CustomSource),
    ParseError(String),
    Thrown(OwnedDataValue),
    FormatError(String),
    IndexOutOfBounds { index: isize, length: usize },
    ConfigurationError(String),
}
```

`Error` serialises (with serde) to:

```json
{
  "type": "<KindTag>",
  "message": "<Display>",
  "operator": "<name>",        // present only when known
  "path": [42, 13, 7],         // present only when non-empty
  // kind-specific extras (variable, level, thrown, index/length, ...)
}
```

Use `error.kind_tag()` for stable string matching, `error.thrown_value()`
for the `Thrown` payload, and `error.resolved_path(&compiled)` to translate
the path breadcrumb into source `PathStep`s.

To wrap a foreign `std::error::Error` into a `Custom` error:

```rust
"abc".parse::<i32>().map_err(datalogic_rs::Error::wrap)?;
```

`Error::source()` walks the inner chain unchanged.

### Error Constructors

```rust
Error::invalid_operator(name)
Error::invalid_arguments(msg)
Error::variable_not_found(name)
Error::type_error(msg)
Error::arithmetic_error(msg)
Error::custom(msg)            // string-only
Error::wrap(err)              // any Error + Send + Sync + 'static
Error::parse_error(msg)
Error::thrown(value)
Error::format_error(msg)
Error::index_out_of_bounds(index, length)
Error::configuration_error(msg)
```

---

## PathStep

Resolved entry returned by `Logic::resolve_path` and
`Error::resolved_path`. Names the operator and child index of a node along
the failing-evaluation path.

---

## Result Type

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

---

## Trace API (feature = "trace")

```rust
pub struct TracedRun {
    pub result: Result<String>,
    pub steps: Vec<ExecutionStep>,
    pub expression_tree: ExpressionNode,
}

pub struct ExecutionStep { /* per-node entry / result / error */ }
pub struct ExpressionNode { /* compile-time tree shape with stable ids */ }
```

Open a traced session and call `evaluate_str` (or `evaluate` against a
pre-compiled `Logic`):

```rust
#[cfg(feature = "trace")]
{
    let engine = datalogic_rs::Engine::new();
    let run = engine.with_trace().evaluate_str(r#"{"+": [1, 2]}"#, r#"{}"#);
    println!("{}", run.result.unwrap());
    println!("{} steps", run.steps.len());
}
```

> The pre-compiled paths inherit whatever shape `Engine::compile` produced
> (constant folding can hide some operators). For full coverage on a
> single rule, prefer `with_trace().evaluate_str(rule, data)`.

The deprecated `TracedResult` (returned by the v4 `evaluate_json_with_trace`
shim) lives behind `compat`.

---

## Full Example

```rust
use datalogic_rs::{Engine, EvaluationConfig, NanHandling};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Arc::new(
        Engine::builder()
            .config(EvaluationConfig {
                arithmetic_nan_handling: NanHandling::IgnoreValue,
                ..Default::default()
            })
            .build(),
    );

    let compiled = Arc::new(engine.compile(
        r#"{"if": [{">=": [{"var": "score"}, 60]}, "pass", "fail"]}"#,
    )?);

    let mut session = engine.session();
    for score in [85, 45, 60] {
        let r = session.evaluate_str(&compiled, &format!(r#"{{"score": {}}}"#, score))?;
        println!("{} -> {}", score, r);
    }

    Ok(())
}
```
