# Installation

## Adding to Your Project

Add `datalogic-rs` to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "5.0"
```

Or use cargo add:

```bash
cargo add datalogic-rs
```

> **Note:** v5 does **not** require `serde_json` by default — the canonical
> entry points (`Engine::evaluate_str`, `Engine::compile(&str)`) are
> string-based. Add `serde_json` only if you opt into the `compat` feature
> below.

## Feature Flags

v5 splits the surface into a small core plus opt-in features:

| Feature | Default | What it adds |
|---------|---------|-------------|
| `compat` | off | `serde_json::Value` boundary (`evaluate_serde`, `compile_serde_value`), the v4 `LegacyApi` shims, and `serde_json` as a runtime dependency. |
| `preserve` | off | Structure-preservation (templating) mode — `Engine::builder().preserve_structure(true).build()`. |
| `datetime` | off | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` operators (pulls in `chrono`). |
| `trace` | off | Per-evaluation execution tracing (`engine.with_trace()…`). Implies `compat`. |
| `ext-string` | off | Extended string operators. |
| `ext-array` | off | Extended array operators (e.g. `sort`). |
| `ext-control` | off | Extended control-flow operators (e.g. `inspect`). |
| `error-handling` | off | `try` / `throw` operators. |
| `ext-math` | off | Extended math operators. |
| `wasm` | off | Bundle convenience for WASM builds (= `datetime` + `trace` + `preserve`). |

Example — opt into the v4-compatible `serde_json` boundary plus structure
preservation:

```toml
[dependencies]
datalogic-rs = { version = "5.0", features = ["compat", "preserve"] }
serde_json = "1.0"
```

## Version Selection

- **v5.x** (current): canonical string-based API, opt-in `serde_json`, builder-only operator registration.
- **v4.x**: `DataLogic` engine, `serde_json::Value`-first API. Still functional but no longer the active line.
- **v3.x**: Arena-based allocation, predates the v4 simplification. Bug-fix only.

If you're upgrading from v4, see the [Migration Guide](../migration.md).

## WebAssembly Support

For WebAssembly targets, use the npm package:

```bash
npm install @goplasmatic/datalogic
```

Or build from source:

```bash
cd wasm
./build.sh
```

## Minimum Rust Version

datalogic-rs v5 uses **Rust edition 2024** — Rust **1.85** or later is
required. The crate is built with `#![forbid(unsafe_code)]`.

## Verifying Installation

Create a simple test:

```rust
use datalogic_rs::Engine;

fn main() {
    let engine = Engine::new();
    let result = engine
        .evaluate_str(r#"{"+": [1, 2]}"#, r#"{}"#)
        .unwrap();

    println!("1 + 2 = {}", result);
    assert_eq!(result, "3");
}
```

Run with:

```bash
cargo run
```

You should see: `1 + 2 = 3`
