# Custom Operators

Extend datalogic-rs with your own operators to implement domain-specific logic.

> **v5 changes:** Custom operators receive **pre-evaluated** `&DataValue<'a>`
> arguments and return arena-allocated values. The old "args are unevaluated;
> call `evaluator.evaluate()`" model is gone, and so is the `Evaluator` trait.
> The trait is named `CustomOperator`, and registration is **builder-only**.

## The CustomOperator Trait

```rust
use bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Result};

pub trait CustomOperator: Send + Sync {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>>;
}
```

| Parameter | What it is |
|-----------|------------|
| `args` | The operator's arguments **already evaluated** by the engine. Each `&'a DataValue<'a>` borrows from caller input or from earlier arena allocations. |
| `ctx` | Opaque view into the engine's evaluation context. Most operators ignore it; the read-only observations [`EvalContext::root_input`] and [`EvalContext::depth`] cover the rare cases where behaviour depends on the surrounding context. |
| `arena` | The `bumpalo::Bump` allocator for the current call. Use `arena.alloc(...)` for `DataValue`s and `arena.alloc_str(...)` for strings. |

The return value must live in the arena (or be a preallocated singleton like
`DataValue::Null`). Never return a stack reference.

## Basic Custom Operator

```rust
use bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Engine, Error, Result};

struct DoubleOperator;

impl CustomOperator for DoubleOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args
            .first()
            .and_then(|v| v.as_f64())
            .ok_or_else(|| Error::invalid_arguments("expected number"))?;
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}
```

## Registering Custom Operators

Operator registration is builder-only. Once the engine is built, its operator set is frozen and immutable.

Select your language to see how to register a custom operator:

<div class="codetabs">

```rust
// Rust
let engine = Engine::builder()
    .add_operator("double", DoubleOperator)
    .build();

let result = engine.eval_str(r#"{"double": 21}"#, r#"{}"#).unwrap();
assert_eq!(result, "42");
```

```javascript
// Node.js (native FFI): pass a { name: fn } map as the second constructor argument
import { Engine } from '@goplasmatic/datalogic-node';
const engine = new Engine({}, {
  double: (argsJson) => {
    const args = JSON.parse(argsJson);
    return JSON.stringify(args[0] * 2);
  }
});
// browser/edge: same callback shape via @goplasmatic/datalogic-wasm
// (customOperators constructor option), see the WASM chapter
```

```python
# Python
from datalogic_py import Engine
import json

engine = Engine(custom_operators={
    "double": lambda args_json: json.dumps(json.loads(args_json)[0] * 2)
})
```

```go
// Go
import (
    "encoding/json"
    "fmt"
    datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

engine := datalogic.NewEngineBuilder().
    AddOperator("double", func(argsJson string) (string, error) {
        var args []float64
        if err := json.Unmarshal([]byte(argsJson), &args); err != nil {
            return "", err
        }
        return fmt.Sprintf("%g", args[0]*2), nil
    }).
    Build()
defer engine.Close()
```

```java
// Java (FFM)
import com.goplasmatic.datalogic.Engine;

// argsJson is a JSON array string; parse with your JSON library (Jackson shown)
try (Engine engine = Engine.builder()
        .addOperator("double", argsJson -> {
            int n = mapper.readTree(argsJson).get(0).asInt();
            return String.valueOf(n * 2);
        })
        .build()) {
    System.out.println(engine.apply("{\"double\": [21]}", "{}")); // "42"
}
```

```csharp
// C# / .NET
using Goplasmatic.Datalogic;

using var engine = Engine.Builder()
    .AddOperator("double", argsJson =>
    {
        var n = System.Text.Json.Nodes.JsonNode.Parse(argsJson)![0]!.GetValue<double>();
        return (n * 2).ToString();
    })
    .Build();
Console.WriteLine(engine.Apply("""{"double": [21]}""", "{}")); // "42"
```

```php
// PHP
use Goplasmatic\Datalogic\Engine;

$engine = Engine::builder()
    ->addOperator('double', function (string $argsJson): string {
        $args = json_decode($argsJson, true);
        return (string) ((int) $args[0] * 2);
    })
    ->build();
echo $engine->apply('{"double": [21]}', '{}'); // "42"
```

</div>

## Reading Argument Types

