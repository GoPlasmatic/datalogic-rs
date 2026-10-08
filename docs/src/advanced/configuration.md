# Configuration

You configure evaluation behaviour with `EvaluationConfig`, and the engine
itself (templating mode, operator families, custom operators) with
`EngineBuilder`.

## Creating a Configured Engine

```rust
use datalogic_rs::{Engine, EvaluationConfig, NanHandling};

// Default configuration
let engine = Engine::new();

// Custom configuration
let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::IgnoreValue);
let engine = Engine::builder().with_config(config).build();
```

The builder is the only way to configure an engine. The
[Migration Guide](../migration.md) maps the 4.x constructors
(`Engine::with_config` and friends) onto it.

## Configuration Options

`EvaluationConfig` is `#[non_exhaustive]`. Construct it with `default()`
(or a preset such as `safe_arithmetic()` / `strict()`), then chain the
`with_*` setters:

```rust
use datalogic_rs::{EvaluationConfig, NanHandling, DivisionByZeroHandling};

let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::IgnoreValue)
    .with_division_by_zero(DivisionByZeroHandling::ReturnNull)
    .with_loose_equality_errors(false);
```

### NaN Handling

Controls what arithmetic does with an argument it cannot coerce to a
number.

```rust
use datalogic_rs::{EvaluationConfig, NanHandling};

// ThrowError (default), IgnoreValue, CoerceToZero, ReturnNull
let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::IgnoreValue);
```

**Behavior comparison:**

| Setting | `{"+": [1, "text", 2]}` | `{"*": [2, "x", 3]}` |
|---------|-------------------------|----------------------|
| `ThrowError` (default) | `Err(Thrown { type: "NaN" })` | `Err(Thrown { type: "NaN" })` |
| `IgnoreValue` | `3` (skips `"text"`) | `6` (skips `"x"`) |
| `CoerceToZero` | `3` (`"text"` becomes `0`) | `0` (`"x"` becomes `0`) |
| `ReturnNull` | `null` | `null` |

The strings `"NaN"`, `"inf"` and `"infinity"`, and numeric strings that
overflow `f64` (`"1e400"`), count as non-numbers here: arithmetic applies
this setting to them instead of producing a non-finite number.

### Division by Zero

```rust
use datalogic_rs::{EvaluationConfig, DivisionByZeroHandling};

// ReturnSaturated (default), ThrowError, ReturnNull, ReturnInfinity
let config = EvaluationConfig::default()
    .with_division_by_zero(DivisionByZeroHandling::ThrowError);
```

**Behavior comparison** for `{"/": [10.5, 0]}`:

| Setting | Result |
|---------|--------|
| `ReturnSaturated` (default) | `1.7976931348623157e308` (`f64::MAX`, sign of dividend) |
| `ThrowError` | `Err(Thrown { type: "NaN" })` |
| `ReturnNull` | `null` |
| `ReturnInfinity` | `f64::INFINITY` (sign of dividend) as an `OwnedDataValue` / `DataValue`; `null` on the JSON-string paths |

The setting governs the float path only. An integer dividend over an
integer zero (`{"/": [10, 0]}`, and likewise `{"%": [10, 0]}`) raises
`Err(Thrown { type: "NaN" })` under every setting, because no in-range
integer sentinel exists; a fractional dividend such as `10.5` takes the
configured path. `ReturnInfinity` yields the infinite `f64` from
`Engine::eval` / `Session::eval`, but `eval_str` (and every language
binding) renders it as `null` because JSON cannot encode infinity.

### Truthiness Evaluation

```rust
use std::sync::Arc;
use datalogic_rs::{EvaluationConfig, TruthyEvaluator};
use datalogic_rs::datavalue::OwnedDataValue;

// JavaScript (default), Python, StrictBoolean, Custom
let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::Python);

// Custom truthy: receives an OwnedDataValue (no serde_json required)
let custom = Arc::new(|value: &OwnedDataValue| -> bool {
    value.as_f64().map_or(false, |n| n > 0.0)
});
let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::Custom(custom));
```

`TruthyEvaluator::custom(f)` wraps a closure in the `Arc` for you.

**Truthiness comparison:**

