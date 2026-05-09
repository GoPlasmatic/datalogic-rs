# datalogic-rs

[![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
[![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
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

**Which evaluate method should I use?** Start with `evaluate_str` for
one-shot calls. Switch to `Session` once you're evaluating the same
compiled rule many times — it reuses one arena instead of allocating
per call. Drop down to `Engine::evaluate` only when you're managing
your own `bumpalo::Bump` (custom pools, integration with arena-aware
downstream code). The full comparison table is in the
[Performance](#performance) section below.

## Migrating from 4.x

v5 reshapes the public API around a single arena-based dispatch path
and trims the surface to a small, documented set of types. Headline
renames: `DataLogic` → `Engine`, `evaluate_json` → `evaluate_str`,
`Operator` → `CustomOperator`, `with_config(...)` →
`Engine::builder().config(...).build()`. The full breakage list and
each replacement live in [`CHANGELOG.md`](CHANGELOG.md); a runnable
side-by-side cheat sheet is in
[`examples/migrating_from_v4.rs`](examples/migrating_from_v4.rs)
(`cargo run --example migrating_from_v4 --features compat,preserve`).
The `compat` feature keeps the 4.x entry points compiling for one
release cycle (each is `#[deprecated]` with a `note` pointing at its
v5 replacement); the `compat` module is scheduled for removal in 5.1.

## Input shapes

Both `Engine::evaluate` and `Session::evaluate*` accept any of the
input shapes a caller is likely to have on hand, via the sealed
[`EvalInput`] trait — all five resolve to `&'a DataValue<'a>` inside
the engine. Per-call cost differs:

| Shape | Cost per call |
|---|---|
| `&str` (JSON literal) | parse + arena alloc |
| `&serde_json::Value` (`compat` feature) | deep-convert into the arena |
| `&OwnedDataValue` | deep-clone into the arena |
| `DataValue<'a>` (by value) | one arena alloc for the top node |
| `&'a DataValue<'a>` (by reference) | **zero** — pass-through |

If you're evaluating the same input against many rules, or feeding
input from an upstream stage that already lives in the arena, prefer
the `&'a DataValue<'a>` path — it's genuinely allocation-free for the
input. See `examples/zero_copy_input.rs` for the five paths side by
side, including a runtime arena-bytes measurement that proves the
zero-copy claim.

[`EvalInput`]: https://docs.rs/datalogic-rs/latest/datalogic_rs/trait.EvalInput.html

## Working with `DataValue`

Evaluation returns `&'a DataValue<'a>` — an arena-allocated, borrowed
JSON-shaped value tree. The type lives in the sibling `datavalue` crate
(re-exported here at the crate root and as `datalogic_rs::datavalue`).
Most callers only need a handful of accessors:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"var": "user.score"}"#).unwrap();
let mut session = engine.session();
let result = session.evaluate_ref(&compiled, r#"{"user": {"score": 42}}"#).unwrap();

assert_eq!(result.as_i64(), Some(42));
// Other accessors: .as_f64(), .as_str(), .as_bool(), .as_array(), .as_object().
```

Conversion to other shapes:

- **To a JSON string:** `data_to_json_string(value)` — re-exported at
  the crate root, called internally by `evaluate_str`.
- **To `serde_json::Value`** (requires `compat` feature): the
  `Session::evaluate_serde` and `Engine::evaluate_serde` entry points
  return `serde_json::Value` directly.
- **Owned vs borrowed:** `DataValue<'a>` borrows from a `Bump`;
  `OwnedDataValue` is the heap-owned counterpart for crossing arena
  lifetimes (cache an evaluation result, send across an `await`, etc.).
  Convert via `.to_owned()` (borrowed → owned) and `.to_arena(&bump)`
  (owned → borrowed).

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