`DataValue<'a>` is the arena-resident value tree, re-exported from the
[`datavalue`](https://docs.rs/datavalue-rs) crate. Common accessors:

```rust
match args[0] {
    DataValue::Null => { /* ... */ }
    DataValue::Bool(b) => { /* ... */ }
    DataValue::Number(_) => {
        let n: Option<f64> = args[0].as_f64();
        let i: Option<i64> = args[0].as_i64();
    }
    DataValue::String(s) => { /* &str */ }
    DataValue::Array(items) => { /* &[DataValue<'a>] */ }
    DataValue::Object(pairs) => { /* &[(&str, DataValue<'a>)] */ }
    _ => {}
}
```

## Example: Average Operator

```rust
use bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Engine, Result};

struct AverageOperator;

impl CustomOperator for AverageOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let mut numbers: Vec<f64> = Vec::new();
        for av in args {
            match av {
                DataValue::Array(items) => {
                    for it in items.iter() {
                        if let Some(n) = it.as_f64() {
                            numbers.push(n);
                        }
                    }
                }
                other => {
                    if let Some(n) = other.as_f64() {
                        numbers.push(n);
                    }
                }
            }
        }

        if numbers.is_empty() {
            return Ok(arena.alloc(DataValue::Null));
        }

        let avg = numbers.iter().sum::<f64>() / numbers.len() as f64;
        Ok(arena.alloc(DataValue::from_f64(avg)))
    }
}

let engine = Engine::builder().add_operator("avg", AverageOperator).build();

let result = engine.eval_str(
    r#"{"avg": {"var": "scores"}}"#,
    r#"{"scores": [80, 90, 85, 95]}"#,
).unwrap();
assert_eq!(result, "87.5");
```

## Example: Range Check Operator

```rust
struct InRangeOperator;

impl CustomOperator for InRangeOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        if args.len() != 3 {
            return Err(Error::invalid_arguments(
                "in_range requires 3 arguments: value, min, max",
            ));
        }
        let v = args[0].as_f64()
            .ok_or_else(|| Error::invalid_arguments("value must be a number"))?;
        let lo = args[1].as_f64()
            .ok_or_else(|| Error::invalid_arguments("min must be a number"))?;
        let hi = args[2].as_f64()
            .ok_or_else(|| Error::invalid_arguments("max must be a number"))?;
        Ok(arena.alloc(DataValue::Bool(v >= lo && v <= hi)))
    }
}

let engine = Engine::builder()
    .add_operator("in_range", InRangeOperator)
    .build();
```

## Example: String Formatting Operator

```rust
struct FormatOperator;

impl CustomOperator for FormatOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        let template = args
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_arguments("expected string template"))?;

        let mut out = template.to_string();
        for av in args.iter().skip(1) {
            if let Some(pos) = out.find("{}") {
                let replacement = match av {
                    DataValue::String(s) => (*s).to_string(),
                    DataValue::Bool(b) => b.to_string(),
                    DataValue::Null => "null".to_string(),
                    DataValue::Number(_) => av.as_f64()
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                    _ => "<value>".to_string(),
                };
                out.replace_range(pos..pos + 2, &replacement);
            }
        }

        // Allocate the rendered string in the arena and wrap it.
        let s = arena.alloc_str(&out);
        Ok(arena.alloc(DataValue::String(s)))
    }
}

let engine = Engine::builder()
    .add_operator("format", FormatOperator)
    .build();

let r = engine.eval_str(
    r#"{"format": ["Hello, {}! You have {} messages.", {"var": "name"}, {"var": "count"}]}"#,
    r#"{"name": "Alice", "count": 5}"#,
).unwrap();
// "Hello, Alice! You have 5 messages."
```

## Thread Safety Requirements

`CustomOperator` is `Send + Sync`. For shared mutable state, use the usual
synchronisation primitives:

```rust
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

struct CounterOperator { counter: Arc<AtomicUsize> }

impl CustomOperator for CounterOperator {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        let count = self.counter.fetch_add(1, Ordering::SeqCst) as i64;
        Ok(arena.alloc(DataValue::from_i64(count)))
    }
}
```

## Error Handling

Return appropriate errors for invalid inputs:

```rust
impl CustomOperator for MyOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        if args.is_empty() {
            return Err(Error::invalid_arguments(
                "myop requires at least one argument",
            ));
        }

        let num = args[0].as_f64().ok_or_else(|| {
            Error::type_error(format!("expected number, got {}", value_type_name(args[0])))
        })?;

        if num < 0.0 {
            return Err(Error::custom_message("value must be non-negative"));
        }

        Ok(arena.alloc(DataValue::from_f64(num.sqrt())))
    }
}

fn value_type_name(v: &DataValue<'_>) -> &'static str {
    match v {
        DataValue::Null => "null",
        DataValue::Bool(_) => "boolean",
        DataValue::Number(_) => "number",
        DataValue::String(_) => "string",
        DataValue::Array(_) => "array",
        DataValue::Object(_) => "object",
        _ => "other",
    }
}
```

The `Error` type is structured: `tag()` returns a stable variant tag,
and the `operator` / `path` fields are populated automatically by the engine
when a custom operator returns an error.

To wrap a foreign error type into `Error`, use `Error::wrap`:

```rust
"not_a_number".parse::<i32>().map_err(Error::wrap)?;
// `error.source()` returns the original `ParseIntError`.
```

## Best Practices

1. **Validate argument count and types early.**
2. **Allocate results in the arena** (`arena.alloc(...)` / `arena.alloc_str(...)`).
3. **Return meaningful errors** — `Error::invalid_arguments`, `Error::type_error`, `Error::custom_message`, `Error::wrap`.
4. **Keep operators focused** — one responsibility per operator.
5. **Use `Arc` for shared configuration** to maintain `Send + Sync`.
6. **Test with literals, variables, and nested expressions** — the engine evaluates each before calling you.
