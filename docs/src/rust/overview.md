# Rust (Native Crate)

`datalogic-rs` is the core: everything the other bindings expose is implemented here. Using the crate directly gives you the full API ladder, including the zero-copy and tracing tiers no wrapper exposes.

## Install

```bash
cargo add datalogic-rs
```

The default build has **no dependency on `serde_json`** and ships the 33 baseline operators. Opt into features as needed:

```toml
[dependencies]
datalogic-rs = { version = "5", features = ["serde_json", "datetime", "templating", "trace", "flagd"] }
```

See the [feature matrix](../getting-started/installation.md) for what each flag adds.

## Quick start

```rust
let result = datalogic_rs::eval_str(
    r#"{">": [{"var": "x"}, 10]}"#,
    r#"{"x": 42}"#,
).unwrap();
assert_eq!(result, "true");
```

Module-level helpers (`eval`, `eval_str`, `eval_into`, `compile`) are backed by a default engine, so one-off evaluation needs no setup.

## Five tiers, one engine

The crate exposes a fine-grained API ladder; pick the tier matching your performance budget and trace requirements:

| Tier | API Entry Point | When to use |
| :--- | :--- | :--- |
| **Tier 0** | `eval_str`, `eval`, `eval_into`, `compile` | Quick scripts, simple tasks, one-off execution |
| **Tier 1** | `Engine::eval*` | Custom operators, non-default configs, templating mode |
| **Tier 2** | `Engine::session()` + `Session::eval*` | Hot loops (APIs, message queues, bulk pipelines); reuses internal bump arenas |
| **Tier 3** | `Engine::evaluate(&Logic, data, &Bump)` | Zero-copy evaluation with a caller-owned `bumpalo::Bump` arena |
| **Tier 4** | `Engine::trace()` | Full AST execution paths for debuggers and visualizers (`trace` feature) |

Tiers 0–2 exist in every language binding; Tiers 3 and 4 are Rust-only.

## Compile once, evaluate many

```rust
use datalogic_rs::Engine;

let engine = Engine::default();
let logic = engine.compile(r#"{">": [{"var": "x"}, 10]}"#).unwrap();

let mut session = engine.session();
for payload in inputs {
    let result = session.eval_str(&logic, payload).unwrap();
}
```

Compiled `Logic` is `Send + Sync`: share it across threads via `Arc` (or `Engine::compile_arc`). Sessions are cheap but not `Sync`; open one per thread. See [Thread Safety](../advanced/threading.md) for Tokio and rayon patterns.

## Where everything else is documented

- [API Reference](api-reference.md) — every public type, method, and error variant
- [docs.rs/datalogic-rs](https://docs.rs/datalogic-rs) — rustdoc with feature badges
- [Crate README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme) — the deep-dive with per-tier performance profiles
- [Runnable examples](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs/examples) — ten CI-built examples from getting started to zero-copy input
- [Custom Operators](../advanced/custom-operators.md) · [Configuration](../advanced/configuration.md) · [Structured Objects](../advanced/structured-objects.md) · [Security & Sandboxing](../advanced/security.md)
