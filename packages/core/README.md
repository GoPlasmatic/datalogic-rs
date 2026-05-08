# datalogic-rs

[![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
[![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A fast, type-safe Rust implementation of [JSONLogic](http://jsonlogic.com)
for evaluating logical rules as JSON. The same compiled rule can power a
business-rules engine, a JSON template engine, or a safe expression
evaluator — and `Logic` is `Send + Sync` so one compile feeds many threads.

This is the **Rust core** of the
[`datalogic-rs` monorepo](https://github.com/GoPlasmatic/datalogic-rs).
The repo also publishes WebAssembly bindings
([`@goplasmatic/datalogic`](https://www.npmjs.com/package/@goplasmatic/datalogic))
and a React visual debugger
([`@goplasmatic/datalogic-ui`](https://www.npmjs.com/package/@goplasmatic/datalogic-ui))
that reuse this engine.

## Install

```bash
cargo add datalogic-rs
```

## Quick start

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let result = engine
    .evaluate_str(r#"{"+": [1, 2, 3]}"#, r#"{}"#)
    .unwrap();
assert_eq!(result, "6");
```

For repeated evaluation, compile once and reuse a `Session` (resettable
arena, no per-call heap churn). For zero-copy `&DataValue<'a>` results,
call `Engine::evaluate` directly with a caller-managed `bumpalo::Bump`.

## Performance

Three evaluation tiers, in order of caller control:

| API | Arena | Result | When to use |
|---|---|---|---|
| `Engine::evaluate_str(rule, data)` | engine creates a fresh `Bump::with_capacity(4096)` per call | `String` (JSON) | One-shot. CLI-style use, scripts, "I want JSON in and JSON out." |
| `Session::evaluate(&logic, data)` | session-owned `Bump`, caller calls `session.reset()` between batches | `OwnedDataValue` | Hot loop with a long-lived engine. Per-task in tokio, per-message in dataflow-style pipelines. |
| `Engine::evaluate(&logic, data, &arena)` | caller-passed `&Bump`; library never resets | `&'a DataValue<'a>` (borrowed) | Zero-copy result paths, pool-managed arenas, custom allocators. |

`Session` adds two extras for hot loops:

- `Session::evaluate_ref(...)` returns the same borrowed `&DataValue<'a>`
  shape as `Engine::evaluate` but with the bump owned by the session,
  so you skip the `OwnedDataValue::to_owned` deep-clone when the result
  is consumed before the next session call.
- `Session::reset_with_capacity(bytes)` drops the current chunks and
  allocates one fresh chunk of the given size — combine with
  `Session::allocated_bytes()` to capture a steady-state high-water
  mark from a warm-up pass and pre-size for the hot loop. The bench at
  `packages/benchmark/src/bin/self.rs` shows the pattern end-to-end.

The tokio-friendly idiom: `Arc<Engine>` shared across worker threads
(it's `Send + Sync`), one `Session` per task (it's `Send + !Sync`,
moves with the task across `.await` points). `Session::compile` is not
yet exposed because compile-time scratch isn't a hot path for any
known consumer; the existing `Engine::compile(rule)` allocates a small
internal bump and is called once at startup in typical service shapes.

## Feature flags

| Feature           | Effect                                                            |
|-------------------|-------------------------------------------------------------------|
| `compat`          | `serde_json` bridging + 4.x `LegacyApi` shims                     |
| `preserve`        | Structure-preservation (templating) mode                          |
| `datetime`        | Date / time operators (pulls in `chrono`)                         |
| `trace`           | Execution-step recording for the debugger (implies `compat`)      |
| `error-handling`  | `try` / `throw` operators                                         |
| `ext-string`, `ext-array`, `ext-control`, `ext-math` | Optional operator families     |

Default build is `serde_json`-free; opt in via `features = ["compat"]`
when you need the value boundary.

## Learn more

- [**Repo README**](https://github.com/GoPlasmatic/datalogic-rs#readme) — cross-runtime overview, examples for every use case
- [**Documentation site**](https://goplasmatic.github.io/datalogic-rs/) — full guide, operator reference, advanced topics
- [**Online playground**](https://goplasmatic.github.io/datalogic-rs/playground/) — try rules live in the visual debugger
- [`docs.rs/datalogic-rs`](https://docs.rs/datalogic-rs) — Rust API reference
- [`tests/README.md`](./tests/README.md) — JSON suite format
- [`examples/README.md`](./examples/README.md) — index of runnable examples

## License

Apache 2.0 — see [LICENSE](../../LICENSE).
