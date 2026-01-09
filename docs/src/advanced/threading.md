# Thread Safety

datalogic-rs is designed for thread-safe, concurrent evaluation.

## Thread-Safe Design

### CompiledLogic is Arc-wrapped

When you compile a rule, it's automatically wrapped in `Arc`:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let rule = json!({ ">": [{ "var": "x" }, 10] });

// compiled is Arc<CompiledLogic>
let compiled = engine.compile(&rule).unwrap();

// Clone is cheap (just increments reference count)
let compiled_clone = compiled.clone();  // or Arc::clone(&compiled)
```

### Sharing Across Threads

```rust
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;
use std::thread;

let engine = Arc::new(DataLogic::new());
let rule = json!({ "*": [{ "var": "x" }, 2] });
let compiled = engine.compile(&rule).unwrap();

let handles: Vec<_> = (0..4).map(|i| {
    let engine = Arc::clone(&engine);
    let compiled = Arc::clone(&compiled);

    thread::spawn(move || {
        let result = engine.evaluate_owned(
            &compiled,
            json!({ "x": i })
        ).unwrap();
        println!("Thread {}: {}", i, result);
    })
}).collect();

for handle in handles {
    handle.join().unwrap();
}
```

## Async Runtime Integration

### With Tokio

```rust
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let engine = Arc::new(DataLogic::new());
    let rule = json!({ "+": [{ "var": "a" }, { "var": "b" }] });
    let compiled = engine.compile(&rule).unwrap();

    // Spawn multiple async tasks
    let tasks: Vec<_> = (0..10).map(|i| {
        let engine = Arc::clone(&engine);
        let compiled = Arc::clone(&compiled);

        tokio::spawn(async move {
            // Use spawn_blocking for CPU-bound evaluation
            tokio::task::spawn_blocking(move || {
                engine.evaluate_owned(&compiled, json!({ "a": i, "b": i * 2 }))
            }).await.unwrap()
        })
    }).collect();

    for task in tasks {
        let result = task.await.unwrap().unwrap();
        println!("Result: {}", result);
    }
}
```

### Evaluation is CPU-bound

Since evaluation is CPU-bound (not I/O), use `spawn_blocking` in async contexts:

```rust
async fn evaluate_rule(
    engine: Arc<DataLogic>,
    compiled: Arc<CompiledLogic>,
    data: Value,
) -> Result<Value, Error> {
    tokio::task::spawn_blocking(move || {
        engine.evaluate_owned(&compiled, data)
    }).await.unwrap()
}
```

## Thread Pool Pattern

For high-throughput scenarios, use a thread pool:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;
use std::sync::Arc;
use rayon::prelude::*;

let engine = Arc::new(DataLogic::new());
let rule = json!({ "filter": [
    { "var": "items" },
    { ">": [{ "var": "value" }, 50] }
]});
let compiled = engine.compile(&rule).unwrap();

// Process many data sets in parallel
let datasets: Vec<Value> = (0..1000)
    .map(|i| json!({
        "items": (0..100).map(|j| json!({ "value": (i + j) % 100 })).collect::<Vec<_>>()
    }))
    .collect();

let results: Vec<_> = datasets
    .par_iter()  // Rayon parallel iterator
    .map(|data| {
        engine.evaluate(&compiled, data).unwrap()
    })
    .collect();
```

## Shared Engine vs Per-Thread Engine

### Shared Engine (Recommended)

Share one engine across threads when using the same custom operators:

```rust
use std::sync::Arc;

let mut engine = DataLogic::new();
engine.add_operator("custom".to_string(), Box::new(MyOperator));
let engine = Arc::new(engine);

// Share across threads
for _ in 0..4 {
    let engine = Arc::clone(&engine);
    thread::spawn(move || {
        // Use shared engine
    });
}
```

### Per-Thread Engine

Create separate engines when you need thread-local state:

```rust
thread_local! {
    static ENGINE: DataLogic = {
        let mut engine = DataLogic::new();
        // Thread-local configuration
        engine
    };
}

// Use in each thread
ENGINE.with(|engine| {
    let compiled = engine.compile(&rule).unwrap();
    engine.evaluate_owned(&compiled, data)
});
```

## Custom Operator Thread Safety

Custom operators must implement `Send + Sync`:

```rust
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

// Thread-safe custom operator
struct CounterOperator {
    counter: Arc<AtomicUsize>,
}

impl Operator for CounterOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        let count = self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(json!(count))
    }
}

// Create shared counter
let counter = Arc::new(AtomicUsize::new(0));

let mut engine = DataLogic::new();
engine.add_operator("count".to_string(), Box::new(CounterOperator {
    counter: Arc::clone(&counter),
}));
```

## Performance Considerations

### Compile Once, Evaluate Many

```rust
// GOOD: Compile once
let compiled = engine.compile(&rule).unwrap();
for data in datasets {
    engine.evaluate(&compiled, &data);
}

// BAD: Compiling in a loop
for data in datasets {
    let compiled = engine.compile(&rule).unwrap();  // Unnecessary!
    engine.evaluate(&compiled, &data);
}
```

### Minimize Cloning

```rust
// GOOD: Use references where possible
let result = engine.evaluate(&compiled, &data)?;

// Use owned version only when you don't need the data afterwards
let result = engine.evaluate_owned(&compiled, data)?;
```

### Batch Processing

```rust
// Process in batches to balance parallelism overhead
let batch_size = 100;
for chunk in datasets.chunks(batch_size) {
    let results: Vec<_> = chunk.par_iter()
        .map(|data| engine.evaluate(&compiled, data))
        .collect();
    // Process results
}
```

## Error Handling in Threads

```rust
use std::thread;

let handles: Vec<_> = datasets.into_iter().map(|data| {
    let engine = Arc::clone(&engine);
    let compiled = Arc::clone(&compiled);

    thread::spawn(move || -> Result<Value, Error> {
        engine.evaluate_owned(&compiled, data)
    })
}).collect();

// Collect results, handling errors
let results: Vec<Result<Value, Error>> = handles
    .into_iter()
    .map(|h| h.join().expect("Thread panicked"))
    .collect();

// Process results
for (i, result) in results.into_iter().enumerate() {
    match result {
        Ok(value) => println!("Result {}: {}", i, value),
        Err(e) => eprintln!("Error {}: {}", i, e),
    }
}
```
