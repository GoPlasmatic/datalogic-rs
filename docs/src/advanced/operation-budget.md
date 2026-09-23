# Operation Budget

*Requires the `budget` feature. Off by default; enabled in every
published binding.*

The engine has no built-in timeout, and a wall-clock one would not help
much: it is not deterministic, it fires after the work is done rather
than before, and it cannot tell you *which* rule was expensive. The
operation budget is the alternative: a counter the engine increments as
it works, and a ceiling it refuses to cross.

```rust,ignore
use datalogic_rs::{Engine, EvaluationConfig};

let engine = Engine::builder()
    .with_config(EvaluationConfig::default().with_ops_budget(Some(100_000)))
    .build();

// A tenant's rule over a tenant's data. Expensive rules are refused,
// not run.
match engine.eval_str(tenant_rule, payload) {
    Ok(result) => serve(result),
    Err(e) if e.tag() == "BudgetExceeded" => reject_as_too_expensive(e),
    Err(e) => report(e),
}
```

## Why a count and not a timeout

A count is **deterministic**: the same rule over the same data charges
the same number on every machine, so a rule that is accepted in staging
is accepted in production, and a rule that is refused is refused for
everyone. Wall-clock time depends on the machine, its load, and what
else the process is doing.

A count is also charged **before** the work. Every operator prices what
it is about to do and asks for it up front, so the engine refuses a rule
that would build a billion-element tensor instead of running it and
reporting afterwards.

And the failure is **attributable**: `BudgetExceeded` carries the node
breadcrumb like every other engine error, so you can point at the part of
the rule that went over.

A budget does *not* bound wall-clock time directly. It bounds
work, and work correlates with time; for a hard time guarantee you still
need process-level isolation (see
[Security and Sandboxing](security.md)).

## What one operation is

> "Nodes dispatched at runtime, plus whatever operators charge."

Concretely:

| Charged | Amount |
|---------|--------|
| Each node the engine dispatches | 1 |
| Each item an iterator examines (`map`, `filter`, `reduce`, the quantifiers, `sort`, …) | 1 per item, charged as soon as the source resolves |
| Each tensor operator | `max(elements read, elements produced)` |
| A custom operator | whatever it charges via `EvalContext::charge` |

And what is **not** charged:

- **Literals cost 0.** They return before dispatch; they are data the
  compiler already resolved, not work the rule asked for.
- **Constant-folded subtrees cost 0.** `{"+": [1, 2]}` is a literal by
  the time evaluation starts. Charging for it would make the count depend
  on whether folding was enabled.
- **A CSE-memoised subtree is charged once**, on the evaluation that
  fills the slot.

The per-item charge keeps the number honest. Several operators
recognise predicate and body shapes at compile time and evaluate them
inline without dispatching the body. Under a naive scheme those would
cost nothing per item, and whether a rule fitted its budget would depend
on which shape the compiler happened to recognise. Charging when the
source resolves means an iteration costs at least its input length
whichever path runs.

## Picking a number

The count is deterministic for a **pinned crate version**, not across
versions: a new fast path or fold changes what gets dispatched. Budget
for the work you want to allow, not for a number you measured.

To calibrate, meter your real rules against your real payloads, take the worst case, and leave generous headroom.

```rust,ignore
use bumpalo::Bump;
use datalogic_rs::{Engine, Metered};

let engine = Engine::new();
let compiled = engine.compile(rule)?;
let arena = Bump::new();

// `u64::MAX` meters without bounding.
let Metered { value, ops } = engine.evaluate_metered(&compiled, data, &arena, u64::MAX)?;
println!("{ops} operations");
```

`Session::eval_metered` is the same thing on a session's arena, and the
per-call `budget` argument overrides the engine-wide
`EvaluationConfig::ops_budget` for that call.

## The abort is final

`BudgetExceeded` is not recoverable. Once the counter is past its
ceiling every later charge fails too, and `try` propagates the error
instead of moving to its next arm:

