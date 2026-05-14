# API Reference

Core types and methods in datalogic-rs v5.

## Public surface at a glance

v5 exposes five evaluation tiers, in order of caller control. Pick by
use case, not by curiosity — most callers want **Tier 0** for ad-hoc
work or **Tier 2** for repeated evaluation.

| Tier | Entry point | Arena owner | Returns | Use when |
|------|-------------|-------------|---------|----------|
| **0** | `datalogic_rs::eval_str` / `eval` / `eval_into` / `compile` | lazy static `Engine` | `String` / `OwnedDataValue` / `T` / `Logic` | One-shot scripts, ad-hoc evaluation, no custom config |
| **1** | `Engine::eval_str` / `eval` / `eval_into` | per-call `Bump` | `String` / `OwnedDataValue` / `T` | You need custom operators, config, or templating mode |
| **2** | `Engine::session()` → `Session::eval*` | session-owned `Bump` | owned **or** `&DataValue<'a>` | Hot loops, services, batch jobs |
| **3** | `Engine::evaluate(&Logic, data, &Bump)` | caller-owned `Bump` | `&'a DataValue<'a>` | Zero-copy result pipelines, custom pool strategies |
| **4** | `Engine::trace()` → `TracedSession::*` | session-owned + trace buffer | `TracedRun<R>` | Debugging, visualisation, instrumentation |

The same tier model is exposed in every binding — see each binding's
README for the language-idiomatic entry points.

## Module-level helpers

For the simplest cases, skip the engine entirely:

```rust
let result = datalogic_rs::eval_str(
    r#"{"==": [{"var": "x"}, 1]}"#,
    r#"{"x": 1}"#,
).unwrap();
assert_eq!(result, "true");
```

```rust
pub fn compile<R: IntoLogic>(rule: R) -> Result<Logic>;
pub fn eval<R, D>(rule: R, data: D) -> Result<OwnedDataValue>;
pub fn eval_str<R, D>(rule: R, data: D) -> Result<String>;

#[cfg(feature = "serde_json")]
pub fn eval_into<T, R, D>(rule: R, data: D) -> Result<T>;
```

These delegate to a shared default engine (lazy `OnceLock<Engine>`).
Escalate to a real `Engine` when you need custom operators, a non-default
config, templating, or a long-lived `Session`.

## Engine

The configured engine. Compiles rules and evaluates them.

### Creating an Engine

```rust
use datalogic_rs::{Engine, EvaluationConfig};

// Default engine.
let engine = Engine::new();

// Builder — set config, enable templating, register custom operators.
let engine = Engine::builder()
    .with_config(EvaluationConfig::strict())
    .with_templating(true)           // requires feature = "templating"
    .add_operator("my_op", MyOperator)
    .with_constant_folding(true)     // default; pass false to keep every operator visible in the compiled tree
    .build();
```

> v5 makes operator registration **builder-only**. The `Engine` produced
> by `build()` has a frozen operator set.

### Methods

#### `compile`

