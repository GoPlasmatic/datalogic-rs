# Thread Safety

`Engine` and `Logic` are `Send + Sync`: build and compile once, then share
them across threads. A `Session` owns a mutable arena, so it is `Send` but
not `Sync`: give each thread or worker its own.

## Thread-Safe Design

### Logic is Send + Sync

`Logic` (the v5 name for `CompiledLogic`) is `Send + Sync`. The engine
does not wrap it in `Arc` for you. Wrap it yourself when you want cheap
cross-thread sharing, or call `Engine::compile_arc` to do it in one step:

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

// Cloning the Arc is cheap: it bumps the refcount.
let compiled_clone = Arc::clone(&compiled);
```

`Engine` is also `Send + Sync` once built, so wrap it in `Arc` the same
way when sharing across threads.

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

### Sessions That Own Their Engine

`engine.session()` returns a `Session<'_>` that borrows the engine, so it
cannot outlive that borrow. A `SharedSession` holds the engine by `Arc`
instead. It is `'static + Send`: you can store it in a struct field, move
it to another thread, or hold it across an `.await`. Build one with
`SharedSession::new(Arc<Engine>)` or `SharedSession::from(arc)`. It has
the same methods as a borrowed session (`SharedSession` is
`Session<'static, Arc<Engine>>`), and `session.engine()` returns the
engine either way.

```rust
use std::sync::Arc;
use datalogic_rs::{Engine, Logic, SharedSession};

struct Worker {
    session: SharedSession,
    rule: Arc<Logic>,
}

impl Worker {
    fn new(engine: Arc<Engine>, rule: Arc<Logic>) -> Self {
        Self { session: SharedSession::new(engine), rule }
    }

    fn run(&mut self, payload: &str) -> datalogic_rs::Result<String> {
        let out = self.session.eval_str(&self.rule, payload);
        self.session.reset();
        out
    }
}

let engine = Arc::new(Engine::new());
let rule = engine.compile_arc(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

let mut worker = Worker::new(Arc::clone(&engine), rule);
let handle = std::thread::spawn(move || worker.run(r#"{"x": 42}"#).unwrap());
assert_eq!(handle.join().unwrap(), "true");
```

## Async Runtime Integration

### With Tokio

Evaluation is CPU-bound, so use `spawn_blocking` to keep async runtimes
responsive:

```rust
use datalogic_rs::Engine;
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

A task that evaluates many small rules between awaits can keep one
`SharedSession` for its whole lifetime. Each evaluation runs to completion
before the task can yield, and the session moves with the task if the
runtime reschedules it on another thread:

```rust
use std::sync::Arc;
use datalogic_rs::{Engine, SharedSession};

let engine = Arc::new(Engine::new());
let rule = engine.compile_arc(r#"{"+": [{"var": "a"}, {"var": "b"}]}"#).unwrap();

let task = tokio::spawn(async move {
    let mut session = SharedSession::new(engine);
    let mut total = 0;
    for i in 0..3 {
        tokio::task::yield_now().await; // the session lives across the await
        let out = session.eval_str(&rule, &format!(r#"{{"a": {i}, "b": 1}}"#)).unwrap();
        total += out.parse::<i64>().unwrap();
        session.reset();
    }
    total
});
assert_eq!(task.await.unwrap(), 6);
```

## Thread Pool Pattern

For high-throughput workloads, use a thread pool. Each worker keeps its own
`Session`, so it reuses one arena across calls without contention:

```rust
use datalogic_rs::Engine;
use rayon::prelude::*;
use std::sync::Arc;

let engine = Arc::new(Engine::new());
let compiled = engine
    .compile_arc(r#"{"filter": [{"var": "items"}, {">": [{"var": "value"}, 50]}]}"#)
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

> **Tip:** `Session` does **not** reset itself. Call `session.reset()`
> between batches (as above) to keep peak memory tracking the largest
> single evaluation rather than the lifetime sum.

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

An engine per thread duplicates every operator registration and buys
nothing for thread safety. If a thread-local was there to hold an arena
next to the engine a `Session` borrowed, keep one engine and give each
thread a `SharedSession`:

```rust
use std::cell::RefCell;
use std::sync::{Arc, LazyLock};
use datalogic_rs::{Engine, Logic, SharedSession};

static ENGINE: LazyLock<Arc<Engine>> = LazyLock::new(|| Arc::new(Engine::new()));

thread_local! {
    static SESSION: RefCell<SharedSession> =
        RefCell::new(SharedSession::new(Arc::clone(&ENGINE)));
}

fn run(rule: &Logic, data: &str) -> datalogic_rs::Result<String> {
    SESSION.with_borrow_mut(|session| {
        let out = session.eval_str(rule, data);
        session.reset();
        out
    })
}
```

Build separate engines only when threads need different configurations,
templating modes or operator sets.

## Custom Operator Thread Safety

`CustomOperator` requires `Send + Sync`. For shared mutable state, use the
usual synchronisation primitives:

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
        arena: &'a datalogic_rs::bumpalo::Bump,
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

### Re-entering the Engine

A custom operator that holds an `Arc<Engine>` can evaluate another rule
from inside its own `evaluate` (through `Engine::evaluate`, a session or a
traced run). `EvaluationConfig::max_recursion_depth` (default 256) caps how
many evaluations may be running at once on one thread, counting the
outermost. The count is per thread and shared by every engine on it; each
engine compares it with its own setting. An evaluation that would go past
the cap fails with `ConfigurationError` before it starts. An engine with
no custom operators skips the check, since built-ins cannot re-enter.

The cap does not bound how deeply a rule nests (the compiler caps that at
256 levels) or how much work an evaluation does (see
[Operation Budget](operation-budget.md)). It must be at least 1:
`EngineBuilder::try_build` and `EvaluationConfig::from_json_str` refuse 0
with a `ConfigurationError`, while `build()`, which cannot fail, keeps it.

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

// Bad: compiles the rule again on every iteration
for data in datasets {
    engine.eval_str(rule, data)?;
}
```

### Reuse the Arena

`Session` reuses one `bumpalo::Bump` across calls; you call
`session.reset()` between batches so peak memory tracks the largest
single evaluation rather than the sum. For zero-copy `&DataValue<'a>`
results, manage the `bumpalo::Bump` yourself and call `Engine::evaluate`.

### Short-Circuit Evaluation

`and`, `or`, `if`, `?:`, and `??` short-circuit. Put the cheapest
conditions, and those most likely to decide the result, first.

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

`Error` is `Send + Sync`, so you can return it from a thread or send it
over a channel.
