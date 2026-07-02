# Security and Sandboxing

datalogic-rs is designed to evaluate **untrusted rules over trusted data**:
rules submitted by users, stored in a database, or fetched from an API,
evaluated against data your application controls. This page states exactly
what that guarantees, what it does not, and how to run untrusted rules
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

The core crate is built with `#![forbid(unsafe_code)]`. The language
bindings necessarily cross an FFI boundary, so "no unsafe code" is a
property of the Rust engine, not of every binding shim.

## Determinism

Evaluation is deterministic given the same rule and data, with one
exception: the `now` operator (and any datetime arithmetic relative to it,
available under the `datetime` feature) reads the wall clock. If you need
fully reproducible evaluation, either avoid `now` or inject the current time
as input data instead. All other operators, including the flagd `fractional`
bucketing (a fixed murmurhash3), are pure functions of their arguments.

## Resource bounds that exist

| Bound | Default | What it protects |
|-------|---------|------------------|
| JSON parse depth | 256 | Parsing a rule or data **string** cannot overflow the stack. |
| Compile nesting depth | 256 | A programmatically-built rule (`IntoLogic` from an owned value, which skips the parser) cannot overflow the stack in compile, dispatch, or drop. Exceeding it is a `ConfigurationError`. |
| `max_recursion_depth` | 256 | Caps nested `Engine::evaluate` re-entry from custom operators that hold an `Arc<Engine>`. Configurable via `EvaluationConfig::with_max_recursion_depth`. Pure built-in workloads skip the check. |

Arena memory grows during a single evaluation and is released when the
arena is dropped (per-call tiers) or reset. In a long-running `Session`,
call `Session::reset()` between logical batches so peak memory tracks the
largest single evaluation rather than the cumulative loop.

## What is NOT bounded

The engine does **not** impose limits on, and has no built-in timeout or
cancellation for:

- **Wall-clock time / CPU.** A rule that iterates a large array or nests
  `map`/`reduce`/`filter` can run for a long time.
- **Iteration count.** `map`/`filter`/`reduce`/`all`/`some`/`none` process
  every element of whatever array they are given.
- **Output size.** A templating rule or `merge` can produce an output much
  larger than its input.

These are all functions of the **input data size** and the **rule
complexity**, both of which you control. Mitigate them at the edges:

1. **Bound attacker-controlled input.** Cap array lengths and total payload
   size before evaluating. This is the single most effective control,
   because iteration and output size scale with the data, not the rule.
2. **Bound rule complexity.** For user-authored rules, cap the serialized
   rule size and reject or lower `max_recursion_depth` / compile depth as
   appropriate for your risk tolerance.
3. **For hard wall-clock guarantees, isolate the evaluation.** Rust cannot
   safely abort a thread mid-computation, so a timeout that must interrupt a
   running evaluation needs process-level isolation (run evaluation in a
   subprocess or sandbox you can kill). For most workloads, input and
   complexity bounds are sufficient and far cheaper; reach for process
   isolation only when you must survive an adversarial worst case.

## Untrusted-rule checklist

- [ ] Compile rules once and reject the ones that fail to compile
      (`InvalidOperator`, malformed JSON, over-depth) before they reach a hot
      path.
- [ ] Size-limit the input data (array lengths, total bytes).
- [ ] Size-limit the rule text.
- [ ] Decide how `throw` should surface: a thrown error is a normal
      `Result::Err` (kind `Thrown`) carrying the thrown value, not a crash.
      Catch it if user rules are expected to throw.
- [ ] If you register custom operators, remember they run host-language
      code with host privileges; treat operator implementations as trusted,
      even when the rules that call them are not.
- [ ] Avoid `now` (or inject time as data) if you need reproducibility.

## Reporting a vulnerability

Please report suspected security issues privately rather than in a public
issue. See [`SECURITY.md`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/SECURITY.md)
for the disclosure process.
