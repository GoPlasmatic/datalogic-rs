# Performance

This guide covers performance optimization, benchmarking, and best practices for datalogic-rs.

## The headline numbers

<!-- canonical-bench v5.0 -->
Geomean execution time across 50 benchmark suites (Apple M2 Pro; median of 3 samples; ratios are pairwise shared-suite geomeans; methodology in the [benchmark matrix](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)):

```text
datalogic-rs (native Rust)              | 9.0 ns   (■) 1x
json-logic-engine (JS, compiled)        | 60.4 ns  (■■■■■■) 7.9x
json-logic-engine (JS, interpreted)     | 236.0 ns (■■■■■■■■■■■■■■■■■■■■■■■■) 30.7x
jsonlogic-rs (bestowinc Rust engine)    | 243.7 ns (■■■■■■■■■■■■■■■■■■■■■■■■) 30.3x
json-logic-js (Reference JS library)    | 433.5 ns (■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■) 102.8x
```

The WASM build under Node measures 881.9 ns geomean (98× native): on Node servers, prefer `@goplasmatic/datalogic-node`. Reproduce it yourself with `cargo run --release -p datalogic-bench --bin compare`; positioning against each alternative is on [How It Compares](comparison.md).

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

Built-in operators use direct OpCode dispatch instead of string lookups:

- 59 built-in operators have direct dispatch
- Custom operators use a single map lookup
- No runtime reflection or dynamic dispatch

### Memory Efficiency

v5 optimizations:

- **Arena allocation** — `&DataValue<'a>` results live in a `bumpalo::Bump`
  for one evaluation. Read-through ops like `var` borrow zero-copy from the
  caller's input.
- **Reusable arenas** — `Session` reuses one `Bump` across calls; the caller
  calls `session.reset()` between batches so peak memory tracks the largest
  single evaluation rather than the sum.
- **Pre-built literal singletons** — trivial literals (`Null`, `Bool`,
  empty primitives) are static and incur no per-call allocation.
- **`Arc<Logic>`** — cheap clone for cross-thread sharing.

## Benchmarking

### Running Benchmarks

The benchmark harness lives in its own dev-only crate, `datalogic-bench`,
under `tools/benchmark/`. Two binaries share a common harness:

```bash
# Single-engine benchmark (datalogic-rs alone, fast arena path)
cargo run --release -p datalogic-bench --bin self
cargo run --release -p datalogic-bench --bin self -- --all   # every suite + JSON report

# Cross-library comparison (only datalogic-rs ships by default; see
# tools/benchmark/README.md for adding more subjects)
cargo run --release -p datalogic-bench --bin compare -- --all
```

Reports land in `tools/benchmark/output/` (gitignored).

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

For the absolute hot path, drop down to `Engine::evaluate` and manage the
arena yourself — the result is a zero-copy `&DataValue<'a>` and avoids the
deep-clone Session does at the boundary.

```rust
use bumpalo::Bump;

let arena = Bump::new();
let result = engine.evaluate(&compiled, r#"{"x": 1}"#, &arena).unwrap();
// `result` is `&DataValue<'_>` — borrows from `arena`.
```

## Optimization Tips

### 1. Reuse Compiled Rules

```rust
// Good
let compiled = engine.compile(rule).unwrap();
for data in datasets {
    session.eval_str(&compiled, data)?;
}

// Bad — recompiles every iteration
for data in datasets {
    let compiled = engine.compile(rule).unwrap();
    engine.eval_str(rule, data)?;
    let _ = compiled;
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
| Borrowed result, session-owned arena | `Session::eval_borrowed(&compiled, data)` |
| Hot path, owns the `Bump` | `Engine::evaluate(&compiled, data, &arena)` |

### 3. Short-Circuit Evaluation

`and`, `or`, `if`, `?:`, and `??` short-circuit. Order conditions so the
cheapest / most-likely-to-decide check comes first:

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
materialising into owned values unless you actually need to mutate.

```rust
let n = args[0].as_f64().unwrap_or(0.0); // cheap read
```

### 5. Minimize Nested Variable Access

Deep paths require multiple lookups:

```json
{"var": "user.profile.settings.theme.color"}   // slow
{"var": "themeColor"}                           // fast
```

## JavaScript / WASM Performance

### CompiledRule Advantage

```javascript
const iterations = 10_000;

console.time('evaluate');
for (let i = 0; i < iterations; i++) {
  evaluate(logic, data, false);
}
console.timeEnd('evaluate');

const rule = new CompiledRule(logic, false);
console.time('compiled');
for (let i = 0; i < iterations; i++) {
  rule.evaluate(data);
}
console.timeEnd('compiled');
```

Typical improvement: 2–5× faster with `CompiledRule`.

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
to inspect every executed node.

```rust
#[cfg(feature = "trace")]
{
    let run = engine.trace().eval_str(rule, data);
    for step in &run.steps {
        // step.node_id, step.expression, step.context, step.result, ...
    }
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
