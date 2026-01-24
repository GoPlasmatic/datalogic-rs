# Migration Guide

This guide covers migrating between major versions of datalogic-rs.

## v3 to v4 Migration

### Overview

v4 redesigns the API for ergonomics and simplicity. The core JSONLogic behavior is unchanged, but the Rust API is different.

**Key changes:**
- Simplified `DataLogic` engine API
- `CompiledLogic` automatically wrapped in `Arc`
- No more arena allocation (simpler lifetime management)
- New evaluation methods

### When to Migrate

**Migrate to v4 if:**
- Starting a new project
- Want simpler, more ergonomic API
- Don't need arena-based memory optimization
- Want easier thread safety

**Stay on v3 if:**
- Already using v3 in production with no issues
- Need maximum performance with arena allocation
- Have complex lifetime requirements

### API Changes

#### Engine Creation

```rust
// v3
use datalogic_rs::DataLogic;
let engine = DataLogic::default();

// v4
use datalogic_rs::DataLogic;
let engine = DataLogic::new();

// v4 with config
use datalogic_rs::{DataLogic, EvaluationConfig};
let engine = DataLogic::with_config(EvaluationConfig::default());
```

#### Compilation

```rust
// v3
let compiled = engine.compile(&logic)?;
// compiled is not automatically Arc-wrapped

// v4
let compiled = engine.compile(&logic)?;
// compiled is Arc<CompiledLogic>, thread-safe by default
```

#### Evaluation

```rust
// v3
let result = engine.evaluate(&compiled, &data)?;

// v4 - two options
// Option 1: Takes owned data, returns Value
let result = engine.evaluate_owned(&compiled, data)?;

// Option 2: Takes reference, returns Cow<Value>
let result = engine.evaluate(&compiled, &data)?;
```

#### Quick Evaluation

```rust
// v3
let result = engine.apply(&logic, &data)?;

// v4
let result = engine.evaluate_json(
    r#"{"==": [1, 1]}"#,
    r#"{}"#
)?;
```

#### Custom Operators

```rust
// v3
struct MyOperator;
impl Operator for MyOperator {
    fn evaluate(&self, args: &[Value], data: &Value, engine: &DataLogic) -> Result<Value> {
        // ...
    }
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
        // Arguments are unevaluated - call evaluator.evaluate() as needed
        let value = evaluator.evaluate(&args[0], context)?;
        // ...
    }
}
```

#### Thread Safety

```rust
// v3 - Manual Arc wrapping
use std::sync::Arc;
let compiled = engine.compile(&logic)?;
let compiled_arc = Arc::new(compiled);

// v4 - Already Arc-wrapped
let compiled = engine.compile(&logic)?; // Already Arc<CompiledLogic>
let compiled_clone = Arc::clone(&compiled);
```

### Configuration Changes

```rust
// v3
let engine = DataLogic::default();

// v4
use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling};

let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::IgnoreValue);
let engine = DataLogic::with_config(config);
```

### Structured Objects

```rust
// v3
let engine = DataLogic::with_preserve_structure(true);

// v4
let engine = DataLogic::with_preserve_structure();

// v4 with config
let config = EvaluationConfig::default();
let engine = DataLogic::with_config_and_structure(config, true);
```

### Error Handling

```rust
// v3
use datalogic_rs::Error;
match engine.evaluate(&compiled, &data) {
    Ok(result) => { /* ... */ }
    Err(Error::UnknownOperator(op)) => { /* ... */ }
    Err(e) => { /* ... */ }
}

// v4 - Same pattern
use datalogic_rs::Error;
match engine.evaluate_owned(&compiled, data) {
    Ok(result) => { /* ... */ }
    Err(Error::UnknownOperator(op)) => { /* ... */ }
    Err(e) => { /* ... */ }
}
```

### Migration Checklist

1. **Update Cargo.toml:**
   ```toml
   [dependencies]
   datalogic-rs = "4.0"
   ```

2. **Update engine creation:**
   - `DataLogic::default()` → `DataLogic::new()`

3. **Update evaluation calls:**
   - `engine.evaluate(&compiled, &data)` → `engine.evaluate_owned(&compiled, data.clone())`
   - Or use `engine.evaluate(&compiled, &data)` for reference-based evaluation

4. **Update custom operators:**
   - Add `context: &mut ContextStack` parameter
   - Replace `engine: &DataLogic` with `evaluator: &dyn Evaluator`
   - Call `evaluator.evaluate()` on arguments

5. **Remove manual Arc wrapping:**
   - `CompiledLogic` is now automatically `Arc<CompiledLogic>`

6. **Test thoroughly:**
   - Run your test suite
   - Verify expected behavior with your specific rules

### Performance Considerations

v4 trades some raw performance for a simpler API:

- No arena allocation means more heap allocations
- `Arc` wrapping adds a small overhead for single-threaded use
- For most use cases, the difference is negligible

If you need maximum performance:
- Reuse `CompiledLogic` instances
- Use `evaluate` with references for large data
- Consider staying on v3 for hot paths

### Getting Help

If you encounter issues during migration:

1. Check the [API Reference](api/reference.md)
2. Review the [examples](https://github.com/GoPlasmatic/datalogic-rs/tree/main/examples)
3. Open an issue on [GitHub](https://github.com/GoPlasmatic/datalogic-rs/issues)
