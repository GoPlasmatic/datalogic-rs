# Operation Budget

*Requires the `budget` feature. Off by default; enabled in every
published binding.*

The engine has no built-in timeout. A wall-clock one would not be
deterministic, would fire after the work is done rather than before, and
could not tell you *which* rule was expensive. The operation budget is a
counter the engine increments as it works, and a ceiling it refuses to
cross.

```rust,ignore
use datalogic_rs::{Engine, ErrorCode, EvaluationConfig};

let engine = Engine::builder()
    .with_config(EvaluationConfig::default().with_ops_budget(Some(100_000)))
    .build();

// A tenant's rule over a tenant's data. Expensive rules are refused,
// not run.
match engine.eval_str(tenant_rule, payload) {
    Ok(result) => serve(result),
    Err(e) if e.code() == ErrorCode::BudgetExceeded => reject_as_too_expensive(e),
    Err(e) => report(e),
}
```

`ErrorKind::BudgetExceeded` and `ErrorCode::BudgetExceeded` are present
in every build, with or without the `budget` feature, so a library can
match on them without mirroring the feature set of whoever builds the
engine. Only the `budget` feature raises the error. The variant carries
`budget` (the ceiling) and `spent` (what the rule had asked for when it
crossed it).

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

A budget bounds work, and work correlates with wall-clock time without
bounding it. For a hard time guarantee you still need process-level
isolation (see [Security and Sandboxing](security.md)).

## What one operation is

> "Nodes dispatched at runtime, plus whatever operators charge."

In detail:

| Charged | Amount |
|---------|--------|
| Each node the engine dispatches | 1 |
| Each item an iterator examines (`map`, `filter`, `reduce`, the quantifiers, `sort`, …), over an array or an object | 1 per item or key, charged as soon as the source resolves |
| Each item a collection operator copies, examines or compares | 1 per item, charged before the work (see below) |
| Each whole 64 bytes of string a string operator reads | 1 (see below) |
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

The collection operators whose work grows with an argument charge for it:

| Operator | Charge on top of its node |
|----------|---------------------------|
| `merge` | 1 per item of each array argument (scalars and `null` cost nothing extra) |
| `in` with an array haystack | the haystack's length, whether or not the needle is found |
| `missing`, `missing_some` with a path list from data | 1 per path (literal paths cost nothing extra) |
| `keys`, `values`, `entries` | 1 per key |
| `slice` of an array with a step other than 1 | 1 per item produced (a contiguous slice borrows its source and costs nothing extra) |
| `distinct`, `group_by` | 1 per comparison: each item is compared with every value or group kept so far, so n distinct values cost n(n-1)/2 |
| `sort` | n·⌈log₂ n⌉ comparisons |
| `+`, `-`, `*`, `/`, `%` over one array argument | 1 per item |
| `==`, `===`, `!=`, `!==`, `in`, `switch` comparing two arrays or two objects | 1 per array element, and \|a\|×\|b\| per object level (object equality finds each key by scanning the other side), for each level the comparison reaches; containers of different lengths cost nothing extra |

Without these, an accumulator does quadratic work for a linear count:

```json
{"reduce": [{"var": "xs"}, {"merge": [{"var": "accumulator"}, [{"var": "current"}]]}, []]}
```

copies the whole accumulator on every step, n(n+1)/2 items over n
inputs, and is charged for each of them.

String operators charge 1 per whole 64 bytes of string they read, so a
string shorter than 64 bytes, which is nearly every string a rule
touches, costs nothing extra:

| Operator | Bytes charged |
|----------|---------------|
| `cat` | each piece it appends (an array or object argument is charged its JSON text), plus 1 per item of an array argument |
| `substr`, `upper`, `lower`, `trim`, `length`, `slice` of a string | the input string |
| `in` with a string haystack | the haystack |
| `starts_with`, `ends_with` | the shorter of the string and the prefix or suffix |
| `split` | the input string, plus 1 per part produced |

So a `cat` accumulator, `{"reduce": [xs, {"cat": [{"var": "accumulator"}, …]}, ""]}`,
is charged for the accumulated string on every step. A unit of 64 bytes
leaves ordinary rules' counts at their node counts while a megabyte of
string still costs about 16,000.

