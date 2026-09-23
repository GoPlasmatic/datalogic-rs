# API Reference

Core types and methods in datalogic-rs v5.

## Public surface at a glance

v5 exposes five evaluation tiers, in order of caller control. Most
callers want **Tier 0** for ad-hoc work or **Tier 2** for repeated
evaluation.

| Tier | Entry point | Arena owner | Returns | Use when |
|------|-------------|-------------|---------|----------|
| **0** | `datalogic_rs::eval_str` / `eval` / `eval_into` / `compile` | lazy static `Engine` | `String` / `OwnedDataValue` / `T` / `Logic` | One-shot scripts, ad-hoc evaluation, no custom config |
| **1** | `Engine::eval_str` / `eval` / `eval_into` | per-call `Bump` | `String` / `OwnedDataValue` / `T` | You need custom operators, config, or templating mode |
| **2** | `Engine::session()` → `Session::eval*` | session-owned `Bump` | owned **or** `&DataValue<'a>` | Hot loops, services, batch jobs |
| **3** | `Engine::evaluate(&Logic, data, &Bump)` | caller-owned `Bump` | `&'a DataValue<'a>` | Zero-copy result pipelines, custom pool strategies |
| **4** | `Engine::trace()` → `TracedSession::*` | per-call `Bump` (caller-owned for `eval_borrowed`) + trace buffer | `TracedRun<R>` | Debugging, visualisation, instrumentation |

Every binding exposes the same tier model; see each binding's README
for the language-idiomatic entry points.

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

// Builder: set config, enable templating, register custom operators.
let engine = Engine::builder()
    .with_config(EvaluationConfig::strict())
    .with_templating(true)           // requires feature = "templating"
    .with_template_key_escape('$')   // optional: `{"$type": ...}` emits the key `type`
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

Engine-owned arena per call. They differ only in the result type:

```rust
pub fn eval<R, D>(&self, rule: R, data: D) -> Result<OwnedDataValue>;
pub fn eval_str<R, D>(&self, rule: R, data: D) -> Result<String>;

#[cfg(feature = "serde_json")]
pub fn eval_into<T, R, D>(&self, rule: R, data: D) -> Result<T>;
```

