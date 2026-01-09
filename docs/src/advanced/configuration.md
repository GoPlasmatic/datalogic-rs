# Configuration

Customize evaluation behavior with `EvaluationConfig`.

## Creating a Configured Engine

```rust
use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling};

// Default configuration
let engine = DataLogic::new();

// Custom configuration
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::IgnoreValue);
let engine = DataLogic::with_config(config);
```

## Configuration Options

### NaN Handling

Control how non-numeric values are handled in arithmetic operations.

```rust
use datalogic_rs::{EvaluationConfig, NanHandling};

// Option 1: Throw an error (default)
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::ThrowError);

// Option 2: Ignore non-numeric values
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::IgnoreValue);

// Option 3: Coerce to zero
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::CoerceToZero);
```

**Behavior comparison:**

```rust
let rule = json!({ "+": [1, "text", 2] });

// NanHandling::ThrowError
// Result: Error

// NanHandling::IgnoreValue
// Result: 3 (ignores "text")

// NanHandling::CoerceToZero
// Result: 3 ("text" becomes 0)
```

### Division by Zero

Control how division by zero is handled.

```rust
use datalogic_rs::{EvaluationConfig, DivisionByZero};

// Option 1: Return Infinity/-Infinity (default)
let config = EvaluationConfig::default()
    .with_division_by_zero(DivisionByZero::ReturnBounds);

// Option 2: Throw an error
let config = EvaluationConfig::default()
    .with_division_by_zero(DivisionByZero::ThrowError);

// Option 3: Return null
let config = EvaluationConfig::default()
    .with_division_by_zero(DivisionByZero::ReturnNull);
```

**Behavior comparison:**

```rust
let rule = json!({ "/": [10, 0] });

// DivisionByZero::ReturnBounds
// Result: Infinity

// DivisionByZero::ThrowError
// Result: Error

// DivisionByZero::ReturnNull
// Result: null
```

### Truthiness Evaluation

Control how values are evaluated for truthiness in boolean contexts.

```rust
use datalogic_rs::{EvaluationConfig, TruthyEvaluator};
use std::sync::Arc;

// Option 1: JavaScript-style (default)
let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::JavaScript);

// Option 2: Python-style
let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::Python);

// Option 3: Strict boolean (only true/false)
let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::StrictBoolean);

// Option 4: Custom evaluator
let custom = Arc::new(|value: &serde_json::Value| -> bool {
    // Custom logic: only positive numbers are truthy
    value.as_f64().map_or(false, |n| n > 0.0)
});
let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::Custom(custom));
```

**Truthiness comparison:**

| Value | JavaScript | Python | StrictBoolean |
|-------|-----------|--------|---------------|
| `true` | truthy | truthy | truthy |
| `false` | falsy | falsy | falsy |
| `1` | truthy | truthy | error/falsy |
| `0` | falsy | falsy | error/falsy |
| `""` | falsy | falsy | error/falsy |
| `"0"` | truthy | truthy | error/falsy |
| `[]` | falsy | falsy | error/falsy |
| `[0]` | truthy | truthy | error/falsy |
| `null` | falsy | falsy | error/falsy |

### Loose Equality Errors

Control whether loose equality (`==`) throws errors for incompatible types.

```rust
let config = EvaluationConfig::default()
    .with_loose_equality_throws_errors(true);  // default
// or
let config = EvaluationConfig::default()
    .with_loose_equality_throws_errors(false);
```

## Configuration Presets

### Safe Arithmetic

Ignores invalid values in arithmetic operations:

```rust
let engine = DataLogic::with_config(EvaluationConfig::safe_arithmetic());

// Equivalent to:
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::IgnoreValue)
    .with_division_by_zero(DivisionByZero::ReturnNull);
```

### Strict Mode

Throws errors for any type mismatches:

```rust
let engine = DataLogic::with_config(EvaluationConfig::strict());

// Equivalent to:
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::ThrowError)
    .with_division_by_zero(DivisionByZero::ThrowError)
    .with_loose_equality_throws_errors(true);
```

## Combining with Structure Preservation

Use both configuration and structure preservation:

```rust
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::CoerceToZero);

let engine = DataLogic::with_config_and_structure(config, true);
```

## Configuration Examples

### Lenient Data Processing

For processing potentially messy data:

```rust
let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::IgnoreValue)
    .with_division_by_zero(DivisionByZero::ReturnNull);

let engine = DataLogic::with_config(config);

// This won't error even with bad data
let rule = json!({ "+": [1, "not a number", null, 2] });
let result = engine.evaluate_json(&rule.to_string(), "{}").unwrap();
// Result: 3 (ignores non-numeric values)
```

### Strict Validation

For scenarios requiring precise type handling:

```rust
let config = EvaluationConfig::strict();
let engine = DataLogic::with_config(config);

// This will error on type mismatches
let rule = json!({ "+": [1, "2"] });
let result = engine.evaluate_json(&rule.to_string(), "{}");
// Result: Error (strict mode doesn't coerce "2" to number)
```

### Custom Business Logic Truthiness

For domain-specific truth evaluation:

```rust
use std::sync::Arc;

// Only non-empty strings and positive numbers are truthy
let custom_truthy = Arc::new(|value: &serde_json::Value| -> bool {
    match value {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().map_or(false, |n| n > 0.0),
        serde_json::Value::String(s) => !s.is_empty(),
        _ => false,
    }
});

let config = EvaluationConfig::default()
    .with_truthy_evaluator(TruthyEvaluator::Custom(custom_truthy));

let engine = DataLogic::with_config(config);

// With this config:
// { "if": [0, "yes", "no"] } => "no" (0 is not positive)
// { "if": [-5, "yes", "no"] } => "no" (-5 is not positive)
// { "if": [1, "yes", "no"] } => "yes" (1 is positive)
```
