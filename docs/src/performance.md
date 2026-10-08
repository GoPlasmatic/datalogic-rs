# Performance

This page covers where datalogic-rs spends time, how to benchmark it, and
how to keep evaluation fast.

## The headline numbers

<!-- canonical-bench v5.1 -->
Geomean execution time across 51 benchmark suites (Apple M2 Pro; median of 3 samples; ratios are pairwise shared-suite geomeans; methodology in the [benchmark matrix](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)):

```text
datalogic-rs (native Rust)              | 10.3 ns  (■) 1x
json-logic-engine (JS, compiled)        | 63.3 ns  (■■■■■■) 7.0x
json-logic-engine (JS, interpreted)     | 234.8 ns (■■■■■■■■■■■■■■■■■■■■■■■) 25.8x
jsonlogic-rs (bestowinc Rust engine)    | 264.2 ns (■■■■■■■■■■■■■■■■■■■■■■■■■■) 28.1x
json-logic-js (Reference JS library)    | 465.1 ns (■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■) 83.6x
```

The WASM build under Node measures 900.5 ns geomean (88× native): on Node servers, prefer `@goplasmatic/datalogic-node`. Reproduce it yourself with `cargo run --release -p datalogic-bench --bin compare`; positioning against each alternative is on [How It Compares](comparison.md).

## Performance Characteristics

### Compilation vs Evaluation

datalogic-rs uses a two-phase approach:

1. **Compilation** (slower): Parse and optimize the JSONLogic expression
2. **Evaluation** (faster): Execute compiled logic against data

**Best practice:** compile once, evaluate many times.

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(rule_json).unwrap();

let mut session = engine.session();
for data in datasets {
    session.eval_str(&compiled, data)?;
}
```

### OpCode Dispatch

The compiler resolves each operator name once, so evaluating on the
compiling engine does not look an operator up by its string:

- Each of the 84 built-in operators compiles to an `OpCode` that dispatch
  matches on.
- A custom operator call compiles to the operator's slot on the engine
  that compiled the rule, and dispatch indexes that slot. A rule calling
  eight custom operators measured 181 ns per evaluation with the earlier
  name lookup and 75 ns with slot dispatch. Evaluated on a different
  engine, the rule finds that engine's operator by name.

### Memory Efficiency

- **Arena allocation**: `&DataValue<'a>` results live in a `bumpalo::Bump`
  for one evaluation. Read-through ops like `var` borrow zero-copy from the
  caller's input.
- **Reusable arenas**: `Session` reuses one `Bump` across calls; you call
  `session.reset()` between batches so peak memory tracks the largest
  single evaluation rather than the sum.
- **Pre-built literal singletons**: trivial literals (`Null`, `Bool`,
  empty primitives) are static and need no per-call allocation.
- **`Arc<Logic>`**: cheap clone for cross-thread sharing.

### Input Projection

Before a rule can read an owned or `serde_json` input, the engine builds
an arena view of it. When a compiled rule's reads are all known
(`Logic::facts()` reports `reads_complete()`) and none of them is the
whole input, the view covers those paths and nothing else: on the way
down each path, only the object keys it names, and the whole value at its
end. Results are the same either way. Reading one field of an 8 MB
context took 36 ns from an `OwnedDataValue` and 41 ns from a
`serde_json::Value`, against 2.96 ms and 15.7 ms for a view of the whole
input; on a 1 KB context projection was 3 to 12 times faster.

Projection applies to `&OwnedDataValue`, `&serde_json::Value` and `Roots`
input, through `Engine::evaluate`, a `Session`, the one-shot
`Engine::eval*` methods and the top-level `eval*` functions. JSON text and
`ParsedData` are parsed straight into the arena and see no change. The
engine views the whole input when the rule:

- reads a computed path (`{"var": {"cat": [..]}}`),
- calls a custom operator that may read the context (the default unless
  its `info()` declares otherwise; see
  [Custom Operators](advanced/custom-operators.md)),
- reads the whole input (`{"var": ""}`),
- runs on an engine other than the one that compiled it, or
- runs traced.