Compile a JSONLogic rule into reusable [`Logic`](#logic).

```rust
pub fn compile<R: IntoLogic>(&self, rule: R) -> Result<Logic>;
pub fn compile_arc<R: IntoLogic>(&self, rule: R) -> Result<Arc<Logic>>;
```

`R: IntoLogic` accepts `&str` (JSON-parsed), `&String`,
`&OwnedDataValue` / `OwnedDataValue`, and `&serde_json::Value` (gated
on `feature = "serde_json"`). Use `compile_arc` for the dominant
cross-thread sharing pattern (equivalent to
`Arc::new(engine.compile(rule)?)`).

#### `eval` / `eval_str` / `eval_into` (one-shot)

Engine-owned arena per call. The differences are only in the result
type:

```rust
pub fn eval<R, D>(&self, rule: R, data: D) -> Result<OwnedDataValue>;
pub fn eval_str<R, D>(&self, rule: R, data: D) -> Result<String>;

#[cfg(feature = "serde_json")]
pub fn eval_into<T, R, D>(&self, rule: R, data: D) -> Result<T>;
```

`R: IntoLogic` and `D: OwnedInput` — `data` accepts `&str`, `String`,
`&OwnedDataValue` / `OwnedDataValue`, and `&serde_json::Value` (gated on
`serde_json`). For `eval_into`, `T: DeserializeOwned`; the typical
choices are `serde_json::Value` (JSON-shaped boundary) or your own
domain struct.

```rust
let result = engine.eval_str(
    r#"{"+": [{"var": "x"}, 1]}"#,
    r#"{"x": 41}"#,
)?;
assert_eq!(result, "42");

let value: serde_json::Value = engine.eval_into(
    r#"{"+": [{"var": "x"}, 1]}"#,
    r#"{"x": 41}"#,
)?;
```

#### `evaluate` (raw tier)

Hot-path evaluation against arena-resident data. The caller owns the
`bumpalo::Bump`; the result borrows from it.

```rust
pub fn evaluate<'a, D: EvalInput<'a>>(
    &self,
    compiled: &'a Logic,
    data: D,
    arena: &'a bumpalo::Bump,
) -> Result<&'a DataValue<'a>>;
```

`D` accepts any of: `&'a DataValue<'a>`, `DataValue<'a>`, `&'a str`,
`&OwnedDataValue`, or `&serde_json::Value` (under
`feature = "serde_json"`).

```rust
use bumpalo::Bump;
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
let arena = Bump::new();
let result = engine.evaluate(&compiled, r#"{"x": 1}"#, &arena).unwrap();
assert_eq!(result.as_bool(), Some(true));
```

#### `session`

Open a [`Session`](#session) that owns a reusable arena.

```rust
pub fn session(&self) -> Session<'_>;
```

#### `trace` (feature = "trace")

Open a [`TracedSession`](#tracedsession-feature--trace) that records
execution steps. Mirrors `session()` 1:1 — every `eval*` returns a
`TracedRun<R>` carrying the result, steps, and compile-time expression
tree.

```rust
#[cfg(feature = "trace")]
pub fn trace(&self) -> TracedSession<'_>;
```

#### Introspection helpers

```rust
pub fn config(&self) -> &EvaluationConfig
pub fn has_custom_operator(&self, name: &str) -> bool
pub fn custom_operator_names(&self) -> impl Iterator<Item = &str>
```

---

## EngineBuilder

Fluent constructor for `Engine`. Returned by `Engine::builder()`.

```rust
EngineBuilder::new()
    .with_config(EvaluationConfig::default())
    .with_templating(true)                  // feature = "templating"
    .with_constant_folding(true)            // default; disable to keep every operator visible
    .add_operator("name", MyOp)             // typed operator
    .add_operator("dyn", boxed_op)          // also accepts Box<dyn CustomOperator>
    .build();
```

`with_constant_folding(false)` is useful for tooling that walks the
compiled tree and would be surprised by `{"+": [1, 2]}` collapsing to a
literal `3`. The trace surface always disables folding internally
regardless of this setting.

---

## Logic

The compiled, reusable rule tree. Output of `Engine::compile`.

- `Send + Sync` — wrap in `Arc` to share across threads (or use
  `Engine::compile_arc` to do it in one step).
- Immutable after construction.
- `resolve_node_ids(&self, ids: &[u32]) -> Vec<PathStep>` — translate
  the breadcrumb of a structured `Error` into the source path of the
  failing node.

---

## Session

Reusable evaluation handle that owns a `bumpalo::Bump`. The session
**never** auto-resets — the caller decides when to release arena memory
back to the start-of-chunk position. Construct via `Engine::session()`.

```rust
let mut session = engine.session();
let result_str: String = session.eval_str(&compiled, data_json)?;
let result_owned: datalogic_rs::datavalue::OwnedDataValue =
    session.eval(&compiled, data)?;

#[cfg(feature = "serde_json")]
let value: serde_json::Value = session.eval_into(&compiled, &serde_data)?;

// Zero-copy borrowed result; lives until the next &mut self call.
let view: &datalogic_rs::DataValue<'_> = session.eval_borrowed(&compiled, data)?;

session.reset();                       // bound peak memory between batches
session.reset_with_capacity(64 * 1024);
let bytes = session.allocated_bytes();
```

`Session::eval` / `eval_str` / `eval_into` accept any `EvalInput<'_>`.
`eval_borrowed` returns a `&'a DataValue<'a>` that borrows from the
session's arena — Rust's borrow checker enforces that the next
`&mut self` call invalidates it.

---

## EvalInput

Sealed input adapter trait used by `Engine::evaluate`,
`Session::eval_borrowed`, and the `OwnedInput` cousin used by the owned
entry points.

| Implementor | Cost |
|-------------|------|
| `&'a DataValue<'a>` | Pass-through. |
| `DataValue<'a>` | One arena alloc. |
| `&'a str` | JSON parse via `DataValue::from_str`. |
| `&OwnedDataValue` | Deep-borrow into the arena. |
| `&serde_json::Value` (`feature = "serde_json"`) | Deep-convert into the arena. |

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
heap-allocated owned tree (e.g. as the return of `Engine::eval` /
`Session::eval`).

---

## EvaluationConfig

Configuration for evaluation behavior. All fields are public — set them
with struct update syntax:

```rust
EvaluationConfig {
    arithmetic_nan_handling: NanHandling::ThrowError,
    division_by_zero: DivisionByZeroHandling::ReturnSaturated,
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
    ReturnSaturated,    // default — f64::MAX / MIN
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

> The `Custom` callback receives an `&OwnedDataValue` (not
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
    Custom(CustomErrorSource),
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
  "node_ids": [42, 13, 7],     // present only when non-empty
  // kind-specific extras (variable, level, thrown, index/length, ...)
}
```

Use `error.tag()` for stable string matching, `error.thrown_value()`
for the `Thrown` payload, and `error.resolve_path(&compiled)` to translate
the `node_ids` breadcrumb into source `PathStep`s.

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
Error::custom_message(msg)    // string-only
Error::wrap(err)              // any Error + Send + Sync + 'static
Error::parse_error(msg)
Error::thrown(value)
Error::format_error(msg)
Error::index_out_of_bounds(index, length)
Error::configuration_error(msg)
```

---

## PathStep

Resolved entry returned by `Logic::resolve_node_ids` and
`Error::resolve_path`. Names the operator and child index of a node along
the failing-evaluation path.

---

## Result Type

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

---

## Trace API (feature = "trace")

### TracedSession

Open via `engine.trace()`. Mirrors [`Session`](#session) 1:1 — every
`eval*` returns a [`TracedRun<R>`](#tracedrunr-feature--trace).

```rust
#[cfg(feature = "trace")]
{
    let engine = datalogic_rs::Engine::new();
    let run = engine.trace().eval_str(r#"{"+": [1, 2]}"#, r#"{}"#);
    println!("{}", run.result.unwrap());
    println!("{} steps", run.steps.len());
}
```

The pre-compiled paths inherit whatever shape `Engine::compile` produced
(constant folding can hide some operators). For full coverage on a
single rule, prefer `engine.trace().eval_str(rule, data)` — the
one-shot path compiles internally with folding disabled.

### TracedRun&lt;R&gt; (feature = "trace")

```rust
pub struct TracedRun<R> {
    pub result: Result<R, Error>,        // success and failure share one field
    pub steps: Vec<ExecutionStep>,
    pub expression_tree: ExpressionNode,
}
```

`R` is the same shape that `Session::eval*` would return:
`OwnedDataValue` for `eval`, `String` for `eval_str`, `T` for
`eval_into::<T>`, `&'a DataValue<'a>` for `eval_borrowed`.

```rust
pub struct ExecutionStep { /* per-node entry / result / error */ }
pub struct ExpressionNode { /* compile-time tree shape with stable ids */ }
```

---

## Full Example

```rust
use datalogic_rs::{Engine, EvaluationConfig, NanHandling};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::builder()
        .with_config(EvaluationConfig {
            arithmetic_nan_handling: NanHandling::IgnoreValue,
            ..Default::default()
        })
        .build();

    let compiled = engine.compile_arc(
        r#"{"if": [{">=": [{"var": "score"}, 60]}, "pass", "fail"]}"#,
    )?;

    let mut session = engine.session();
    for score in [85, 45, 60] {
        let r = session.eval_str(&compiled, &format!(r#"{{"score": {}}}"#, score))?;
        println!("{} -> {}", score, r);
        session.reset();
    }

    Ok(())
}
```
