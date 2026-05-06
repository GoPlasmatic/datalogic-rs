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