`R: IntoLogic` and `D: OwnedInput`: `data` accepts `&str`, `&String`,
`&OwnedDataValue` / `OwnedDataValue`, and `&serde_json::Value` (gated on
`serde_json`). An owned `String` is not accepted; pass `&s`. For
`eval_into`, `T: DeserializeOwned`; the typical choices are
`serde_json::Value` (JSON-shaped boundary) or your own domain struct.

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
`&'a String`, `&OwnedDataValue`, `&ParsedData` (see
[`ParsedData`](#parseddata)), or `&serde_json::Value` (under
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

#### `truthy`

Apply the engine's configured `TruthyEvaluator` to an arena value. This
is the same coercion `if`, `and`, `or`, `!`, and `!!` use internally, so
custom operators and callers of `evaluate` can decide truthiness the way
the engine would.

```rust
pub fn truthy(&self, value: &DataValue<'_>) -> bool;
```

```rust
let compiled = engine.compile(r#"{"var": "items"}"#)?;
let arena = bumpalo::Bump::new();
let result = engine.evaluate(&compiled, r#"{"items": [1]}"#, &arena)?;
assert!(engine.truthy(result));
```

#### `trace` (feature = "trace")

Open a [`TracedSession`](#tracedsession) that records execution steps.
Every `eval*` on it returns a `TracedRun<R>` carrying the result, steps,
and compile-time expression tree, with the same `R` that `Session` would
return; the inputs differ from `Session` (see the
[Trace API](#trace-api-feature--trace) section).

```rust
#[cfg(feature = "trace")]
pub fn trace(&self) -> TracedSession<'_>;
```

#### Introspection helpers

```rust
pub fn config(&self) -> &EvaluationConfig
pub fn has_custom_operator(&self, name: &str) -> bool
pub fn custom_operator_names(&self) -> impl Iterator<Item = &str>
pub fn builtin_operator_names(&self) -> impl Iterator<Item = &'static str>
```

`builtin_operator_names()` reports every built-in key this build resolves
as an operator: the baseline set plus whichever extension families were
compiled in, including the input aliases `var`, `?:` and `match`. It is
derived from the compiler's own lookup table, so it cannot drift from
dispatch. Together with `custom_operator_names()` it is the engine's full
vocabulary, which is what authoring-side tooling needs under templating
mode, where an unknown key is not an error but echoes back as data.

---

## EngineBuilder

Fluent constructor for `Engine`. Returned by `Engine::builder()`.

```rust
EngineBuilder::new()
    .with_config(EvaluationConfig::default())
    .with_templating(true)                  // feature = "templating"
    .with_template_key_escape('$')          // optional escape for operator-named template keys
    .with_constant_folding(true)            // default; disable to keep every operator visible
    .add_operator("name", MyOp)             // typed operator
    .add_operator("dyn", boxed_op)          // also accepts Box<dyn CustomOperator>
    .build();
```

`with_template_key_escape(prefix)` is unset by default. With it, the
engine strips exactly one leading `prefix` from every template key and
never resolves an escaped key as an operator, so `{"$type": ...}` emits the key
`type` instead of running the `type` operator, and `{"$$type": ...}`
emits a literal `$type`. It recovers the ~60 built-in names (and any
registered custom operator) as output keys. Only meaningful in templating
mode. See
[Structured Objects](../advanced/structured-objects.md#emitting-keys-that-are-operator-names).

`with_constant_folding(false)` is useful for tooling that walks the
compiled tree and would be surprised by `{"+": [1, 2]}` collapsing to a
literal `3`. The trace surface always disables folding internally
regardless of this setting.

---

## Logic

The compiled, reusable rule tree. Output of `Engine::compile`.

- `Send + Sync`: wrap in `Arc` to share across threads (or use
  `Engine::compile_arc` to do it in one step).
- Immutable after construction.
- `resolve_node_ids(&self, ids: &[u32]) -> Vec<PathStep>`: translate
  the breadcrumb of a structured `Error` into the source path of the
  failing node.

---

## Session

Reusable evaluation handle that owns a `bumpalo::Bump`. The session
**never** auto-resets: the caller decides when to release arena memory
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
session's arena; Rust's borrow checker enforces that the next
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
| `&'a String` | JSON parse, same as `&str`. |
| `&OwnedDataValue` | Deep-borrow into the arena. |
| `&'a ParsedData` | Pass-through: the tree is already arena-resident. |
| `&serde_json::Value` (`feature = "serde_json"`) | Deep-convert into the arena. |

The trait is sealed; external crates cannot add new shapes.

### OwnedInput

The owned-entry-point cousin used by `Engine::eval*` and the module-level
helpers, where the engine creates and owns the arena per call. Also
sealed; the supported shapes are `&str` and `&String` (JSON-parsed),
`&OwnedDataValue` (cloned), `OwnedDataValue` (moved), and
`&serde_json::Value` (`feature = "serde_json"`, deep-converted).

### FromDataValue

Sealed result-side counterpart: the `R` in `Session::eval*` and
`TracedSession::eval*` is projected out of the arena through
`FromDataValue::from_arena(&DataValue) -> Result<R>`. Implemented for
`OwnedDataValue` (deep clone), `String` (JSON serialisation), and
`serde_json::Value` (`feature = "serde_json"`). The typed
`eval_into::<T>` paths go through `serde_json::Value` and then
`serde_json::from_value`.

---

## ParsedData

A parse-once data handle: a self-contained JSON document that owns its
own arena. Parse a payload once, then evaluate any number of rules
against it at zero per-call conversion cost (`&ParsedData` implements
`EvalInput`).

```rust
impl ParsedData {
    pub fn from_json(json: &str) -> Result<Self>;   // ParseError on malformed input
    pub fn value(&self) -> &DataValue<'_>;         // borrow the parsed tree
    pub fn allocated_bytes(&self) -> usize;        // input copy + tree
}
```

```rust
use datalogic_rs::{Engine, ParsedData};
use datalogic_rs::bumpalo::Bump;

let engine = Engine::new();
let data = ParsedData::from_json(r#"{"user": {"age": 34}}"#)?;

let adult = engine.compile(r#"{">=": [{"var": "user.age"}, 18]}"#)?;
let senior = engine.compile(r#"{">=": [{"var": "user.age"}, 65]}"#)?;

let arena = Bump::new();
assert_eq!(engine.evaluate(&adult, &data, &arena)?.as_bool(), Some(true));
assert_eq!(engine.evaluate(&senior, &data, &arena)?.as_bool(), Some(false));
```

`ParsedData` is `Send` (move it across threads) but not `Sync`; share
by cloning the source string or parsing once per thread.

---

## DataValue / OwnedDataValue

`DataValue<'a>` is the arena-resident value tree:

```rust
enum DataValue<'a> {
    Null,
    Bool(bool),
    Number(NumberValue),
    String(&'a str),
    Array(&'a [DataValue<'a>]),
    Object(&'a [(&'a str, DataValue<'a>)]),
    DateTime(...),  // feature = "datetime"
    Duration(...),  // feature = "datetime"
}
```

Both `DataValue` and `OwnedDataValue` are re-exported from the
[`datavalue`](https://docs.rs/datavalue-rs) crate. Use `arena.alloc(...)` to
return values from custom operators; use `OwnedDataValue` when you need a
heap-allocated owned tree (e.g. as the return of `Engine::eval` /
`Session::eval`).

---

## EvaluationConfig

Configuration for evaluation behavior. The struct is `#[non_exhaustive]`,
so you cannot build it with a struct literal from outside the crate.
Construct it via `default()` (or a preset) and chain the `with_*` setters:

```rust
// #[non_exhaustive]: fields shown for reference, not for direct literals.
pub struct EvaluationConfig {
    pub arithmetic_nan_handling: NanHandling,        // default: ThrowError
    pub division_by_zero: DivisionByZeroHandling,    // default: ReturnSaturated
    pub loose_equality_errors: bool,                 // default: true
    pub truthy_evaluator: TruthyEvaluator,           // default: JavaScript
    pub numeric_coercion: NumericCoercionConfig,     // default: NumericCoercionConfig::default()
    pub max_recursion_depth: u32,                    // default: 256
    // more fields may be added in 5.x
}

let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::ThrowError)
    .with_division_by_zero(DivisionByZeroHandling::ReturnSaturated)
    .with_loose_equality_errors(true)
    .with_truthy_evaluator(TruthyEvaluator::JavaScript)
    .with_numeric_coercion(NumericCoercionConfig::default())
    .with_max_recursion_depth(256);
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
    ReturnSaturated,    // default: f64::MAX / MIN with the dividend's sign
    ThrowError,
    ReturnNull,
    ReturnInfinity,     // f64::INFINITY as a DataValue; null on the JSON-string paths
}
```

Applies to the float path only: an integer dividend over an integer zero
(`{"/": [10, 0]}`) always raises `Thrown { type: "NaN" }`, whatever the
setting, because there is no in-range integer sentinel. A fractional
dividend (`{"/": [10.5, 0]}`) takes the configured path. See
[Division by Zero](../advanced/configuration.md#division-by-zero).

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

`TruthyEvaluator::custom(f)` wraps a closure without spelling out the
`Arc::new(...)`:

```rust
let config = EvaluationConfig::default().with_truthy_evaluator(
    TruthyEvaluator::custom(|v: &OwnedDataValue| {
        v.as_i64().map(|n| n % 2 == 0).unwrap_or(false)   // even integers are truthy
    }),
);
```

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
| `arena` | Allocator for the current call. Use `arena.alloc(...)` for `DataValue` and `arena.alloc_str(...)` for strings, or the one-call helpers on [`ArenaExt`](#arenaext). |

---

## ArenaExt

Extension trait on `bumpalo::Bump` (bring it into scope with
`use datalogic_rs::ArenaExt;`) that folds "build a `DataValue`, then
allocate it" into one call. It returns static singletons where it can
(`null`, `bool`, small integers, empty strings/arrays/objects), so it is
the recommended way to return values from a custom operator.

```rust
pub trait ArenaExt<'a> {
    fn null(&'a self) -> &'a DataValue<'a>;
    fn bool(&'a self, b: bool) -> &'a DataValue<'a>;
    fn i64(&'a self, n: i64) -> &'a DataValue<'a>;
    fn f64(&'a self, n: f64) -> &'a DataValue<'a>;
    fn string(&'a self, s: &str) -> &'a DataValue<'a>;
    fn array(&'a self, items: &[DataValue<'a>]) -> &'a DataValue<'a>;
    fn object(&'a self, pairs: &[(&'a str, DataValue<'a>)]) -> &'a DataValue<'a>;
}
```

```rust
use datalogic_rs::{ArenaExt, CustomOperator, DataValue, Result};
use datalogic_rs::operator::EvalContext;

struct Triple;
impl CustomOperator for Triple {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.f64(n * 3.0))
    }
}
```

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
#[non_exhaustive]
pub struct Error {
    pub kind: ErrorKind,
    /* private fields: operator, node_ids */
}

// Read the contextual metadata via accessor methods, not fields:
impl Error {
    pub fn operator(&self) -> Option<&str>;  // outermost failing operator, when known
    pub fn node_ids(&self) -> &[u32];         // compiled-node breadcrumb, leaf-to-root
}

// ErrorKind variants carry `Cow<'static, str>` payloads (not `String`).
// The enum is #[non_exhaustive]: a `match` on `error.kind` needs a `_ =>` arm.
#[non_exhaustive]
pub enum ErrorKind {
    InvalidOperator(Cow<'static, str>),
    InvalidArguments(Cow<'static, str>),
    VariableNotFound(Cow<'static, str>),
    InvalidContextLevel(isize),
    TypeError(Cow<'static, str>),
    ArithmeticError(Cow<'static, str>),
    Custom(CustomErrorSource),
    ParseError(Cow<'static, str>),
    Thrown(OwnedDataValue),
    FormatError(Cow<'static, str>),
    IndexOutOfBounds { index: isize, length: usize },
    ConfigurationError(Cow<'static, str>),
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
Error::new(kind)              // from an ErrorKind, no operator / node metadata
Error::invalid_operator(name)
Error::invalid_arguments(msg)
Error::variable_not_found(name)
Error::invalid_context_level(level)
Error::type_error(msg)
Error::arithmetic_error(msg)
Error::custom_message(msg)    // string-only
Error::wrap(err)              // any Error + Send + Sync + 'static
Error::parse_error(msg)
Error::thrown(value)
Error::format_error(msg)
Error::index_out_of_bounds(index, length)
Error::configuration_error(msg)

// Builder-style metadata (the engine sets these itself on errors that
// bubble out of an operator; only needed when constructing errors by hand):
error.with_operator(name)     // impl Into<Cow<'static, str>>
error.with_node_ids(ids)      // Vec<u32>, leaf-to-root
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

Open via `engine.trace()`. Every `eval*` returns a
[`TracedRun<R>`](#tracedrunr-feature--trace) with the same `R` that
[`Session`](#session) would return, but the inputs differ from `Session`:
the one-shot methods take a rule source rather than a compiled `&Logic`,
and there is no session-owned arena. `TracedSession` holds only a
reference to the engine; each owned call allocates a fresh `Bump`, and
`eval_borrowed` uses the caller's.

```rust
impl TracedSession<'_> {
    // Pre-compiled Logic + owned input; fresh per-call arena.
    pub fn eval<D: OwnedInput>(&self, compiled: &Logic, data: D) -> TracedRun<OwnedDataValue>;

    // Rule source (IntoLogic) + owned input; compiles with folding disabled.
    pub fn eval_str<R: IntoLogic, D: OwnedInput>(&self, rule: R, data: D) -> TracedRun<String>;
    #[cfg(feature = "serde_json")]
    pub fn eval_into<T: DeserializeOwned, R: IntoLogic, D: OwnedInput>(&self, rule: R, data: D) -> TracedRun<T>;

    // Pre-compiled Logic + arena input + caller-owned arena; borrowed result.
    pub fn eval_borrowed<'a, D: EvalInput<'a>>(
        &self,
        compiled: &'a Logic,
        data: D,
        arena: &'a bumpalo::Bump,
    ) -> TracedRun<&'a DataValue<'a>>;
}
```

```rust
#[cfg(feature = "trace")]
{
    let engine = datalogic_rs::Engine::new();
    let run = engine.trace().eval_str(r#"{"+": [1, 2]}"#, r#"{}"#);
    println!("{}", run.result.unwrap());
    println!("{} steps", run.steps.len());
}
```

The pre-compiled paths (`eval`, `eval_borrowed`) inherit whatever shape
`Engine::compile` produced (constant folding can hide some operators).
For full coverage on a single rule, prefer `engine.trace().eval_str(rule,
data)` or `eval_into`: the one-shot paths compile internally with folding
disabled.

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
pub struct ExecutionStep {
    pub step_id: u32,                  // recording order
    pub node_id: u32,                  // which compiled node ran
    pub context: serde_json::Value,    // scope data at this step
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub iteration_index: Option<u32>,  // iterator bodies only
    pub iteration_total: Option<u32>,
}

pub struct ExpressionNode {
    pub id: u32,                       // matches ExecutionStep::node_id
    pub expression: String,            // JSON text of this sub-expression
    pub children: Vec<ExpressionNode>,
}
```

Steps carry no timing data; they record which nodes ran, in what order,
and with what context and result.

---

## Full Example

```rust
use datalogic_rs::{Engine, EvaluationConfig, NanHandling};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::builder()
        .with_config(
            EvaluationConfig::default()
                .with_arithmetic_nan_handling(NanHandling::IgnoreValue),
        )
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