| Value | JavaScript | Python | StrictBoolean |
|-------|-----------|--------|---------------|
| `true` | truthy | truthy | truthy |
| `false` | falsy | falsy | falsy |
| `1` | truthy | truthy | truthy |
| `0` | falsy | falsy | truthy |
| `""` | falsy | falsy | truthy |
| `"0"` | truthy | truthy | truthy |
| `[]` | falsy | falsy | truthy |
| `[0]` | truthy | truthy | truthy |
| `{}` | falsy | falsy | truthy |
| `null` | falsy | falsy | falsy |

`StrictBoolean` treats only `null` and `false` as falsy; every other
value, including `0`, `""`, and empty collections, is truthy. `Python`
differs from `JavaScript` on one value: `NaN` (which only arithmetic
produces, never a JSON literal) is falsy in JavaScript and truthy in
Python.

To apply the configured rules to a value you already hold, such as an
evaluated result, call `engine.truthy_of(&value)` (see the
[API Reference](../rust/api-reference.md#truthy_of)) instead of writing
your own check.

### Loose Equality Errors

Controls whether loose equality (`==`) raises errors for incompatible types.

```rust
let config = EvaluationConfig::default()
    .with_loose_equality_errors(true);   // default
```

### Numeric Coercion

`NumericCoercionConfig` is `#[non_exhaustive]` too: start from
`default()` and chain its own `with_*` setters, then pass it through
`with_numeric_coercion`.

```rust
use datalogic_rs::{EvaluationConfig, NumericCoercionConfig};

let config = EvaluationConfig::default()
    .with_numeric_coercion(
        NumericCoercionConfig::default()
            .with_empty_string_to_zero(false)
            .with_null_to_zero(false)
            .with_bool_to_number(false)
            .with_reject_non_numeric(true),
    );
```

### Missing Variables

`missing_var` chooses what a `var` / `val` read that finds nothing
evaluates to. The default, `MissingVar::Null`, is the JSONLogic rule.
`MissingVar::Error` raises `VariableNotFound` naming the path, so a typo
in a path fails instead of flowing on as `null`:

```rust
use datalogic_rs::{Engine, ErrorCode, EvaluationConfig, MissingVar};

let engine = Engine::builder()
    .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error))
    .build();

let err = engine.eval_str(r#"{"var": "user.nmae"}"#, r#"{"user": {"name": "ana"}}"#).unwrap_err();
assert_eq!(err.code(), ErrorCode::VariableNotFound);

// Not misses: a default, a present null, and the existence operators.
assert_eq!(engine.eval_str(r#"{"var": ["x", 0]}"#, "{}").unwrap(), "0");
assert_eq!(engine.eval_str(r#"{"var": "a"}"#, r#"{"a": null}"#).unwrap(), "null");
assert_eq!(engine.eval_str(r#"{"missing": ["x"]}"#, "{}").unwrap(), r#"["x"]"#);
```

Only reads count. A `var` with a default takes the default, a field that
is present and `null` is `null`, `missing`, `missing_some` and `exists`
still answer whether paths exist, and iteration metadata
(`{"val": [[1], "index"]}`) is `null` outside an iterator. `try` catches
the error like any other. Under `MissingVar::Error` the iterator fast
paths that read fields inline step aside, so `map`, `filter` and `reduce`
over a field run slower; with the `budget` feature they charge the same
operation count either way.

### Max Recursion Depth

`max_recursion_depth` caps how many evaluations may run at once on one
thread, the outermost included. It exists for custom operators that hold
an `Arc<Engine>` and re-enter it (`engine.evaluate(...)`, a session or a
traced run) from inside their own `evaluate`. An evaluation that would
pass the cap fails with a `ConfigurationError` before it starts.

It does not bound how deeply a rule nests (the compiler caps that at 256
levels on every engine), how deep a value gets, or how much work an
evaluation does (see [Operation Budget](#operation-budget)). The count is
kept per thread across every engine on it; each engine compares it with
its own setting. An engine with no custom operators skips the check,
since built-ins cannot re-enter the engine.

```rust
use datalogic_rs::EvaluationConfig;

// Default is 256: raise it for deeply nested custom-operator graphs,
// lower it to bail sooner.
let config = EvaluationConfig::default()
    .with_max_recursion_depth(256);
```

The value must be at least 1. `EvaluationConfig::from_json_str` and
`EngineBuilder::try_build` refuse 0 with a `ConfigurationError`;
`EngineBuilder::build`, which cannot fail, keeps it, and no evaluation on
that engine with custom operators can then start.

### Operation Budget

Caps the work one evaluation may do (requires `feature = "budget"`).
Where the recursion depth above bounds *boundary re-entry*, this bounds
the work itself: a `map` over a large input nested inside another `map`
is unbounded under the depth cap and bounded under this one.

```rust,ignore
use datalogic_rs::EvaluationConfig;

// Unbounded by default. `Some(n)` refuses any evaluation that would
// charge more than n operations.
let config = EvaluationConfig::default().with_ops_budget(Some(100_000));
```

Crossing the ceiling raises `ErrorKind::BudgetExceeded { budget, spent }`
**before** the work is done, and a `try` in the rule cannot recover from
it. The variant exists in every build, so a `match` on it compiles with or
without the feature; only a `budget` build raises it. See
[Operation Budget](operation-budget.md) for what one operation is, how to
pick a number, and the per-call `Engine::evaluate_metered` entry point
that reports what a rule spent.

## Configuration Presets

```rust
use datalogic_rs::{Engine, EvaluationConfig};

// Lenient: IgnoreValue arithmetic, ReturnNull division by zero,
// loose equality errors off
let engine = Engine::builder()
    .with_config(EvaluationConfig::safe_arithmetic())
    .build();

// Strict: ThrowError for NaN and division by zero, and no numeric coercion
let engine = Engine::builder()
    .with_config(EvaluationConfig::strict())
    .build();
```

## Configuring from JSON

`EvaluationConfig::from_json_str` (requires `feature = "serde_json"`)
builds a configuration from a JSON object. The language bindings pass
engine configuration across their FFI boundaries in this format, through
one shared parser; in Rust code, the typed `with_*` setters above do the
same job.

All keys are optional. The parser applies the `"preset"` key first, then
the remaining keys override individual fields on top of it. It rejects
unknown keys and unknown enum strings with a `ConfigurationError`, so a
typo fails instead of being ignored.

| Key | Value |
|-----|-------|
| `preset` | `"default"`, `"safe_arithmetic"`, or `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, or `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, or `"return_infinity"` |
| `loose_equality_errors` | bool |
| `truthy_evaluator` | `"javascript"`, `"python"`, or `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `null` for unbounded (`budget` feature) |
| `missing_var` | `"null"` or `"error"` |

Custom truthiness closures (`TruthyEvaluator::Custom`) cannot be
expressed in JSON; only the Rust API offers them.

From Rust:

```rust
use datalogic_rs::{Engine, EvaluationConfig};

let config = EvaluationConfig::from_json_str(r#"{
    "preset": "strict",
    "division_by_zero": "return_null",
    "numeric_coercion": {"null_to_zero": true},
    "max_recursion_depth": 64
}"#).unwrap();

let engine = Engine::builder().with_config(config).build();
```

The same JSON object is what you hand to a binding's engine
constructor. For example, to start from the lenient preset but use
strict-boolean truthiness and fail on missing variables:

```json
{
  "preset": "safe_arithmetic",
  "truthy_evaluator": "strict_boolean",
  "missing_var": "error",
  "max_recursion_depth": 128
}
```

## Combining with Templating Mode

Use both configuration and templating mode (requires
`feature = "templating"`):

```rust
let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::CoerceToZero);

let engine = Engine::builder()
    .with_config(config)
    .with_templating(true)
    .build();
```

`with_templating` sets the mode `compile` uses. To choose the mode for
one rule instead, call `engine.compile_template(rule)` or
`engine.compile_strict(rule)`: one engine, with one set of custom
operators, can then check conditions strictly and compile output
templates. See
[Structured Objects](./structured-objects.md#per-compile).

Templating mode carries one option of its own,
`with_template_key_escape(prefix)`, unset by default. Without it a
single-key object is always an operator invocation, so a key that names a
built-in (`type`, `map`, `if`, `length`, ...) or a registered custom
operator can never be emitted as an output field. With it, the engine
strips one leading `prefix` from every template key and never resolves an
escaped key as an operator:

```rust
let engine = Engine::builder()
    .with_templating(true)
    .with_template_key_escape('$')
    .build();

// {"$type": {"var": "x"}}  ->  {"type": 1}
// {"$$type": 1}            ->  {"$type": 1}
```

The prefix is a `char` rather than a fixed `$`, so payloads that already
use `$` keys can choose `~` or `#` instead. The escape applies to
`compile_template` too, and `try_add_operator` refuses a custom operator
whose name begins with it, since such a key is an output field in a
template. Full rules, including the duplicate-key and bare-sigil cases,
are in
[Structured Objects](./structured-objects.md#emitting-keys-that-are-operator-names).

## Operator Families

By default an engine has every operator family the build compiled in.
`EngineBuilder::with_families` keeps it to the JSONLogic core plus the
families you name:

```rust
use datalogic_rs::{Engine, Family};

// The JSONLogic core and the string extensions, nothing else.
let engine = Engine::builder().with_families([Family::ExtString]).build();

assert_eq!(engine.eval_str(r#"{"upper": "a"}"#, "null").unwrap(), r#""A""#);
assert!(engine.eval_str(r#"{"sort": [[2, 1]]}"#, "null").is_err()); // ExtArray left out
```

`Family` names every family in every build: `Core`, `DateTime`,
`ExtString`, `ExtArray`, `ExtObject`, `ExtControl`, `ErrorHandling`,
`ExtMath`, `Tensor` and `Flagd`. `Family::ALL` lists them,
`name()` gives the name `Engine::operators()` reports, and
`is_compiled()` says whether the Cargo feature behind it is on. `Core` is
always there, named or not, and a family the build did not compile is
absent whatever you pass. The `all-operators` feature compiles every
family.

A family left out is not there for that engine. Its operator names
compile as unknown operators (an error at evaluation, or up front from
`compile_checked` and `check`), as output fields in templating mode, or
as a custom operator registered under that name. `Engine::operators()`,
`builtin_operator_names()`, `check`'s suggestions and `try_add_operator`
all follow the engine's families. The set applies when a rule is
compiled: a rule compiled on another engine keeps its operators wherever
it runs.

The bindings take the same family names: `families` (Node, WASM,
Python), `Families` (Go), `withFamilies` (JVM, PHP) and `WithFamilies`
(.NET). An unknown name is a `ConfigurationError`.

## Changing a Running Engine's Configuration

`Engine::to_builder()` returns a builder holding the engine's custom
operators, config, templating mode, template key escape, folding setting
and families. Change what differs and build a new engine; the running one
is untouched:

```rust
use datalogic_rs::{Engine, EvaluationConfig, MissingVar};

let running = Engine::new();
let reloaded = running
    .to_builder()
    .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error))
    .build();
```

See [Custom Operators](custom-operators.md#rebuilding-an-engine) for how
the two engines share operator instances.

## Configuration Examples

### Lenient Data Processing

```rust
let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::IgnoreValue)
    .with_division_by_zero(DivisionByZeroHandling::ReturnNull);

let engine = Engine::builder().with_config(config).build();

let r = engine.eval_str(
    r#"{"+": [1, "not a number", null, 2]}"#,
    r#"{}"#,
).unwrap();
// "3" (ignores non-numeric values)
```

### Strict Validation

```rust
let engine = Engine::builder()
    .with_config(EvaluationConfig::strict())
    .build();

let result = engine.eval_str(r#"{"+": [1, null]}"#, r#"{}"#);
// Err(Thrown { type: "NaN" }): strict mode does not coerce null, "",
// true/false, or non-numeric strings such as "abc" to a number.
// Numeric strings still parse, so {"+": [1, "2"]} is 3 even under strict().
```

### Custom Business Logic Truthiness

```rust
use std::sync::Arc;
use datalogic_rs::datavalue::OwnedDataValue;

let custom_truthy = Arc::new(|value: &OwnedDataValue| -> bool {
    match value {
        OwnedDataValue::Bool(b) => *b,
        OwnedDataValue::Number(_) => value.as_f64().map_or(false, |n| n > 0.0),
        OwnedDataValue::String(s) => !s.is_empty(),
        _ => false,
    }
});

let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::Custom(custom_truthy));

let engine = Engine::builder().with_config(config).build();
// {"if": [0,  "yes", "no"]}  ⇒ "no"
// {"if": [-5, "yes", "no"]}  ⇒ "no"
// {"if": [1,  "yes", "no"]}  ⇒ "yes"
```
