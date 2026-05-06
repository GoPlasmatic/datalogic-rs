# Performance

This guide covers performance optimization, benchmarking, and best practices for datalogic-rs.

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
    session.evaluate_str(&compiled, data)?;
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
- **Reusable arenas** — `Session` resets its `Bump` between calls so peak
  memory tracks the largest single evaluation rather than the sum.
- **Pre-built literal singletons** — trivial literals (`Null`, `Bool`,
  empty primitives) are static and incur no per-call allocation.
- **`Arc<Logic>`** — cheap clone for cross-thread sharing.

## Benchmarking

### Running Benchmarks

```bash
# Run the benchmark example
cargo run --example benchmark --release

# With custom iterations
BENCH_ITERATIONS=100000 cargo run --example benchmark --release
```

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
        let _ = session.evaluate_str(&compiled, r#"{"x": 1}"#);
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
    session.evaluate_str(&compiled, data)?;
}

// Bad — recompiles every iteration
for data in datasets {
    let compiled = engine.compile(rule).unwrap();
    engine.evaluate_str(rule, data)?;
}
```

### 2. Pick the Right Entry Point

| Caller has on hand | Best entry point |
|--------------------|------------------|
| JSON strings (one-shot) | `Engine::evaluate_str(rule, data)` |
| JSON strings (many runs) | `Session::evaluate_str(&compiled, data)` |
| `OwnedDataValue` (many runs) | `Session::evaluate(&compiled, &owned)` → `OwnedDataValue` |
| `serde_json::Value` (legacy boundary) | `Engine::evaluate_serde` / `Session::evaluate_serde` (`compat`) |
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
2. **Debounce data changes in debug mode:**
   ```tsx
   const debouncedData = useDebouncedValue(data, 200);
   <DataLogicEditor value={expr} data={debouncedData} mode="debug" />
   ```
3. **Use visualize mode when debugging isn't needed:**
   ```tsx
   <DataLogicEditor value={expr} mode="visualize" />
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

Enable the `trace` feature and call `engine.with_trace().evaluate_str(...)`
to inspect every executed node.

```rust
#[cfg(feature = "trace")]
{
    let run = engine.with_trace().evaluate_str(rule, data);
    for step in &run.steps {
        // step.node_id, step.expression, step.context, step.result, ...
    }
}
```

## Production Recommendations

1. **Pre-compile all rules at startup**
2. **Use a worker pool with per-worker Sessions** for parallel evaluation
3. **Monitor evaluation latency in production**
4. **Set appropriate timeouts for untrusted rules**
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
            let compiled = Arc::new(engine.compile(&logic).unwrap());
            rules.insert(name, compiled);
        }

        Self { engine, rules }
    }

    pub fn evaluate(&self, rule_name: &str, data: &str) -> datalogic_rs::Result<String> {
        let compiled = self.rules.get(rule_name)
            .ok_or_else(|| datalogic_rs::Error::custom(format!("unknown rule: {rule_name}")))?;
        let mut session = self.engine.session();
        session.evaluate_str(compiled, data)
    }
}
# fn load_rules() -> Vec<(String, String)> { Vec::new() }
```