[Rule Analysis](advanced/rule-analysis.md#what-a-rule-reads-logicfacts)
covers `Logic::facts()`. `cargo run --release -p datalogic-bench --bin
projection` reproduces the numbers.

### Iterator Fast Paths

Some iterator bodies run as a loop specialised to their shape, reading
fields straight from each element instead of pushing a frame and
dispatching the body per item:

| Operator | Body shape |
|----------|------------|
| `filter`, `all`, `some`, `none` | a comparison of the element or one of its fields with a literal (`{">": [{"var": "score"}, 50]}`), `and` / `or` / `!` / `!!` trees of those, a bare field, or `in` against a list of strings; for `filter`, also `===` / `!==` of a field and a value that does not change per item |
| `map` | a field, or `+` / `-` / `*` of a field and a literal or of two fields |
| `reduce` | `+` / `-` / `*` of `current` (or `current.path`) and `accumulator`, including a `reduce` over such a `map` |
| `sort` | a key that is a field of the element |

The predicate shape is chosen once per call, not once per item: over 1,000
items, `{"filter": [xs, {">": [{"var": ""}, 500]}]}` measured 1.17 µs.
These paths give the results the general path gives, fall back to it when
a value has the wrong type, and under an
[operation budget](advanced/operation-budget.md) charge what it charges.
They stand aside when the run is traced and on an engine configured with
`MissingVar::Error`, where a missing field must raise an error that an
inline read cannot.

## Benchmarking

### Running Benchmarks

The benchmark harness lives in its own dev-only crate, `datalogic-bench`,
under `tools/benchmark/`. Its binaries share a common harness; these two
cover most uses:

```bash
# Single-engine benchmark (datalogic-rs alone, fast arena path)
cargo run --release -p datalogic-bench --bin self
cargo run --release -p datalogic-bench --bin self -- --all   # every suite + JSON report

# Cross-library comparison (only datalogic-rs ships by default; see
# tools/benchmark/README.md for adding more subjects)
cargo run --release -p datalogic-bench --bin compare -- --all
```

Reports land in `tools/benchmark/output/` (gitignored). The `projection`
binary times a rule that reads a few fields of a large context, with and
without [input projection](#input-projection); `tools/benchmark/README.md`
lists the others.

### Creating Custom Benchmarks

```rust
use std::time::Instant;
use datalogic_rs::Engine;

fn main() {
    let engine = Engine::new();
    let compiled = engine.compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
    let mut session = engine.session();

    let iterations = 100_000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = session.eval_str(&compiled, r#"{"x": 1}"#);
    }

    let elapsed = start.elapsed();
    let per_op = elapsed / iterations;
    println!("Time per evaluation: {:?}", per_op);
}
```

For the hottest path, call `Engine::evaluate` and manage the arena
yourself. The result is a zero-copy `&DataValue<'a>`, without the deep
clone a `Session` makes at the boundary.

```rust
use bumpalo::Bump;

let arena = Bump::new();
let result = engine.evaluate(&compiled, r#"{"x": 1}"#, &arena).unwrap();
// `result` is `&DataValue<'_>`, borrowed from `arena`.
```

## Optimization Tips

### 1. Reuse Compiled Rules

```rust
// Good
let compiled = engine.compile(rule).unwrap();
for data in datasets {
    session.eval_str(&compiled, data)?;
}

// Bad: compiles the rule again on every iteration
for data in datasets {
    engine.eval_str(rule, data)?;
}
```

### 2. Pick the Right Entry Point

| Caller has on hand | Best entry point |
|--------------------|------------------|
| JSON strings, no engine config | `datalogic_rs::eval_str(rule, data)` |
| JSON strings (one-shot via configured engine) | `Engine::eval_str(rule, data)` |
| JSON strings (many runs) | `Session::eval_str(&compiled, data)` |
| `OwnedDataValue` (many runs) | `Session::eval(&compiled, &owned)` → `OwnedDataValue` |
| Typed `T` from `serde_json` (`feature = "serde_json"`) | `Session::eval_into::<T, _>(&compiled, data)` |
| Several separate values the rule reads as one object | `Session::eval*(&compiled, &Roots::from([..]))` |
| A session stored in a struct, moved across threads or held across `.await` | `SharedSession::new(Arc<Engine>)` |
| Borrowed result, session-owned arena | `Session::eval_borrowed(&compiled, data)` |
| Hot path, owns the `Bump` | `Engine::evaluate(&compiled, data, &arena)` |

### 3. Short-Circuit Evaluation

`and`, `or`, `if`, `?:`, and `??` short-circuit. Put the cheapest check,
or the one most likely to decide the result, first:

```json
{
  "and": [
    {"var": "isActive"},
    {"in": ["admin", {"var": "roles"}]}
  ]
}
```

### 4. Minimize Cloning in Custom Operators

`CustomOperator` receives args as `&DataValue<'a>` borrows. Avoid
materialising into owned values unless you need to mutate.

```rust
let n = args[0].as_f64().unwrap_or(0.0); // cheap read
```

Declare what the operator does through `CustomOperator::info`. A
deterministic operator that does not read the context is folded at compile
time when its arguments are constant, and it leaves the rule eligible for
[input projection](#input-projection). See
[Custom Operators](advanced/custom-operators.md).

### 5. Minimize Nested Variable Access

Each path segment is one lookup:

```json
{"var": "user.profile.settings.theme.color"}   // five lookups
{"var": "themeColor"}                           // one lookup
```

### 6. Pass Separate Values as `Roots`

If a rule reads several values your host keeps apart (a payload, its
metadata, the caller's claims), pass them as `Roots` instead of merging
them into one object per evaluation. Each root is borrowed and viewed in
place, and projection keeps only the roots the rule reads. Against a
per-evaluation `json!` merge, `Roots` measured 7 to 10 times faster for
payloads of 4 to 1,024 fields.

```rust
use datalogic_rs::{Engine, Roots};
use serde_json::json;

let engine = Engine::new();
let rule = engine
    .compile(r#"{"and": [{"==": [{"var": "metadata.channel"}, "web"]}, {">": [{"var": "data.total"}, 100]}]}"#)
    .unwrap();

let data = json!({"total": 120});
let metadata = json!({"channel": "web"});

let roots = Roots::from([("data", &data), ("metadata", &metadata)]);
let mut session = engine.session();
assert_eq!(session.eval_str(&rule, &roots).unwrap(), "true");
```

Each root may be a `&serde_json::Value`, `&OwnedDataValue`, `&ParsedData`
or `&DataValue`, mixed freely.

## JavaScript / WASM Performance

### Compile Once

`engine.evalStr(logic, data)` parses and compiles the rule on every call.
Compile it once with `engine.compile(logic)` and evaluate the returned
`Rule`:

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';
if (typeof init === 'function') await init();

const engine = new Engine();
const iterations = 10_000;

console.time('one-shot');
for (let i = 0; i < iterations; i++) {
  engine.evalStr(logic, data);
}
console.timeEnd('one-shot');

const rule = engine.compile(logic);
console.time('compiled');
for (let i = 0; i < iterations; i++) {
  rule.evaluate(data);
}
console.timeEnd('compiled');
```

For a hot loop, `engine.session()` reuses one arena across calls, and
`new DataHandle(json)` parses a payload once for many evaluations. The
`CompiledRule` class and the free `evaluate` function are deprecated and
removed in 6.0.

### React UI Performance

For the `DataLogicEditor` component:

1. **Memoise expressions:**
   ```tsx
   const expression = useMemo(() => ({ ... }), [deps]);
   ```
2. **Debounce data changes when debugging:** providing `data` enables the debugger, so debounce it to avoid re-evaluating on every keystroke.
   ```tsx
   const debouncedData = useDebouncedValue(data, 200);
   <DataLogicEditor value={expr} data={debouncedData} />
   ```
3. **Omit `data` when debugging isn't needed:** without it the component renders the expression as a read-only tree, skipping evaluation.
   ```tsx
   <DataLogicEditor value={expr} />
   ```

## Profiling

### Rust Profiling

```bash
# perf (Linux)
cargo build --release
perf record ./target/release/your-binary
perf report

# Instruments (macOS)
cargo instruments --release -t "CPU Profiler"
```

### Tracing for Bottlenecks

Enable the `trace` feature and call `engine.trace().eval_str(...)`
to inspect each executed node. Steps carry no timing data (use the
profilers above for that); they tell you which nodes ran, in what order,
and with which context, result, and iteration counts.

```rust
#[cfg(feature = "trace")]
{
    let run = engine.trace().eval_str(rule, data);
    for step in &run.steps {
        // step.step_id, step.node_id, step.context, step.result, step.error,
        // step.iteration_index, step.iteration_total
    }
    // run.expression_tree carries the per-node expression text, keyed by
    // the same node ids (ExpressionNode { id, expression, children }).
}
```

## Production Recommendations

1. **Pre-compile all rules at startup**
2. **Use a worker pool with per-worker Sessions** for parallel evaluation
3. **Monitor evaluation latency in production**
4. **Bound untrusted rules and their input data.** The engine has no
   built-in timeout; iteration and output size scale with the input, so cap
   array lengths and payload size. See
   [Security & Sandboxing](advanced/security.md) for the full guidance.
5. **Consider rule complexity limits for user-defined logic**

```rust
use datalogic_rs::{Engine, Logic};
use std::collections::HashMap;
use std::sync::Arc;

struct RuleEngine {
    engine: Arc<Engine>,
    rules: HashMap<String, Arc<Logic>>,
}

impl RuleEngine {
    pub fn new() -> Self {
        let engine = Arc::new(Engine::new());
        let mut rules = HashMap::new();

        for (name, logic) in load_rules() {
            let compiled = engine.compile_arc(&logic).unwrap();
            rules.insert(name, compiled);
        }

        Self { engine, rules }
    }

    pub fn evaluate(&self, rule_name: &str, data: &str) -> datalogic_rs::Result<String> {
        let compiled = self.rules.get(rule_name)
            .ok_or_else(|| datalogic_rs::Error::custom_message(format!("unknown rule: {rule_name}")))?;
        let mut session = self.engine.session();
        let result = session.eval_str(compiled, data);
        session.reset();
        result
    }
}
# fn load_rules() -> Vec<(String, String)> { Vec::new() }
```
