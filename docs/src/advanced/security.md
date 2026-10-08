# Security and Sandboxing

datalogic-rs targets **untrusted rules over trusted data**:
rules submitted by users, stored in a database, or fetched from an API,
evaluated against data your application controls. This page states what
that guarantees, what it does not, and how to run untrusted rules
safely.

## The sandbox model

A compiled rule is pure data. Evaluating it can only:

- read from the input data you pass in,
- compute with the built-in (and any custom) operators, and
- return a value.

A rule **cannot**:

- execute arbitrary code: there is no `eval`, no scripting runtime, no shell
  out. Operators are a fixed, compiled-in set (plus any custom operators you
  register in the host language),
- perform I/O: no file, network, environment, or clock access, with the
  single exception of the `now` operator (see Determinism below),
- reach outside the data you provide: `var` / `val` resolve against the
  input value and the active iteration scope only,
- mutate the input, the engine, or shared state: evaluation takes `&self`
  and returns a fresh value.

The core crate sets `#![forbid(unsafe_code)]`. The language
bindings cross an FFI boundary, so "no unsafe code" is a property of the
Rust engine, not of every binding shim.

## Determinism

Evaluation is deterministic given the same rule and data, with one
exception: the `now` operator (and any datetime arithmetic relative to it,
available under the `datetime` feature) reads the wall clock. If you need
reproducible evaluation, avoid `now` or pass the current time in as input
data. The other operators, including the flagd `fractional` bucketing (a
fixed murmurhash3), are pure functions of their arguments.

## Resource bounds that exist

| Bound | Default | What it protects |
|-------|---------|------------------|
| JSON parse depth | 256 | Parsing a rule or data **string** cannot overflow the stack. Deeper input is a `ParseError`. |
| Compile nesting depth | 256 | A programmatically-built rule (`IntoLogic` from an owned value, which skips the parser) cannot overflow the stack in compile, dispatch, or drop. Exceeding it is a `ConfigurationError`. |
| `max_recursion_depth` | 256 | Caps how many evaluations may run at once on one thread, counting the outermost. Only a custom operator that holds an `Arc<Engine>` and evaluates from inside its own `evaluate` can stack them. Exceeding it is a `ConfigurationError`. Set with `EvaluationConfig::with_max_recursion_depth`; `try_build` refuses 0. Engines without custom operators skip the check. |
| `reduce` accumulator nesting | 1,024 levels | A fold that wraps its accumulator each step (`{"reduce": [xs, [{"var": "accumulator"}], null]}`) cannot build a value deep enough to overflow the stack when the result is serialised, compared or copied. Passing it is `InvalidArguments`. |
| Tensor constructor size | 2^28 elements | `zeros`, `full`, `scatter`, `rle_expand`, `one_hot` and `pad` refuse to allocate more than 268,435,456 elements (2 GiB of `f64`) from the rule's dimensions. Passing it is `InvalidArguments`. |
| `ops_budget` | unset | Caps the **work** one evaluation may do: one operation per dispatched node, one per item an iterator examines, plus what operators charge for the data they move. Off unless you set it. Requires the `budget` feature (on in every published binding). |

Arena memory grows during a single evaluation and is released when the
arena is dropped (per-call tiers) or reset. In a long-running `Session`,
call `Session::reset()` between logical batches so peak memory tracks the
largest single evaluation rather than the cumulative loop.

## Bounding the work a rule does

Iteration count and output size are functions of the **input data size**
and the **rule complexity**. A `map` over a large array nested inside
another `map` does not touch the recursion cap: it is one boundary
call doing N x M items of work.

Set an **operation budget** to bound that:

```rust,ignore
use datalogic_rs::{Engine, EvaluationConfig};

let engine = Engine::builder()
    .with_config(EvaluationConfig::default().with_ops_budget(Some(100_000)))
    .build();
```

Every binding accepts the same thing as the `ops_budget` config key.
Crossing the ceiling fails the evaluation with `BudgetExceeded` **before**
the work is done, carrying the node breadcrumb, and a `try` inside the
rule cannot recover from it. The count is deterministic for a pinned
engine version, so a rule that is refused is refused identically on every
machine. See [Operation Budget](operation-budget.md) for what one
operation is and how to pick a number.

Belt and braces, in the order that buys the most:

