# Custom Operators

Extend datalogic-rs with your own operators to implement domain-specific logic.

A custom operator receives its arguments **already evaluated**, as
`&DataValue<'a>` borrows, and returns a value allocated in the arena. You
register it on the `EngineBuilder`; a built engine's operator set is
fixed. Porting a 4.x operator? See
[Migration Guide: custom operators](../migration.md#custom-operators).

## The CustomOperator Trait

```rust
use bumpalo::Bump;
use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperatorInfo, DataValue, Diagnostic, Result};

pub trait CustomOperator: Send + Sync {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>>;

    // Default: `CustomOperatorInfo::opaque()`. See "Declaring what the engine may assume".
    fn info(&self) -> CustomOperatorInfo { /* ... */ }

    // Default: accepts every call. See "Validating calls before they run".
    fn check(&self, args: &[OwnedDataValue]) -> std::result::Result<(), Diagnostic> { /* ... */ }
}
```

Only `evaluate` is required. Within 5.x the trait gains default methods
only, so an operator written against 5.0 compiles against every 5.x
release.

| Parameter | What it is |
|-----------|------------|
| `args` | The operator's arguments **already evaluated** by the engine. Each `&'a DataValue<'a>` borrows from caller input or from earlier arena allocations. |
| `ctx` | Opaque view into the engine's evaluation context. Most operators ignore it. `root_input()` and `depth()` are read-only observations for the cases where behaviour depends on the surrounding context, and `charge(n)` prices work against the [operation budget](operation-budget.md#pricing-a-custom-operator). |
| `arena` | The `bumpalo::Bump` allocator for the current call. Use `arena.alloc(...)` for `DataValue`s and `arena.alloc_str(...)` for strings, or the one-call helpers on [`ArenaExt`](../rust/api-reference.md#arenaext). |

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

Operator registration is builder-only. Once you build the engine, its operator set is frozen.

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

engine, err := datalogic.NewEngineBuilder().
    AddOperator("double", func(argsJson string) (string, error) {
        var args []float64
        if err := json.Unmarshal([]byte(argsJson), &args); err != nil {
            return "", err
        }
        return fmt.Sprintf("%g", args[0]*2), nil
    }).
    Build()
if err != nil {
    panic(err)
}
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

### Names a built-in already answers to

Built-ins win: `add_operator` accepts an operator registered as `if`,
`var` or any other built-in name (aliases included), and that operator
never runs. `try_add_operator` refuses such a name instead, with a
`ConfigurationError` naming the built-in that would win:

```rust
let engine = Engine::builder()
    .try_add_operator("double", DoubleOperator)?   // a free name
    .build();

assert!(Engine::builder().try_add_operator("if", DoubleOperator).is_err());
```

The check follows the builder's [operator families](configuration.md#operator-families):
only operators of the families the engine has count, so `now` is a free
name without the `datetime` feature or with `DateTime` left out of
`with_families`. It also refuses a name that begins with the
[template key escape](configuration.md#combining-with-templating-mode),
which a template reads as an output field.

The check reads the builder's settings when you call it. If you set
`with_families` or `with_template_key_escape` after `try_add_operator`,
build with `try_build`, which checks every name `try_add_operator` took
against the final settings (and refuses a `max_recursion_depth` of 0).
`build` does neither. `check_operator_name(name)` gives the same refusal
without registering anything or consuming the builder.

In the bindings the same refusal is an option: `strictOperatorNames`
(Node, WASM), `strict_operator_names` (Python), `StrictOperatorNames`
(Go), `withStrictOperatorNames` (JVM, PHP) and `WithStrictOperatorNames`
(.NET).

### One operator on several engines

`Arc<T>` implements `CustomOperator` for any `T: CustomOperator + ?Sized`,
`Arc<dyn CustomOperator>` included, so a host that builds several engines
(one per tenant, say) can keep each operator in an `Arc` and register a
clone on every builder. The engines share the one instance and its state:

```rust
let registry: Vec<(&str, Arc<dyn CustomOperator>)> = vec![
    ("double", Arc::new(DoubleOperator)),
];

let build = || {
    registry
        .iter()
        .fold(Engine::builder(), |b, (name, op)| b.add_operator(*name, Arc::clone(op)))
        .build()
};
let engine = build();
```

`Box<dyn CustomOperator>` implements the trait as well, for registries
that hold boxed operators.

### Rebuilding an engine

`Engine::to_builder()` starts a builder from a running engine: the same
custom operators, config, templating mode, template key escape, folding
setting and families. Add or replace what differs and build; the running
engine is untouched. Both engines hold the same operator instances, so a
host that rebuilds on every configuration reload does not register its
operators again:

```rust
let running = Engine::builder()
    .add_operator("double", DoubleOperator)
    .build();

let reloaded = running
    .to_builder()
    .add_operator("avg", AverageOperator)
    .build();

assert!(reloaded.has_custom_operator("double"));
```

A rule compiled on one engine still evaluates on another: it finds a
custom operator by name there. On the engine that compiled it, the call
goes straight to the operator's slot.

## Declaring what the engine may assume

By default the engine assumes nothing about a custom operator: it may
return a different result on each call, read the whole data context, and
take any number of arguments. Override `info` to declare more, as a
`CustomOperatorInfo`:

```rust
use datalogic_rs::CustomOperatorInfo;

impl CustomOperator for DoubleOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }

    // A function of its one argument alone.
    fn info(&self) -> CustomOperatorInfo {
        CustomOperatorInfo::pure().with_args(1, Some(1))
    }
}

let engine = Engine::builder().add_operator("double", DoubleOperator).build();

// Deterministic, reads no context, constant argument: folded at compile time.
assert!(engine.compile(r#"{"double": 21}"#).unwrap().is_constant());
// Outside the declared count: InvalidArguments before any argument runs.
assert!(engine.eval_str(r#"{"double": [1, 2]}"#, "{}").is_err());
```

`CustomOperatorInfo::opaque()` is the default; `pure()` is deterministic
and reads no context. Chain `reading_context()` and `with_args(min, max)`
(`max` of `None` for no limit) onto either. The struct is
`#[non_exhaustive]`, so build it through these methods.

What the declaration changes:

- **Constant folding.** A deterministic operator that does not read the
  context, called with constant arguments, is evaluated once when the
  rule compiles and replaced by its result, like a built-in.
- **Rule facts.** [`Logic::facts()`](rule-analysis.md#what-a-rule-reads-logicfacts)
  trusts it: `is_deterministic()` holds for a deterministic operator, and
  `reads_complete()` for one that does not read the context.
- **Argument count.** A call outside the declared count fails with
  `InvalidArguments` before any argument is evaluated, and `Engine::check`
  reports it as an `ArgumentCount` error.

The engine reads the declaration when it compiles a call, so keep it
fixed for the operator's lifetime. A wrong declaration gives wrong
results: an operator that declares itself deterministic but is not gets
folded to whatever it returned at compile time.
`engine.custom_operator_info(name)` returns what a registered operator
declares.

## Validating calls before they run

Override `check` to reject a call from its arguments as written.
`Engine::check` and `Engine::compile_checked` call it for each call whose
argument count fits `info`; `compile` and evaluation do not.

`args` is the call's argument list as JSON: `{"op": [a, b]}` gives
`[a, b]`, and a lone non-array argument `{"op": a}` gives `[a]`. An
argument that is an expression arrives as written (`{"var": "x"}`), so
only a literal can be checked as a value. Return `Diagnostic::error` for
a call that will fail or `Diagnostic::warning` for one that runs but
probably not as meant, optionally narrowed with `at_argument(i)`; the
checker fills in the JSON Pointer and the operator name.

```rust
use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::{CheckMode, Diagnostic};

struct Table;

impl CustomOperator for Table {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        _arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(args[0])
    }

    fn check(&self, args: &[OwnedDataValue]) -> std::result::Result<(), Diagnostic> {
        match args.first() {
            Some(OwnedDataValue::String(name)) if name != "users" => {
                Err(Diagnostic::error(format!("no table {name:?}")).at_argument(0))
            }
            _ => Ok(()),
        }
    }
}

let engine = Engine::builder().add_operator("table", Table).build();
let diags = engine.check(r#"{"table": ["orders"]}"#, CheckMode::Engine);
assert_eq!(diags[0].pointer, "/table/0");
```

The diagnostic carries the code `DiagnosticCode::OperatorCheck`. See
[Rule Analysis](rule-analysis.md) for the rest of the checker.

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

`Error` is structured: `code()` returns its `ErrorCode` and `tag()` the
stable name of that code. When a custom operator returns an error, the
engine fills in `operator()` with the custom operator's name, even when it
sits inside built-ins, and `node_ids()` with the breadcrumb, which
`resolve_path(&compiled)` turns into a source path.

To wrap a foreign error type into `Error`, use `Error::wrap`:

```rust
"not_a_number".parse::<i32>().map_err(Error::wrap)?;
// `error.source()` returns the original `ParseIntError`.
```

## Best Practices

1. **Declare the argument count in `info`**, so a bad call fails before
   its arguments run and `check` reports it; validate types early in
   `evaluate`.
2. **Allocate results in the arena** (`arena.alloc(...)` / `arena.alloc_str(...)`).
3. **Return meaningful errors**: `Error::invalid_arguments`, `Error::type_error`, `Error::custom_message`, `Error::wrap`.
4. **Keep operators focused**: one responsibility per operator.
5. **Use `Arc` for shared configuration** to maintain `Send + Sync`.
6. **Test with literals, variables, and nested expressions**: the engine evaluates each before calling you.
