# Configuration

Customize evaluation behavior with `EvaluationConfig` and the
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

> v5 dropped the inherent `Engine::with_config` /
> `with_preserve_structure` / `with_config_and_structure` constructors —
> use the builder. There is no compatibility shim. See the
> [Migration Guide](../migration.md) for the v4 → v5 mapping.

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

Control how non-numeric values are handled in arithmetic operations.

```rust
use datalogic_rs::{EvaluationConfig, NanHandling};

// ThrowError (default), IgnoreValue, CoerceToZero, ReturnNull
let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::IgnoreValue);
```

**Behavior comparison** for `{"+": [1, "text", 2]}`:

| Setting | Result |
|---------|--------|
| `ThrowError` (default) | `Err(Thrown { type: "NaN" })` |
| `IgnoreValue` | `3` (skips `"text"`) |
| `CoerceToZero` | `3` (`"text"` → `0`) |
| `ReturnNull` | `null` |

### Division by Zero

```rust
use datalogic_rs::{EvaluationConfig, DivisionByZeroHandling};

// ReturnSaturated (default), ThrowError, ReturnNull, ReturnInfinity
let config = EvaluationConfig::default()
    .with_division_by_zero(DivisionByZeroHandling::ThrowError);
```

**Behavior comparison** for `{"/": [10, 0]}`:

| Setting | Result |
|---------|--------|
| `ReturnSaturated` (default) | `f64::MAX` (sign of dividend) |
| `ThrowError` | `Err(Thrown { type: "NaN" })` |
| `ReturnNull` | `null` |
| `ReturnInfinity` | `Infinity` (sign of dividend) |

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

> **v5 change:** `TruthyEvaluator::Custom` now takes
> `Arc<dyn Fn(&OwnedDataValue) -> bool + Send + Sync>` (the canonical owned
> value type). v4 used `&serde_json::Value`.

**Truthiness comparison:**

| Value | JavaScript | Python | StrictBoolean |
|-------|-----------|--------|---------------|
| `true` | truthy | truthy | truthy |
| `false` | falsy | falsy | falsy |
| `1` | truthy | truthy | falsy |
| `0` | falsy | falsy | falsy |
| `""` | falsy | falsy | falsy |
| `"0"` | truthy | truthy | falsy |
| `[]` | falsy | falsy | falsy |
| `[0]` | truthy | truthy | falsy |
| `null` | falsy | falsy | falsy |

### Loose Equality Errors

Control whether loose equality (`==`) raises errors for incompatible types.

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

### Max Recursion Depth

Cap the number of nested evaluation-boundary calls before the engine
bails with a `ConfigurationError`. The limit is tracked per thread and
guards against custom operators that hold an `Arc<Engine>` and re-enter
via `engine.evaluate(...)`. Pure built-in workloads skip the check
entirely, so they pay nothing.

```rust
use datalogic_rs::EvaluationConfig;

// Default is 256: raise it for deeply nested custom-operator graphs,
// lower it to bail sooner.
let config = EvaluationConfig::default()
    .with_max_recursion_depth(256);
```

## Configuration Presets

```rust
use datalogic_rs::{Engine, EvaluationConfig};

// Lenient arithmetic — IgnoreValue + ReturnNull divide-by-zero
let engine = Engine::builder()
    .with_config(EvaluationConfig::safe_arithmetic())
    .build();

// Strict — errors for any type mismatch and no numeric coercion
let engine = Engine::builder()
    .with_config(EvaluationConfig::strict())
    .build();
```

## Configuring from JSON

`EvaluationConfig::from_json_str` (requires `feature = "serde_json"`)
builds a configuration from a JSON object. This is the wire format the
language bindings use to pass engine configuration across FFI
boundaries through one shared parser; Rust callers normally use the
typed `with_*` setters above.

All keys are optional. The `"preset"` key is applied first, then the
remaining keys override individual fields on top of it. Unknown keys
and unknown enum strings are rejected with a `ConfigurationError`, so
typos fail loudly instead of being silently ignored.

| Key | Value |
|-----|-------|
| `preset` | `"default"`, `"safe_arithmetic"`, or `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, or `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, or `"return_infinity"` |
| `loose_equality_errors` | bool |
| `truthy_evaluator` | `"javascript"`, `"python"`, or `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

Custom truthiness closures (`TruthyEvaluator::Custom`) cannot be
expressed in JSON; they are available through the Rust API only.

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
strict-boolean truthiness:

```json
{
  "preset": "safe_arithmetic",
  "truthy_evaluator": "strict_boolean",
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

let result = engine.eval_str(r#"{"+": [1, "2"]}"#, r#"{}"#);
// Err(...) — strict mode does not coerce "2" to a number
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