```json
{"try": [{"map": [{"var": "huge"}, {"var": ""}]}, "fallback"]}
```

Under an exhausted budget this raises rather than returning
`"fallback"`. That is deliberate: a rule that could catch its own budget
failure could spend the budget in a loop.

Every other error stays catchable exactly as before: a budget changes
nothing about `throw`, `try`, or any other failure mode.

## Reaching it from a binding

Every binding accepts the budget as the `ops_budget` config key, which
bounds every evaluation that engine performs:

```js
// Node / WASM
const engine = new Engine({ config: { ops_budget: 100_000 } });
```

```python
engine = Engine(config={"ops_budget": 100_000})
```

```go
b := datalogic.NewEngineBuilder()
if err := b.SetConfigJSON(`{"ops_budget":100000}`); err != nil { /* ... */ }
```

```java
Engine engine = Engine.builder().setConfigJson("{\"ops_budget\":100000}").build();
```

```csharp
using var engine = Engine.Builder().SetConfigJson("""{"ops_budget":100000}""").Build();
```

```php
$engine = Engine::builder()->setConfigJson('{"ops_budget":100000}')->build();
```

The JS and Python bindings additionally expose **per-call metering**,
which reports the cost alongside the result:

```js
const { result, ops } = engine.evalMetered(rule, data);       // Node
const { result, ops } = JSON.parse(engine.evalMetered(rule, data)); // WASM
```

```python
result_json, ops = engine.eval_metered(rule, data)
```

Both take an optional third argument that overrides the configured
budget for one call. The C ABI (and the Go, JVM, .NET and PHP bindings
built on it) carry the engine-wide config key only; build a second
engine when you want two budgets.

## In the Studio

The [playground](../playground.md) exposes the budget under **Engine
settings → Operation budget**, and reports what every evaluation spent
as an *N ops* badge on the Result panel, so you can see the cost of a
rule while editing it, not only when it trips a ceiling. Setting a budget
that a rule crosses shows the `BudgetExceeded` error with its `budget`
and `spent` figures and highlights the node that went over.

## Pricing a custom operator

The dispatcher already charges 1 for the operator node itself, so an
operator whose work is bounded by a constant needs no charge at all. One
that walks a large input or builds a large result should price it
before allocating:

```rust,ignore
use datalogic_rs::{CustomOperator, DataValue, Result, operator::EvalContext};

struct Repeat;

impl CustomOperator for Repeat {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        ctx: &mut EvalContext<'_, 'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        let text = args.first().and_then(|v| v.as_str()).unwrap_or("");
        let times = args.get(1).and_then(|v| v.as_i64()).unwrap_or(0).max(0) as usize;
        // Price the output before allocating it.
        ctx.charge((text.len() * times) as u64)?;
        Ok(arena.alloc(DataValue::String(arena.alloc_str(&text.repeat(times)))))
    }
}
```

`charge` is always available: with the `budget` feature off it compiles
to `Ok(())`, so an operator can call it unconditionally rather than
carrying a `cfg` of its own.

Pick a unit that makes the operator's cost **proportional to its data**.
Elements touched is the usual choice. An operator whose work is *not*
proportional to its data would be under-priced by an unbounded ratio.
That is why the built-in tensor family is arithmetic-free: a
matmul reads 2n² elements and does n³ multiplies, so pricing it by data
moved would make the budget stop measuring anything.

## Cost

The feature is a Cargo flag rather than an always-on `Option<u64>`
because the add-and-compare per dispatched node is measurable. On the
self benchmark's full suite, with the feature compiled in and no budget
set, the geomean moves from 22.75 to 23.56 ns/op (+3.6%), measured as
paired runs on one machine, with the feature-off number unchanged from
before the feature existed. Builds that do not want a counter compile it out entirely: the
counter, the compare and the error variant all disappear.