1. **Bound attacker-controlled input.** Cap array lengths and total payload
   size before evaluating. Iteration and output size scale with the data,
   so this is the cheapest control and the one that fails earliest.
2. **Set an operation budget.** This control survives a rule designed to
   be expensive over input you thought was small enough.
3. **Bound rule complexity.** For user-authored rules, cap the serialized
   rule size, and lower `max_recursion_depth` if your custom operators
   re-enter the engine.

## Checking a rule before you store it

`Engine::compile` accepts a call to an unknown operator and leaves it to
fail when it runs, so a bad rule can sit in storage until some input
reaches the broken branch. `Engine::check` reports every problem the
engine can see before the rule runs, in every branch, each with an RFC
6901 JSON Pointer into the rule: unknown operators (with a "did you mean"
suggestion), argument counts an operator rejects, `and` / `or` / `if`
without an argument array, literal timezones that do not exist, and whatever a
custom operator's own `check` rejects. `Engine::compile_checked` compiles
only a rule with no error diagnostic and otherwise returns every
diagnostic in a `CompileError`. Use it where you accept rules from users.
See [Rule Analysis](rule-analysis.md#checking-a-rule-enginecheck).

A rule that reads a misspelled path evaluates the read to `null` under the
JSONLogic default. If your rules and data have a known shape, set
`EvaluationConfig::missing_var` to `MissingVar::Error` (config key
`"missing_var": "error"` in the bindings) and a read that finds nothing
fails with `VariableNotFound` naming the path. A read with a default
(`{"var": ["x", 0]}`), a present `null`, `missing`, `missing_some` and
`exists` are not misses, and `try` catches the error.

## Panics and the bindings

The engine reports a bad rule or bad input as an `Error`. For example, a
`format_date` pattern with a specifier chrono does not know (`"%Q"`) or a
trailing `%` is `InvalidArguments("Invalid date format")`, not a panic
inside chrono's formatter. If a panic does escape the engine, the Node
binding turns it into a thrown error (or a rejected promise) with `name` /
`errorType` `"InternalError"` instead of aborting the process, and the C
ABI that the Go, JVM, .NET and PHP bindings use catches it and returns
`DATALOGIC_STATUS_INTERNAL` with error tag `"InternalError"`.

A custom-operator callback in those C-ABI bindings must not evaluate on
the session that is running it. Such a call fails with
`DATALOGIC_STATUS_INVALID_ARG` (and `reset` / `free` on that session do
nothing until the callback returns). Open a second session, or evaluate
the rule without a session, for a nested evaluation.

## What is still NOT bounded

- **Wall-clock time / CPU.** An operation budget bounds *work*, and work
  correlates with time, but it is not a time limit: there is no built-in
  timeout or cancellation, and an individual operator's charge is taken
  before its work, not during it.
- **Work, if you leave `ops_budget` unset.** It is off by default; an
  engine without one iterates whatever array it is given.

For hard wall-clock guarantees, isolate the evaluation. Rust cannot safely
abort a thread mid-computation, so a timeout that must interrupt a running
evaluation needs process-level isolation (run evaluation in a subprocess or
sandbox you can kill). For most workloads a budget plus input bounds is
enough and far cheaper; reach for process isolation only when you must
survive an adversarial worst case.

## Untrusted-rule checklist

- [ ] Compile rules with `Engine::compile_checked` (or run `Engine::check`)
      when you accept them, and reject the ones with error diagnostics
      (unknown operators, wrong argument counts, malformed JSON, over-depth)
      before they reach a hot path.
- [ ] Size-limit the input data (array lengths, total bytes).
- [ ] Size-limit the rule text.
- [ ] Set an `ops_budget`. Meter your real rules against your real payloads
      to calibrate it, then leave generous headroom.
- [ ] Decide how `throw` should surface: a thrown error is a normal
      `Result::Err` (`ErrorCode::Thrown`) carrying the thrown value, not a
      crash. Catch it if user rules are expected to throw.
- [ ] If you register custom operators, remember they run host-language
      code with host privileges; treat operator implementations as trusted,
      even when the rules that call them are not.
- [ ] Consider `MissingVar::Error` if a misspelled path should fail
      instead of reading as `null`.
- [ ] Avoid `now` (or pass the time in as data) if you need reproducibility.

## Reporting a vulnerability

Please report suspected security issues privately rather than in a public
issue. See [`SECURITY.md`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/SECURITY.md)
for the disclosure process.
