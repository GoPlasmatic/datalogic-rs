# API Reference

Core types and methods in datalogic-rs.

## DataLogic

The main engine for compiling and evaluating JSONLogic rules.

### Creating an Engine

```rust
use datalogic_rs::DataLogic;

// Default engine
let engine = DataLogic::new();

// With configuration
let engine = DataLogic::with_config(config);

// With structure preservation (templating mode)
let engine = DataLogic::with_preserve_structure();

// With both
let engine = DataLogic::with_config_and_structure(config, true);
```

### Methods

#### `compile`

Compile a JSONLogic rule into an optimized representation.

```rust
pub fn compile(&self, logic: &Value) -> Result<Arc<CompiledLogic>>
```

**Parameters:**
- `logic` - The JSONLogic rule as a `serde_json::Value`

**Returns:**
- `Ok(Arc<CompiledLogic>)` - Compiled rule, thread-safe and shareable
- `Err(Error)` - Compilation error

**Example:**
```rust
let rule = json!({ ">": [{ "var": "x" }, 10] });
let compiled = engine.compile(&rule)?;
```

#### `evaluate`

Evaluate compiled logic against borrowed data.

```rust
pub fn evaluate(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value>
```

**Parameters:**
- `compiled` - Reference to compiled logic
- `data` - Reference to input data

**Returns:**
- `Ok(Value)` - Evaluation result
- `Err(Error)` - Evaluation error

**Example:**
```rust
let data = json!({ "x": 15 });
let result = engine.evaluate(&compiled, &data)?;
```

#### `evaluate_owned`

Evaluate compiled logic, taking ownership of data.

```rust
pub fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value>
```

**Parameters:**
- `compiled` - Reference to compiled logic
- `data` - Input data (owned)

**Returns:**
- `Ok(Value)` - Evaluation result
- `Err(Error)` - Evaluation error

**Example:**
```rust
let result = engine.evaluate_owned(&compiled, json!({ "x": 15 }))?;
```

#### `evaluate_json`

Convenience method to evaluate JSON strings directly.

```rust
pub fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value>
```

**Parameters:**
- `logic` - JSONLogic rule as a JSON string
- `data` - Input data as a JSON string

**Returns:**
- `Ok(Value)` - Evaluation result
- `Err(Error)` - Parse or evaluation error

**Example:**
```rust
let result = engine.evaluate_json(
    r#"{ "+": [1, 2] }"#,
    r#"{}"#
)?;
```

#### `add_operator`

Register a custom operator.

```rust
pub fn add_operator(&mut self, name: String, operator: Box<dyn Operator>)
```

**Parameters:**
- `name` - Operator name (used in rules)
- `operator` - Boxed operator implementation

**Example:**
```rust
engine.add_operator("double".to_string(), Box::new(DoubleOperator));
```

---

## CompiledLogic

Pre-compiled rule representation. Created by `DataLogic::compile()`.

### Characteristics

- **Thread-safe**: Wrapped in `Arc`, implements `Send + Sync`
- **Immutable**: Cannot be modified after compilation
- **Shareable**: Cheap to clone (reference counting)

### Usage

```rust
// Compile once
let compiled = engine.compile(&rule)?;

// Share across threads
let compiled_clone = Arc::clone(&compiled);
std::thread::spawn(move || {
    engine.evaluate(&compiled_clone, &data);
});

// Evaluate multiple times
for data in datasets {
    engine.evaluate(&compiled, &data)?;
}
```

---

## EvaluationConfig

Configuration for evaluation behavior.

### Creating Configuration

```rust
use datalogic_rs::EvaluationConfig;

let config = EvaluationConfig::default();
```

### Builder Methods

```rust
// NaN handling
config.with_nan_handling(NanHandling::IgnoreValue)

// Division by zero
config.with_division_by_zero(DivisionByZero::ReturnNull)

// Truthiness evaluation
config.with_truthy_evaluator(TruthyEvaluator::Python)

// Loose equality errors
config.with_loose_equality_throws_errors(false)
```

### Presets

```rust
// Lenient arithmetic
let config = EvaluationConfig::safe_arithmetic();

// Strict type checking
let config = EvaluationConfig::strict();
```

---

## NanHandling

How to handle non-numeric values in arithmetic.

```rust
pub enum NanHandling {
    ThrowError,    // Default: throw an error
    IgnoreValue,   // Skip non-numeric values
    CoerceToZero,  // Treat as 0
}
```

---

## DivisionByZero

How to handle division by zero.

```rust
pub enum DivisionByZero {
    ReturnBounds,  // Default: return Infinity/-Infinity
    ThrowError,    // Throw an error
    ReturnNull,    // Return null
}
```

---

## TruthyEvaluator

How to evaluate truthiness.

```rust
pub enum TruthyEvaluator {
    JavaScript,    // Default: JS-style truthiness
    Python,        // Python-style truthiness
    StrictBoolean, // Only true/false are valid
    Custom(Arc<dyn Fn(&Value) -> bool + Send + Sync>),
}
```

---

## Operator Trait

Interface for custom operators.

```rust
pub trait Operator: Send + Sync {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value>;
}
```

**Parameters:**
- `args` - Unevaluated arguments from the rule
- `context` - Current evaluation context
- `evaluator` - Interface to evaluate nested expressions

**Important:** Arguments are **unevaluated**. Call `evaluator.evaluate()` to resolve them.

---

## Evaluator Trait

Interface for evaluating expressions (used in custom operators).

```rust
pub trait Evaluator {
    fn evaluate(&self, value: &Value, context: &mut ContextStack) -> Result<Value>;
}
```

---

## ContextStack

Manages variable scope during evaluation.

### Accessing Current Element

In array operations (`map`, `filter`, `reduce`):

```rust
// Access current element
let current = context.current();

// Access current index
let index = context.index();

// Access accumulator (in reduce)
let acc = context.accumulator();
```

---

## Error

Error types returned by datalogic-rs.

```rust
pub enum Error {
    InvalidArguments(String),
    UnknownOperator(String),
    TypeError(String),
    DivisionByZero,
    Custom(String),
    // ... other variants
}
```

### Common Error Handling

```rust
use datalogic_rs::Error;

match engine.evaluate(&compiled, &data) {
    Ok(result) => println!("Result: {}", result),
    Err(Error::InvalidArguments(msg)) => eprintln!("Bad arguments: {}", msg),
    Err(Error::UnknownOperator(op)) => eprintln!("Unknown operator: {}", op),
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## Result Type

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

---

## Full Example

```rust
use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling, Error};
use serde_json::json;
use std::sync::Arc;

fn main() -> Result<(), Error> {
    // Create configured engine
    let config = EvaluationConfig::default()
        .with_nan_handling(NanHandling::IgnoreValue);
    let engine = Arc::new(DataLogic::with_config(config));

    // Compile rule
    let rule = json!({
        "if": [
            { ">=": [{ "var": "score" }, 60] },
            "pass",
            "fail"
        ]
    });
    let compiled = engine.compile(&rule)?;

    // Evaluate with different data
    let results: Vec<_> = vec![
        json!({ "score": 85 }),
        json!({ "score": 45 }),
        json!({ "score": 60 }),
    ].into_iter()
    .map(|data| engine.evaluate_owned(&compiled, data))
    .collect();

    for result in results {
        println!("{}", result?);
    }

    Ok(())
}
```
