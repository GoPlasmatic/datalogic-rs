# Custom Operators

Extend datalogic-rs with your own operators to implement domain-specific logic.

## Basic Custom Operator

Custom operators implement the `Operator` trait:

```rust
use datalogic_rs::{DataLogic, Operator, ContextStack, Evaluator, Result, Error};
use serde_json::{json, Value};

struct DoubleOperator;

impl Operator for DoubleOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Arguments are unevaluated - must call evaluate() first!
        let value = evaluator.evaluate(
            args.first().unwrap_or(&Value::Null),
            context
        )?;

        match value.as_f64() {
            Some(n) => Ok(json!(n * 2.0)),
            None => Err(Error::InvalidArguments("Expected number".to_string()))
        }
    }
}
```

## Registering Custom Operators

Add custom operators to the engine before compiling rules:

```rust
let mut engine = DataLogic::new();
engine.add_operator("double".to_string(), Box::new(DoubleOperator));

// Now use it in rules
let rule = json!({ "double": 21 });
let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
assert_eq!(result, json!(42.0));
```

## Important: Evaluating Arguments

**Arguments passed to custom operators are unevaluated.** You must call `evaluator.evaluate()` to resolve them:

```rust
impl Operator for MyOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // WRONG: Using args directly
        // let value = args[0].as_f64();

        // CORRECT: Evaluate first
        let value = evaluator.evaluate(&args[0], context)?;
        let num = value.as_f64();

        // Now work with the evaluated value
        // ...
    }
}
```

This allows your operator to work with both literals and expressions:

```json
// Works with literals
{ "double": 21 }

// Also works with variables
{ "double": { "var": "x" } }

// And nested expressions
{ "double": { "+": [10, 5] } }
```

## Example: Average Operator

An operator that calculates the average of numbers:

```rust
struct AverageOperator;

impl Operator for AverageOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Evaluate the argument (should be an array)
        let value = evaluator.evaluate(
            args.first().unwrap_or(&Value::Null),
            context
        )?;

        let arr = value.as_array()
            .ok_or_else(|| Error::InvalidArguments("Expected array".to_string()))?;

        if arr.is_empty() {
            return Ok(Value::Null);
        }

        let sum: f64 = arr.iter()
            .filter_map(|v| v.as_f64())
            .sum();

        let count = arr.len() as f64;
        Ok(json!(sum / count))
    }
}

// Usage
engine.add_operator("avg".to_string(), Box::new(AverageOperator));

let rule = json!({ "avg": { "var": "scores" } });
let compiled = engine.compile(&rule).unwrap();
let result = engine.evaluate_owned(&compiled, json!({
    "scores": [80, 90, 85, 95]
})).unwrap();
assert_eq!(result, json!(87.5));
```

## Example: Range Check Operator

An operator that checks if a value is within a range:

```rust
struct InRangeOperator;

impl Operator for InRangeOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() != 3 {
            return Err(Error::InvalidArguments(
                "inRange requires 3 arguments: value, min, max".to_string()
            ));
        }

        let value = evaluator.evaluate(&args[0], context)?
            .as_f64()
            .ok_or_else(|| Error::InvalidArguments("Expected number".to_string()))?;

        let min = evaluator.evaluate(&args[1], context)?
            .as_f64()
            .ok_or_else(|| Error::InvalidArguments("Expected number".to_string()))?;

        let max = evaluator.evaluate(&args[2], context)?
            .as_f64()
            .ok_or_else(|| Error::InvalidArguments("Expected number".to_string()))?;

        Ok(json!(value >= min && value <= max))
    }
}

// Usage
engine.add_operator("inRange".to_string(), Box::new(InRangeOperator));

let rule = json!({ "inRange": [{ "var": "age" }, 18, 65] });
```

## Example: String Formatting Operator

```rust
struct FormatOperator;

impl Operator for FormatOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        let template = evaluator.evaluate(
            args.first().unwrap_or(&Value::Null),
            context
        )?;

        let template_str = template.as_str()
            .ok_or_else(|| Error::InvalidArguments("Expected string template".to_string()))?;

        // Replace {0}, {1}, etc. with arguments
        let mut result = template_str.to_string();
        for (i, arg) in args.iter().skip(1).enumerate() {
            let value = evaluator.evaluate(arg, context)?;
            let value_str = match &value {
                Value::String(s) => s.clone(),
                v => v.to_string(),
            };
            result = result.replace(&format!("{{{}}}", i), &value_str);
        }

        Ok(json!(result))
    }
}

// Usage
engine.add_operator("format".to_string(), Box::new(FormatOperator));

let rule = json!({
    "format": ["Hello, {0}! You have {1} messages.", { "var": "name" }, { "var": "count" }]
});
// Data: { "name": "Alice", "count": 5 }
// Result: "Hello, Alice! You have 5 messages."
```

## Thread Safety Requirements

Custom operators must be `Send + Sync` for thread-safe usage:

```rust
// This is automatically satisfied for most operators
struct MyOperator {
    // Use Arc for shared state
    config: Arc<Config>,
}

// For mutable state, use synchronization primitives
struct StatefulOperator {
    counter: Arc<AtomicUsize>,
}

impl Operator for StatefulOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        let count = self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(json!(count))
    }
}
```

## Error Handling

Return appropriate errors for invalid inputs:

```rust
impl Operator for MyOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        // Check argument count
        if args.is_empty() {
            return Err(Error::InvalidArguments(
                "myop requires at least one argument".to_string()
            ));
        }

        // Check argument types
        let value = evaluator.evaluate(&args[0], context)?;
        let num = value.as_f64().ok_or_else(|| {
            Error::InvalidArguments(format!(
                "Expected number, got {}",
                value_type_name(&value)
            ))
        })?;

        // Business logic errors
        if num < 0.0 {
            return Err(Error::Custom(
                "Value must be non-negative".to_string()
            ));
        }

        Ok(json!(num.sqrt()))
    }
}

fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
```

## Best Practices

1. **Always evaluate arguments** before using them
2. **Validate argument count and types** early
3. **Return meaningful error messages**
4. **Keep operators focused** - one responsibility per operator
5. **Document the expected syntax** for each operator
6. **Use `Arc` for shared configuration** to maintain thread safety
7. **Test with both literals and expressions** as arguments