Not priced beyond their node: field lookups (`var`, `val`, `exists`)
on wide objects, which may scan the object's keys; the datetime and
`flagd` parsers; a custom truthiness function, which receives an owned
copy of each value it tests; and a traced run's per-step context
snapshots.

`filter`, `all`, `some`, `none`, `map`, `reduce` and `sort` recognise
some predicate and body shapes (a comparison on a field, `+` on a field
and a literal) and evaluate them inline without dispatching the body. Those fast paths charge what the general path charges for the same
data, so a count does not depend on which shape the compiler recognised,
on the data's types, on whether the run is traced, or on
`EvaluationConfig::missing_var`.

**Behaviour change in 5.8.0:** the fast paths used to charge less, and
counts for those shapes rose to match the general path. Re-meter a budget
you calibrated on 5.7 or earlier.

Each built-in's cost class, what its charge grows with, is in the
operator catalogue: `Engine::operators()` reports `cost` as `"node"`
(constant), `"bytes"`, `"per_item"`, `"n_log_n"`, `"quadratic"` or
`"elements"`. See [Rule Analysis](rule-analysis.md#the-operator-catalogue-engineoperators).

## Picking a number

The count is deterministic for a **pinned crate version**, not across
versions: a new fast path or fold changes what gets dispatched. Budget
for the work you want to allow, not for a number you measured.

To calibrate, meter your real rules against your real payloads, take the
worst case, and leave generous headroom.

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
`"fallback"`, so a rule cannot catch its own budget failure and spend the
budget again in a loop.

A budget changes nothing about `throw`, `try` or any other error: they
stay catchable.

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

Every binding also has **per-call metering**, which reports the cost
alongside the result:

```js
const { result, ops } = engine.evalMetered(rule, data);       // Node
const { result, ops } = JSON.parse(engine.evalMetered(rule, data)); // WASM
```

| Binding | Per-call metering | Returns |
|---------|-------------------|---------|
| Node | `engine.evalMetered(rule, data, budget?)`, `rule.evaluateMetered(data, budget?)` | `{ result, ops }`, `result` as JSON text |
| WASM | `engine.evalMetered(..)`, `rule.evaluateMetered(..)`, `session.evaluateMetered(rule, data, budget?)` | JSON text `{"result": .., "ops": ..}` |
| Python | `engine.eval_metered(rule, data, budget=None)`, `rule.evaluate_metered(data, budget=None)` | `(result_json, ops)` |
| Go | `session.EvaluateMetered(rule, dataJSON, budget)` | `(string, uint64, error)` |
| JVM | `session.evaluateMetered(rule, dataJson, budget)` | `Metered(value, ops)` |
| .NET | `session.EvaluateMetered(rule, dataJson, budget = 0)` | `MeteredResult(Value, Ops)` |
| PHP | `$session->evaluateMetered($rule, $dataJson, $budget = 0)` | `['value' => .., 'ops' => ..]` |
| C | `datalogic_session_evaluate_metered(..., budget, ..., &ops, &err)` | status, result and `ops` |

The budget argument overrides the configured budget for that call. In
JavaScript and Python you leave it out to use the engine's budget, and a
budget below 1 is refused. Across the C ABI (Go, JVM, .NET, PHP), `0`
means the engine's configured budget, or unbounded when it has none.

## In the Studio

The [playground](../playground.md) exposes the budget under **Engine
settings → Operation budget**, and reports what each evaluation spent
as an *N ops* badge on the Result panel, so you can see the cost of a
rule while editing it, not only when it trips a ceiling. Setting a budget
that a rule crosses shows the `BudgetExceeded` error with its `budget`
and `spent` figures and highlights the node that went over.

## Pricing a custom operator

The dispatcher charges 1 for the operator node itself, so an operator
whose work is bounded by a constant needs no charge. One that walks a
large input or builds a large result should price it before allocating:

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

`charge` exists in every build: with the `budget` feature off it
compiles to `Ok(())`, so an operator can call it without a `cfg` of its
own.

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
set, the geomean moved from 22.75 to 23.56 ns/op (+3.6%) when the feature
was introduced, measured as paired runs on one machine. Builds without the
feature have no counter and no compare; `ErrorKind::BudgetExceeded` stays
in the enum but nothing raises it.
