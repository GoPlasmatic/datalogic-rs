# Thread Safety

datalogic-rs is designed for thread-safe, concurrent evaluation.

## Thread-Safe Design

### Logic is Send + Sync

`Logic` (the v5 name for `CompiledLogic`) is `Send + Sync`. v5 does **not**
auto-wrap it in `Arc` — wrap it yourself when you want cheap cross-thread
sharing, or use `Engine::compile_arc` to do it in one step:

```rust
use datalogic_rs::Engine;
use std::sync::Arc;

let engine = Engine::new();

// Manual:
let compiled = Arc::new(
    engine.compile(r#"{">": [{"var": "x"}, 10]}"#).unwrap(),
);

// Or in one step (equivalent to `Arc::new(engine.compile(rule)?)`):
let compiled = engine.compile_arc(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

// Cloning the Arc is cheap — just bumps the refcount.
let compiled_clone = Arc::clone(&compiled);
```

`Engine` itself is also `Send + Sync` once built, so wrap it in `Arc`
the same way when sharing across threads.

### Sharing Across Threads

```rust
use datalogic_rs::Engine;
use std::sync::Arc;
use std::thread;

let engine = Arc::new(Engine::new());
let compiled = engine.compile_arc(r#"{"*": [{"var": "x"}, 2]}"#).unwrap();

let handles: Vec<_> = (0..4).map(|i| {
    let engine = Arc::clone(&engine);
    let compiled = Arc::clone(&compiled);

    thread::spawn(move || {
        let mut session = engine.session();
        session
            .eval_str(&compiled, &format!(r#"{{"x": {}}}"#, i))
            .unwrap()
    })
}).collect();

for handle in handles {
    println!("{}", handle.join().unwrap());
}
```

## Async Runtime Integration

### With Tokio

Evaluation is CPU-bound — use `spawn_blocking` to keep async runtimes
responsive:

```rust
use datalogic_rs::{Engine, Logic};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let engine = Arc::new(Engine::new());
    let compiled = engine.compile_arc(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#).unwrap();

    let tasks: Vec<_> = (0..10).map(|i| {
        let engine = Arc::clone(&engine);
        let compiled = Arc::clone(&compiled);

        tokio::task::spawn_blocking(move || {
            let mut session = engine.session();
            let payload = format!(r#"{{"a": {}, "b": {}}}"#, i, i * 2);
            session.eval_str(&compiled, &payload)
        })
    }).collect();

    for task in tasks {
        let result = task.await.unwrap().unwrap();
        println!("{}", result);
    }
}
```

## Thread Pool Pattern

For high-throughput scenarios, use a thread pool — each worker keeps its own
`Session` so the arena is reused across calls without contention:

```rust
use datalogic_rs::Engine;
use rayon::prelude::*;
use std::sync::Arc;

let engine = Arc::new(Engine::new());
let compiled = engine
    .compile_arc(r#"{"filter": [{"var": "items"}, {">": [{"var": ".value"}, 50]}]}"#)
    .unwrap();

let datasets: Vec<String> = (0..1000)
    .map(|i| format!(r#"{{"items": [{{"value": {}}}, {{"value": {}}}]}}"#, i % 100, (i + 1) % 100))
    .collect();

let results: Vec<_> = datasets
    .par_iter()
    .map_init(
        || engine.session(),
        |session, data| {
            let r = session.eval_str(&compiled, data);
            session.reset();
            r
        },
    )
    .collect();
```

> **Tip:** `Session` does **not** auto-reset. Call `session.reset()` between
> batches (as above) to keep peak memory tracking the largest single
> evaluation rather than the lifetime sum.

## Shared Engine vs Per-Thread Engine

### Shared Engine (Recommended)

Build the engine once with all custom operators, then share via `Arc`:

```rust
use std::sync::Arc;
use datalogic_rs::Engine;

let engine = Arc::new(
    Engine::builder()
        .add_operator("custom", MyOperator)
        .build(),
);

for _ in 0..4 {
    let engine = Arc::clone(&engine);
    std::thread::spawn(move || {
        let mut session = engine.session();
        // Use shared engine.
    });
}
```

### Per-Thread Engine

Use when you genuinely need thread-local engine state:

```rust
thread_local! {
    static ENGINE: datalogic_rs::Engine = datalogic_rs::Engine::new();
}

ENGINE.with(|engine| {
    let compiled = engine.compile(r#"{"==": [1, 1]}"#).unwrap();
    let mut session = engine.session();
    session.eval_str(&compiled, r#"{}"#)
});
```

## Custom Operator Thread Safety

`CustomOperator` is `Send + Sync`. For shared mutable state, use the usual
synchronisation primitives:

```rust
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use datalogic_rs::{CustomOperator, DataValue, Engine, Result};
use datalogic_rs::operator::EvalContext;

struct CounterOperator {
    counter: Arc<AtomicUsize>,
}

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

let counter = Arc::new(AtomicUsize::new(0));
let engine = Engine::builder()
    .add_operator("count", CounterOperator { counter: Arc::clone(&counter) })
    .build();
```

## Performance Considerations

### Compile Once, Evaluate Many

```rust
// Good
let compiled = engine.compile(rule).unwrap();
let mut session = engine.session();
for data in datasets {
    session.eval_str(&compiled, data)?;
    session.reset();
}

// Bad — recompiles every iteration
for data in datasets {
    let compiled = engine.compile(rule).unwrap();
    engine.eval_str(rule, data)?;
}
```

### Reuse the Arena

`Session` reuses one `bumpalo::Bump` across calls; the caller calls
`session.reset()` between batches so peak memory tracks the largest
single evaluation rather than the sum. For zero-copy `&DataValue<'a>`
results, manage the `bumpalo::Bump` yourself and call `Engine::evaluate`
directly.

### Short-Circuit Evaluation

`and`, `or`, `if`, `?:`, and `??` short-circuit. Order conditions so that
the cheapest / most-likely-to-decide ones come first.

## Error Handling in Threads

```rust
use datalogic_rs::{Engine, Error};
use std::sync::Arc;
use std::thread;

let engine = Arc::new(Engine::new());
let compiled = engine.compile_arc(r#"{"+": [1, 1]}"#).unwrap();

let handles: Vec<_> = (0..4).map(|_| {
    let engine = Arc::clone(&engine);
    let compiled = Arc::clone(&compiled);
    thread::spawn(move || -> Result<String, Error> {
        let mut session = engine.session();
        session.eval_str(&compiled, r#"{}"#)
    })
}).collect();

for h in handles {
    match h.join().expect("thread panicked") {
        Ok(value) => println!("{}", value),
        Err(e) => eprintln!("error: {} (operator: {:?}, node_ids: {:?})", e, e.operator(), e.node_ids()),
    }
}
```
