# API Reference

Core types and methods of the `datalogic-rs` crate, 5.8.1. For every item
and its feature badge, see [docs.rs/datalogic-rs](https://docs.rs/datalogic-rs).

## Public surface at a glance

The crate exposes five evaluation tiers, in order of caller control. Most
callers want **Tier 0** for ad-hoc work or **Tier 2** for repeated
evaluation.

| Tier | Entry point | Arena owner | Returns | Use when |
|------|-------------|-------------|---------|----------|
| **0** | `datalogic_rs::eval_str` / `eval` / `eval_into` / `compile` | lazy static `Engine` | `String` / `OwnedDataValue` / `T` / `Logic` | One-shot scripts, ad-hoc evaluation, no custom config |
| **1** | `Engine::eval_str` / `eval` / `eval_into` / `eval_as` | per-call `Bump` | `String` / `OwnedDataValue` / `T` | You need custom operators, config, or templating mode |
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
Build your own `Engine` when you need custom operators, a non-default
config, templating, or a long-lived `Session`.

## Engine

The configured engine. Compiles rules and evaluates them. `Engine` is
`Send + Sync`; share one through an `Arc`.

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

Operator registration is builder-only: the `Engine` that `build()`
returns has a fixed operator set. See [EngineBuilder](#enginebuilder).

### Methods

#### `compile`

Compile a JSONLogic rule into reusable [`Logic`](#logic).

```rust
pub fn compile<R: IntoLogic>(&self, rule: R) -> Result<Logic>;
pub fn compile_arc<R: IntoLogic>(&self, rule: R) -> Result<Arc<Logic>>;
pub fn compile_strict<R: IntoLogic>(&self, rule: R) -> Result<Logic>;
#[cfg(feature = "templating")]
pub fn compile_template<R: IntoLogic>(&self, rule: R) -> Result<Logic>;
```

`compile` uses the engine's templating mode. `compile_strict` and
`compile_template` choose the mode for one compile instead, with the
engine's custom operators, template key escape and folding setting, so
one engine can check a rule strictly and compile an output template.
`compile_strict` exists in every build; on an engine without templating
it is `compile`.

`R: IntoLogic` accepts `&str` (JSON-parsed), `&String`,
`&OwnedDataValue` / `OwnedDataValue`, and `&serde_json::Value` (gated
on `feature = "serde_json"`). `compile_arc` is
`Arc::new(engine.compile(rule)?)` in one call, for sharing a rule across
threads.

#### `check` / `compile_checked`

```rust
pub fn check<R: IntoLogic>(&self, rule: R, mode: CheckMode) -> Vec<Diagnostic>;
pub fn compile_checked<R: IntoLogic>(&self, rule: R) -> std::result::Result<Logic, CompileError>;
```

`check` reports every problem the engine can see in a rule before it
runs, each located by a JSON Pointer. `compile_checked` compiles a rule
only when `check` finds no error, and otherwise returns a `CompileError`
listing every diagnostic. See [Rule Analysis](../advanced/rule-analysis.md#checking-a-rule-enginecheck).

#### `eval` / `eval_str` / `eval_into` / `eval_as` (one-shot)

Engine-owned arena per call. They differ only in the result type:

```rust
pub fn eval<R, D>(&self, rule: R, data: D) -> Result<OwnedDataValue>;
pub fn eval_str<R, D>(&self, rule: R, data: D) -> Result<String>;
pub fn eval_as<O: FromDataValue, R, D>(&self, rule: R, data: D) -> Result<O>;

#[cfg(feature = "serde_json")]
pub fn eval_into<T, R, D>(&self, rule: R, data: D) -> Result<T>;
```

`R: IntoLogic` and `D: OwnedInput`: `data` accepts `&str`, `&String`,
`&OwnedDataValue` / `OwnedDataValue`, `&serde_json::Value` (gated on
`serde_json`), and [`Roots`](#roots). An owned `String` is not accepted;
pass `&s`. For `eval_into`, `T: DeserializeOwned`; the typical choices
are `serde_json::Value` (JSON-shaped boundary) or your own domain struct.
`eval_as::<serde_json::Value>` converts the result in one step where
`eval_into::<serde_json::Value>` builds a value and deserialises it again.

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
[`ParsedData`](#parseddata)), `&serde_json::Value` (under
`feature = "serde_json"`), or [`Roots`](#roots).

```rust
use bumpalo::Bump;
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
let arena = Bump::new();
let result = engine.evaluate(&compiled, r#"{"x": 1}"#, &arena).unwrap();
assert_eq!(result.as_bool(), Some(true));
```

With `feature = "budget"`, `evaluate_metered(&compiled, data, &arena, budget)`
runs the same evaluation under an explicit operation budget and returns a
`Metered { value, ops }`. See
[Operation Budget](../advanced/operation-budget.md).

#### `session`

Open a [`Session`](#session) that owns a reusable arena.

```rust
pub fn session(&self) -> Session<'_>;
```

For a session that holds the engine by `Arc` instead of borrowing it, use
[`SharedSession`](#sharedsession).

#### `truthy`

Apply the engine's configured `TruthyEvaluator` to an arena value. `if`,
`and`, `or`, `!` and `!!` apply the same coercion, so custom operators
and callers of `evaluate` can decide truthiness the way the engine would.

```rust
pub fn truthy(&self, value: &DataValue<'_>) -> bool;
```

```rust
let compiled = engine.compile(r#"{"var": "items"}"#)?;
let arena = bumpalo::Bump::new();
let result = engine.evaluate(&compiled, r#"{"items": [1]}"#, &arena)?;
assert!(engine.truthy(result));
```

#### `truthy_of`

The same rules for a value in whatever representation the host holds:
`&serde_json::Value`, `&OwnedDataValue`, `&ParsedData` or `&DataValue`
(the sealed `TruthyInput` trait). Use it on an evaluated result instead
of re-implementing truthiness: it follows the configured evaluator, and
under the default rules an empty object is falsy, like an empty array.

```rust
pub fn truthy_of<V: TruthyInput>(&self, value: V) -> bool;
```

```rust
let result: serde_json::Value = session.eval_into(&compiled, &context)?;
if !engine.truthy_of(&result) {
    return Err(rejected());
}
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

#### `to_builder`

```rust
pub fn to_builder(&self) -> EngineBuilder;
```

A builder holding this engine's custom operators, config, templating
mode, template key escape, folding setting and families. Change what
differs and build a new engine; both share the operator instances. See
[Custom Operators: rebuilding an engine](../advanced/custom-operators.md#rebuilding-an-engine).

#### Introspection helpers

```rust
pub fn config(&self) -> &EvaluationConfig
pub fn has_custom_operator(&self, name: &str) -> bool
pub fn custom_operator_names(&self) -> impl Iterator<Item = &str>
pub fn custom_operator_info(&self, name: &str) -> Option<CustomOperatorInfo>
pub fn builtin_operator_names(&self) -> impl Iterator<Item = &'static str>
pub fn operators(&self) -> impl Iterator<Item = OperatorInfo>
```

`operators()` describes each built-in operator as its table row declares
it: canonical name and aliases, family and gating feature, argument
counts, whether it reads the data context, its effect, its cost class,
and which argument (if any) runs under a pushed frame. See
[Rule Analysis](../advanced/rule-analysis.md#the-operator-catalogue-engineoperators).

`builtin_operator_names()` reports every built-in key this engine
resolves as an operator: the baseline set plus the extension families
the build compiled in and the engine kept (see
[Operator Families](../advanced/configuration.md#operator-families)),
including the input aliases `var`, `?:` and `match`. It is derived from
the compiler's own lookup table, so it cannot drift from dispatch.
Together with `custom_operator_names()` it is the engine's full
vocabulary, which authoring tools need under templating mode, where an
unknown key is not an error but echoes back as data.

`custom_operator_info(name)` returns what a registered custom operator
declares about itself through `CustomOperator::info`, or `None` when no
custom operator has that name.

---

## EngineBuilder

Fluent constructor for `Engine`. Returned by `Engine::builder()` (or
`Engine::to_builder()`).

```rust
EngineBuilder::new()
    .with_config(EvaluationConfig::default())
    .with_templating(true)                  // feature = "templating"
    .with_template_key_escape('$')          // optional escape for operator-named template keys
    .with_constant_folding(true)            // default; disable to keep every operator visible
    .with_families([Family::ExtString])     // optional: the core plus these families
    .add_operator("name", MyOp)             // typed operator
    .add_operator("dyn", boxed_op)          // also accepts Box<dyn CustomOperator> or Arc<T>
    .try_add_operator("other", OtherOp)?    // Err if a built-in answers to the name
    .try_build()?;                          // or .build(), which cannot fail
```

| Method | Notes |
|--------|-------|
| `with_config(config)` | The [`EvaluationConfig`](#evaluationconfig). |
| `with_templating(on)` | Templating mode for `compile`; only effective with `feature = "templating"`. |
| `with_template_key_escape(prefix)` | Unset by default. See [Configuration](../advanced/configuration.md#combining-with-templating-mode). |
| `with_constant_folding(on)` | Default `true`. |
| `with_families(families)` | The JSONLogic core plus the named `Family` values. See [Operator Families](../advanced/configuration.md#operator-families). |
| `add_operator(name, op)` | Registers any `T: CustomOperator + 'static`, `Box<dyn CustomOperator>` and `Arc<T>` included. A second registration under a name replaces the first. |
| `try_add_operator(name, op)` | Refuses a name a built-in of the builder's families answers to, or one that begins with the template key escape, with a `ConfigurationError`. |
| `check_operator_name(&name)` | The same refusal without registering anything or consuming the builder. |
| `check_operator_names()` | Checks every name `try_add_operator` took against the builder's settings now. |
| `try_build()` | `build()`, after `check_operator_names()`, and refusing a `max_recursion_depth` of 0. |
| `build()` | Finalises the engine. |

`add_operator` accepts any name, but a built-in always wins, so an
operator registered as `if` or `var` never runs. The
[Custom Operators](../advanced/custom-operators.md#names-a-built-in-already-answers-to)
guide covers `try_add_operator`, `try_build` and sharing an operator
between engines through `Arc`.

`with_constant_folding(false)` suits tooling that walks the compiled tree
and would be surprised by `{"+": [1, 2]}` collapsing to a literal `3`.
The one-shot trace entry points (`TracedSession::eval_str`, `eval_into`)
and `TracedSession::compile` compile with folding off whatever this
setting says; `TracedSession::eval` and `eval_borrowed` run the `Logic`
you pass.

### Family

`Family` names an operator family: `Core`, `DateTime`, `ExtString`,
`ExtArray`, `ExtObject`, `ExtControl`, `ErrorHandling`, `ExtMath`,
`Tensor`, `Flagd`. Every variant exists in every build.

```rust
impl Family {
    pub const ALL: &'static [Family];
    pub const fn name(self) -> &'static str;      // "ExtString", as OperatorInfo::family reports it
    pub const fn is_compiled(self) -> bool;       // whether the build has its Cargo feature
}
```

---

## Logic

The compiled, reusable rule tree. Output of `Engine::compile`.

- `Send + Sync`: wrap in `Arc` to share across threads (or use
  `Engine::compile_arc` to do it in one step).
- Immutable after construction.

| Method | Returns |
|--------|---------|
| `facts()` | A [`Facts`](#facts): what the rule reads, which operators it calls, whether its result depends only on its data. |
| `resolve_node_ids(&ids)` | `Vec<PathStep>`: the breadcrumb of a structured `Error` translated into the source path of the failing node. |
| `compiled_on(&engine)` | Whether that engine instance compiled the rule. |
| `pointer(id)` / `pointers()` | The JSON Pointer each node was compiled from, for a rule compiled with `TracedSession::compile`; `None` / empty otherwise. |
| `is_static()` | Whether the tree could be evaluated without a data context. |
| `is_constant()` | Whether compilation folded the whole rule to a literal. |
| `cse_slot_count()` | How many shared subexpressions the compiler memoises per evaluation. |
| `to_json()` | The compiled rule as JSONLogic text (also its `Display`). Folded subexpressions appear as literals; the output compiles back to an equivalent rule. |

Any engine can evaluate any rule. On an engine other than the one that
compiled it, the rule looks its custom operators up by name, and a rule
whose constants were folded under different evaluation settings is
compiled again for that engine (once per distinct setting), so folded
constants follow the evaluating engine's config. In 6.0 a rule will
evaluate only on its own engine; `compiled_on` finds the places that rely
on the lookup.

### Facts

```rust
let rule = engine.compile(r#"{"if": [{"var": "user.vip"},
    {"map": [{"var": "cart"}, {"var": "price"}]}, []]}"#)?;
let facts = rule.facts();

facts.reads();               // [cart, user.vip]: `DataPath`s from the root
facts.has_computed_reads();  // false: no path is computed at runtime
facts.reads_complete();      // true: `reads()` is everything the rule can read
facts.reads_data();          // true
facts.operators();           // ["if", "map", "val"]: canonical names
facts.custom_operators();    // []
facts.is_deterministic();    // true: no `now`, no custom operator
```

`DataPath` is one path as segments: `segments()`, `is_root()`,
`covers(&other)`, and a `Display` that joins segments with dots. What
each answer covers, and what it leaves out, is in
[Rule Analysis](../advanced/rule-analysis.md#what-a-rule-reads-logicfacts).

---

## Session

Reusable evaluation handle that owns a `bumpalo::Bump`. The session
**never** auto-resets: you decide when to release arena memory back to
the start-of-chunk position. Construct via `Engine::session()`.

```rust
let mut session = engine.session();
let result_str: String = session.eval_str(&compiled, data_json)?;
let result_owned: datalogic_rs::datavalue::OwnedDataValue =
    session.eval(&compiled, data)?;

// feature = "serde_json"
let value: serde_json::Value = session.eval_into(&compiled, &serde_data)?;

// Zero-copy borrowed result; lives until the next &mut self call.
let view: &datalogic_rs::DataValue<'_> = session.eval_borrowed(&compiled, data)?;

session.reset();                       // bound peak memory between batches
session.reset_with_capacity(64 * 1024);
let bytes = session.allocated_bytes();
let engine_ref: &Engine = session.engine();
```

`Session::eval` / `eval_str` / `eval_into` / `eval_as` accept any
`EvalInput<'_>`. `eval_borrowed` returns a `&'a DataValue<'a>` that
borrows from the session's arena; the borrow checker ends it at the next
`&mut self` call. With `feature = "budget"`,
`eval_metered(&compiled, data, budget)` returns a
`Metered<OwnedDataValue>`.

`Session` is `Send` and not `Sync`: one per thread or task.

### SharedSession

`Session<'engine, E = &'engine Engine>` has a type parameter for how it
holds its engine. `Engine::session()` borrows it (`Session<'_>`, the
default). `SharedSession`, an alias for `Session<'static, Arc<Engine>>`,
holds an `Arc<Engine>` instead, so it is `'static + Send`: you can store
it in a struct, move it to another thread or hold it across an `.await`
without borrowing the engine. Both forms have the same methods.

```rust
use std::sync::Arc;
use datalogic_rs::{Engine, SharedSession};

let engine = Arc::new(Engine::new());
let logic = engine.compile(r#"{"var": "x"}"#)?;
let mut session = SharedSession::new(Arc::clone(&engine)); // or SharedSession::from(engine)
let handle = std::thread::spawn(move || session.eval_str(&logic, r#"{"x": 1}"#));
assert_eq!(handle.join().unwrap()?, "1");
```

---

## EvalInput

Sealed input adapter trait used by `Engine::evaluate` and the `Session`
methods, and the `OwnedInput` cousin used by the owned one-shot entry
points.

| Implementor | Cost |
|-------------|------|
| `&'a DataValue<'a>` | Pass-through. |
| `DataValue<'a>` | One arena alloc. |
| `&'a str` | JSON parse via `DataValue::from_str`; unescaped strings borrow from the text. |
| `&'a String` | JSON parse, same as `&str`. |
| `&OwnedDataValue` | Viewed in place: array and object spines built in the arena, leaves borrowed. |
| `&'a ParsedData` | Pass-through: the tree is already arena-resident. |
| `&serde_json::Value` (`feature = "serde_json"`) | Viewed in place, like an owned value. |
| `&Roots` / `Roots` | One object node, plus each root's own cost. |

The trait is sealed; external crates cannot add new shapes.

On the engine that compiled a rule, an owned, `serde_json` or `Roots`
input is brought in only along the paths the rule reads, when
[`Logic::facts`](#facts) says those reads are complete. A rule with a
computed path, a custom operator that may read the context, or a read of
the whole input sees the whole input, as do traced runs and evaluation on
another engine. JSON text and `ParsedData` are not projected.

### Roots

Several values evaluated as the fields of one top-level object, without
building that object. Each root is borrowed and viewed in place the way
the engine views a single input of that type, so a host that keeps a
payload and its metadata apart does not copy both into a combined value
for every evaluation.

```rust
let roots = Roots::from([("data", &payload), ("metadata", &metadata)]);
session.eval_into::<serde_json::Value, _>(&compiled, &roots)?;

// Mixed representations, built up one root at a time:
let roots = Roots::new()
    .root("data", &payload)        // &serde_json::Value
    .root("claims", &claims)       // &OwnedDataValue
    .root("config", &parsed);      // &ParsedData
```

A rule reads `{"var": "data.user"}` exactly as it would from
`{"data": payload, "metadata": metadata}`. Names keep the order they were
first given in (the key order of `{"var": ""}`), and a repeated name
replaces its value in place. `Roots` is accepted wherever the engine
takes input, by reference or by value. Each root is a `RootValue`, built
through `From` from `&serde_json::Value`, `&OwnedDataValue`, `&ParsedData`
or `&DataValue`. `insert(name, value)`, `names()`, `len()` and
`is_empty()` complete the type.

Against a `json!({"data": data, "metadata": metadata})` merge built per
evaluation, `Roots` measured 7 to 10 times faster for payloads of 4 to
1,024 fields.

### OwnedInput

The owned-entry-point cousin used by `Engine::eval*` and the module-level
helpers, where the engine creates and owns the arena per call. Also
sealed; the supported shapes are `&str` and `&String` (parsed into the
per-call arena), `&OwnedDataValue` and `OwnedDataValue` (viewed in
place), `&serde_json::Value` (`feature = "serde_json"`, viewed in place),
and `&Roots` / `Roots`.

### FromDataValue

Sealed result-side counterpart: the `R` in `Session::eval*`,
`Engine::eval_as` and `TracedSession::eval*` is projected out of the
arena through `FromDataValue::from_arena(&DataValue) -> Result<R>`.
Implemented for `OwnedDataValue` (deep clone), `String` (JSON
serialisation), and `serde_json::Value` (`feature = "serde_json"`). The
typed `eval_into::<T>` paths go through `serde_json::Value` and then
`serde_json::from_value`.

---

## ParsedData

A parse-once data handle: a self-contained JSON document that owns its
own arena. Parse a payload once, then evaluate any number of rules
against it at zero per-call conversion cost (`&ParsedData` implements
`EvalInput`).

```rust
impl ParsedData {
    pub fn from_json(json: &str) -> Result<Self>;                 // ParseError on malformed input
    pub fn from_value(value: &serde_json::Value) -> Self;         // feature = "serde_json"
    pub fn from_owned(value: &OwnedDataValue) -> Self;
    pub fn value(&self) -> &DataValue<'_>;                        // borrow the parsed tree
    pub fn allocated_bytes(&self) -> usize;                       // input copy + tree
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
    Tensor(...),    // feature = "tensor"
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
    #[cfg(feature = "budget")]
    pub ops_budget: Option<u64>,                     // default: None (unbounded)
    pub missing_var: MissingVar,                     // default: Null
}

let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::ThrowError)
    .with_division_by_zero(DivisionByZeroHandling::ReturnSaturated)
    .with_loose_equality_errors(true)
    .with_truthy_evaluator(TruthyEvaluator::JavaScript)
    .with_numeric_coercion(NumericCoercionConfig::default())
    .with_max_recursion_depth(256)
    .with_missing_var(MissingVar::Null);
```

Presets:

```rust
EvaluationConfig::default();
EvaluationConfig::safe_arithmetic();
EvaluationConfig::strict();
```

With `feature = "serde_json"`, `EvaluationConfig::from_json_str` builds
one from the JSON object the bindings use. What each setting does is in
the [Configuration](../advanced/configuration.md) guide.

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
(`{"/": [10, 0]}`) raises `Thrown { type: "NaN" }` under every setting,
because no in-range integer sentinel exists. A fractional dividend
(`{"/": [10.5, 0]}`) takes the configured path. See
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

The `Custom` callback receives an `&OwnedDataValue`, so it needs no
`serde_json`. `TruthyEvaluator::custom(f)` wraps a closure without
spelling out the `Arc::new(...)`:

```rust
let config = EvaluationConfig::default().with_truthy_evaluator(
    TruthyEvaluator::custom(|v: &OwnedDataValue| {
        v.as_i64().map(|n| n % 2 == 0).unwrap_or(false)   // even integers are truthy
    }),
);
```

### MissingVar

```rust
#[non_exhaustive]
pub enum MissingVar {
    Null,     // default: a `var` / `val` that finds nothing is null
    Error,    // raises VariableNotFound naming the path
}
```

See [Missing Variables](../advanced/configuration.md#missing-variables).

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

    fn info(&self) -> CustomOperatorInfo { CustomOperatorInfo::opaque() }
    fn check(&self, args: &[OwnedDataValue]) -> std::result::Result<(), Diagnostic> { Ok(()) }
}
```

| Parameter | Notes |
|-----------|-------|
| `args` | **Pre-evaluated** arguments. The engine has already recursed into each arg's expression tree. |
| `ctx` | Opaque view into the engine's evaluation context. Untouched by most operators. |
| `arena` | Allocator for the current call. Use `arena.alloc(...)` for `DataValue` and `arena.alloc_str(...)` for strings, or the one-call helpers on [`ArenaExt`](#arenaext). |

`info` declares what the engine may assume about the operator as a
`CustomOperatorInfo` (fields `deterministic`, `reads_context`,
`min_args`, `max_args`; built with `opaque()`, `pure()`,
`reading_context()` and `with_args(min, max)`). `check` validates a
call's arguments as written for `Engine::check`. `Box<dyn CustomOperator>`
and `Arc<T: CustomOperator + ?Sized>` implement the trait and forward all
three methods. See [Custom Operators](../advanced/custom-operators.md).

---

## ArenaExt

Extension trait on `bumpalo::Bump` (bring it into scope with
`use datalogic_rs::ArenaExt;`) that folds "build a `DataValue`, then
allocate it" into one call. It returns static singletons where it can
(`null`, `bool`, small integers, empty strings/arrays/objects), which
makes it the recommended way to return values from a custom operator.

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
operators don't need it. It offers:

- `root_input()`: the input the evaluation started from;
- `depth()`: the number of iteration frames enclosing operators pushed;
- `charge(n)`: charge `n` operations against the
  [operation budget](../advanced/operation-budget.md#pricing-a-custom-operator)
  before doing work the node count does not reflect. Without the `budget`
  feature it returns `Ok(())`, so you can call it unconditionally.

The stack layout stays private so it can change without breaking the
trait contract.

---

## Rule analysis types

[Rule Analysis](../advanced/rule-analysis.md) covers these in full.

| Type | Role |
|------|------|
| `CheckMode` | `Engine`, `Strict` or `Template`: the mode `Engine::check` reads a rule in. |
| `Diagnostic` | One problem: `code`, `severity`, `message`, `pointer` (RFC 6901), `operator`. `Diagnostic::error(msg)` / `warning(msg)` / `at_argument(i)` build one from a custom operator's `check`. |
| `DiagnosticCode` | `Unparsable`, `UnknownOperator`, `NotAnOperator`, `ArgumentForm`, `ArgumentCount`, `InvalidTimezone`, `SimilarToOperator`, `Compile`, `OperatorCheck`. |
| `Severity` | `Error` or `Warning`. |
| `CompileError` | What `compile_checked` returns: `diagnostics: Vec<Diagnostic>`. Implements `std::error::Error`. |
| `Facts` / `DataPath` | What `Logic::facts` returns. See [Facts](#facts). |
| `OperatorInfo` / `ScopedArg` | What `Engine::operators` yields. |

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
    pub fn code(&self) -> ErrorCode;          // the kind without its payload
    pub fn tag(&self) -> &'static str;        // code().as_str()
    pub fn operator(&self) -> Option<&str>;   // innermost failing operator, when known
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
    BudgetExceeded { budget: u64, spent: u64 },   // raised only with feature = "budget"
}
```

`Error::operator()` names the innermost operator on the failing path,
custom operators included, on plain and traced runs alike; when no node
on the path has an operator name, the rule's root operator stands in.

`ErrorKind::BudgetExceeded` exists in every build, so a `match` on it
compiles whatever features are on; only a `budget` build raises it.

`Error` implements `serde::Serialize` (in every build) as:

```json
{
  "type": "<KindTag>",
  "message": "<Display>",
  "operator": "<name>",        // present only when known
  "node_ids": [42, 13, 7],     // present only when non-empty
  // kind-specific extras: variable, level, thrown, index/length, budget/spent
}
```

Use `error.code()` to switch on the kind, `error.thrown_value()` for the
`Thrown` payload, and `error.resolve_path(&compiled)` to translate the
`node_ids` breadcrumb into source `PathStep`s.

To wrap a foreign `std::error::Error` into a `Custom` error:

```rust
"abc".parse::<i32>().map_err(datalogic_rs::Error::wrap)?;
```

`Error::source()` walks the inner chain unchanged.

### ErrorCode

`ErrorCode` is an error's kind without its payload: one variant per
`ErrorKind` variant, `Copy`, `Eq`, `Hash` and `Ord`, with every variant
present in every build. `as_str()` (also its `Display`) is the name
`Error::tag()` returns, which the bindings carry on the wire, and
`FromStr` parses it back, failing with `UnknownErrorCode`.
`ErrorCode::ALL` lists every code, and `ErrorKind::code()` gives the code
of a bare kind. The enum is `#[non_exhaustive]`.

```rust
use datalogic_rs::{Engine, ErrorCode};

let engine = Engine::new();
let err = engine.eval_str(r#"{"/": [1, 0]}"#, "{}").unwrap_err();
assert_eq!(err.code(), ErrorCode::Thrown);
assert_eq!("Thrown".parse::<ErrorCode>(), Ok(ErrorCode::Thrown));
```

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
`Error::resolve_path`, root to leaf. `#[non_exhaustive]`, with
`Serialize` / `Deserialize`:

```rust
pub struct PathStep {
    pub node_id: u32,              // matches Error::node_ids
    pub operator: Option<String>,  // None for plain values and arrays
    pub arg_index: Option<u32>,    // position in the parent's arguments; None at the root
    pub json_pointer: String,      // e.g. "/if/0/>/0"; "" for the root
}
```

`json_pointer` escapes tokens as RFC 6901 does (`/` is `~1`). For a rule
compiled with `TracedSession::compile` it is `Logic::pointer`, the
pointer into the rule as written.

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
    // Compile in this mode instead of the engine's (CheckMode::Strict / Template).
    pub fn with_mode(self, mode: CheckMode) -> Self;

    // Compile as the one-shot paths do (folding off) and record each node's JSON Pointer.
    pub fn compile<R: IntoLogic>(&self, rule: R) -> Result<Logic>;

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
// Cargo.toml: datalogic-rs = { version = "5", features = ["trace"] }
let engine = datalogic_rs::Engine::new();
let run = engine.trace().eval_str(r#"{"+": [1, 2]}"#, r#"{}"#);
println!("{}", run.result.unwrap());   // 3
println!("{} steps", run.steps.len());
```

The pre-compiled paths (`eval`, `eval_borrowed`) inherit whatever shape
the `Logic` was compiled into: a rule from `Engine::compile` has its
constant subexpressions folded, so those operators record no step. For
full coverage, use `eval_str` / `eval_into`, or compile with
`engine.trace().compile(rule)` and evaluate that `Logic` as many times as
you need. A rule compiled that way also records, for every node id, the
JSON Pointer of the rule value it came from: `logic.pointer(step.node_id)`
places each `ExecutionStep` and `ExpressionNode` in the rule as written.

```rust
let engine = datalogic_rs::Engine::new();
let logic = engine.trace().compile(r#"{"if": [{"var": "a"}, 1, 2]}"#)?;
let run = engine.trace().eval(&logic, r#"{"a": true}"#);
assert_eq!(logic.pointer(run.steps[0].node_id), Some("/if/0"));
```

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
