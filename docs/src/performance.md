# Performance

This guide covers performance optimization, benchmarking, and best practices for datalogic-rs.

## Performance Characteristics

### Compilation vs Evaluation

datalogic-rs uses a two-phase approach:

1. **Compilation** (slower): Parse and optimize the JSONLogic expression
2. **Evaluation** (faster): Execute compiled logic against data

**Best practice:** Compile once, evaluate many times.

```rust
// Compile once
let compiled = engine.compile(&logic)?;

// Evaluate many times
for data in dataset {
    engine.evaluate_owned(&compiled, data)?;
}
```

### OpCode Dispatch

Built-in operators use direct OpCode dispatch instead of string lookups:

- 59 built-in operators have direct dispatch
- Custom operators use map lookup (still fast)
- No runtime reflection or dynamic dispatch

### Memory Efficiency

v4 optimizations:
- `SmallVec` for small arrays (avoids heap allocation)
- `Cow` types for value passing (avoids cloning)
- `Arc` for compiled logic (cheap cloning for threads)

## Benchmarking

### Running Benchmarks

```bash
# Run the benchmark example
cargo run --example benchmark --release

# With custom iterations
BENCH_ITERATIONS=100000 cargo run --example benchmark --release
```

### Sample Results

Typical performance on modern hardware:

| Operation | Time |
|-----------|------|
| Simple comparison | ~50ns |
| Variable access | ~100ns |
| Complex nested logic | ~500ns |
| Array map (10 items) | ~2μs |
| Large expression (50+ nodes) | ~10μs |

*Results vary by CPU, expression complexity, and data size.*

### Creating Custom Benchmarks

```rust
use std::time::Instant;
use datalogic_rs::DataLogic;
use serde_json::json;

fn main() {
    let engine = DataLogic::new();
    let logic = json!({ "==": [{ "var": "x" }, 1] });
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({ "x": 1 });

    let iterations = 100_000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = engine.evaluate_owned(&compiled, data.clone());
    }

    let elapsed = start.elapsed();
    let per_op = elapsed / iterations;
    println!("Time per evaluation: {:?}", per_op);
}
```

## Optimization Tips

### 1. Reuse Compiled Rules

**Bad:**
```rust
for item in items {
    let compiled = engine.compile(&logic)?; // Recompiles every time!
    engine.evaluate_owned(&compiled, item)?;
}
```

**Good:**
```rust
let compiled = engine.compile(&logic)?;
for item in items {
    engine.evaluate_owned(&compiled, item)?;
}
```

### 2. Use References for Large Data

```rust
// Clones data (fine for small data)
engine.evaluate_owned(&compiled, large_data)

// Uses reference (better for large data)
engine.evaluate(&compiled, &large_data)
```

### 3. Avoid Unnecessary Cloning in Custom Operators

```rust
impl Operator for MyOperator {
    fn evaluate(&self, args: &[Value], context: &mut ContextStack, evaluator: &dyn Evaluator) -> Result<Value> {
        // Avoid: cloning arguments unnecessarily
        // let value = args[0].clone();

        // Better: evaluate directly
        let value = evaluator.evaluate(&args[0], context)?;

        // ...
    }
}
```

### 4. Short-Circuit Evaluation

`and` and `or` operators short-circuit. Order conditions by:
1. Cheapest to evaluate first
2. Most likely to short-circuit first

```json
{
  "and": [
    { "var": "isActive" },           // Simple variable check (fast)
    { "in": ["admin", { "var": "roles" }] }  // Array search (slower)
  ]
}
```

### 5. Use Specific Operators

Some operators are more efficient than others:

```json
// Less efficient: substring check
{ "in": ["@", { "var": "email" }] }

// More efficient: dedicated operator (when available)
{ "contains": [{ "var": "email" }, "@"] }
```

### 6. Minimize Nested Variable Access

Deep nesting requires multiple map lookups:

```json
// Slower: deep nesting
{ "var": "user.profile.settings.theme.color" }

// Faster: flatter structure
{ "var": "themeColor" }
```

## JavaScript/WASM Performance

### CompiledRule Advantage

```javascript
// Benchmark
const iterations = 10000;

// Without CompiledRule
console.time('evaluate');
for (let i = 0; i < iterations; i++) {
  evaluate(logic, data, false);
}
console.timeEnd('evaluate');

// With CompiledRule
const rule = new CompiledRule(logic, false);
console.time('compiled');
for (let i = 0; i < iterations; i++) {
  rule.evaluate(data);
}
console.timeEnd('compiled');
```

Typical improvement: 2-5x faster with `CompiledRule`.

### WASM Considerations

- **Initialization:** Call `init()` once at startup
- **String overhead:** JSON.stringify/parse has some cost
- **Memory:** WASM has its own memory space (efficient for large operations)

### React UI Performance

For the DataLogicEditor component:

1. **Memoize expressions:**
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

Use standard Rust profiling tools:

```bash
# With perf (Linux)
cargo build --release
perf record ./target/release/your-binary
perf report

# With Instruments (macOS)
cargo instruments --release -t "CPU Profiler"
```

### Tracing for Bottlenecks

Use `evaluate_with_trace` to identify slow sub-expressions:

```javascript
const trace = evaluate_with_trace(logic, data, false);
const { steps } = JSON.parse(trace);

// Analyze which steps take longest
steps.forEach(step => {
  console.log(step.operator, step.duration_ns);
});
```

## Comparison with Other Engines

datalogic-rs is designed for high-throughput evaluation. Compared to:

- **json-logic-js (JavaScript):** 10-50x faster for complex rules
- **json-logic-py (Python):** 20-100x faster
- **Other Rust implementations:** Competitive, with better ergonomics

Actual performance depends on:
- Expression complexity
- Data size
- Evaluation frequency
- Thread utilization

## Production Recommendations

1. **Pre-compile all rules at startup**
2. **Use connection/worker pools for parallel evaluation**
3. **Monitor evaluation latency in production**
4. **Set appropriate timeouts for untrusted rules**
5. **Consider rule complexity limits for user-defined logic**

```rust
// Production pattern
use std::sync::Arc;

struct RuleEngine {
    engine: Arc<DataLogic>,
    rules: HashMap<String, Arc<CompiledLogic>>,
}

impl RuleEngine {
    pub fn new() -> Self {
        let engine = Arc::new(DataLogic::new());
        let mut rules = HashMap::new();

        // Pre-compile all rules at startup
        for (name, logic) in load_rules() {
            let compiled = engine.compile(&logic).unwrap();
            rules.insert(name, compiled);
        }

        Self { engine, rules }
    }

    pub fn evaluate(&self, rule_name: &str, data: Value) -> Result<Value> {
        let compiled = self.rules.get(rule_name).ok_or("Unknown rule")?;
        self.engine.evaluate_owned(compiled, data)
    }
}
```
