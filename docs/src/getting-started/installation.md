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
> entry points (`Engine::eval_str`, `Engine::compile(&str)`,
> `datalogic_rs::eval_str`) are string-based. Add the `serde_json` feature
> only if you need `serde_json::Value` interop or the typed
> `eval_into::<T>` paths.

## Feature Flags

v5 splits the surface into a small core plus opt-in features:

| Feature | Default | What it adds |
|---------|---------|-------------|
| `serde_json` | off | `&serde_json::Value` interop (as `EvalInput` / `IntoLogic`) and the typed `eval_into::<T>` paths on `Engine`, `Session`, and the module-level helpers. Pulls in `serde_json` as a runtime dependency. |
| `templating` | off | Templating mode — `Engine::builder().with_templating(true).build()`. |
| `datetime` | off | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` operators (pulls in `chrono`). |
| `trace` | off | Per-evaluation execution tracing (`engine.trace()…`). Transitively enables `serde_json`. |
| `ext-string` | off | Extended string operators. |
| `ext-array` | off | Extended array operators (e.g. `sort`). |
| `ext-control` | off | Extended control-flow operators (e.g. `inspect`). |
| `error-handling` | off | `try` / `throw` operators. |
| `ext-math` | off | Extended math operators. |
| `wasm` | off | Bundle convenience for WASM builds (= `datetime` + `trace` + `templating`). |

Example — opt into `serde_json::Value` interop plus templating:

```toml
[dependencies]
datalogic-rs = { version = "5.0", features = ["serde_json", "templating"] }
serde_json = "1.0"
```

## Version Selection

- **v5.x** (current): canonical string-based API, opt-in `serde_json`, builder-only operator registration. v5 is a hard cliff — no `compat` shim — so plan a single cutover.
- **v4.x**: `DataLogic` engine, `serde_json::Value`-first API. Still functional but no longer the active line.
- **v3.x**: Arena-based allocation, predates the v4 simplification. Bug-fix only.

If you're upgrading from v4, see the [Migration Guide](../migration.md).

## Other languages

The Rust crate is the engine; every other language uses its own
binding. Click through to the binding's README for install
instructions and the language-idiomatic API:

| Language                      | Package                                                                          | Install                                                  | Deep-dive                                                                          |
|-------------------------------|----------------------------------------------------------------------------------|----------------------------------------------------------|------------------------------------------------------------------------------------|
| JavaScript / TypeScript (WASM) | [`@goplasmatic/datalogic-wasm`](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm) | `npm i @goplasmatic/datalogic-wasm`                           | [bindings/wasm/README.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/wasm/README.md) |
| Python                        | [`datalogic-py`](https://pypi.org/project/datalogic-py/)                         | `pip install datalogic-py`                               | [bindings/python/README.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/python/README.md) |
| Go                            | `datalogic-go`                                                                   | `go get github.com/GoPlasmatic/datalogic-rs/bindings/go` | [bindings/go/README.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/go/README.md)        |
| React (visual debugger)       | [`@goplasmatic/datalogic-ui`](https://www.npmjs.com/package/@goplasmatic/datalogic-ui) | `npm i @goplasmatic/datalogic-ui`                  | [ui/README.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/ui/README.md)                          |

Building the WASM binding from source:

```bash
cd bindings/wasm
./build.sh
```

## Minimum Rust Version

datalogic-rs v5 uses **Rust edition 2024** — Rust **1.85** or later is
required. The crate is built with `#![forbid(unsafe_code)]`.

## Verifying Installation

Create a simple test:

```rust
fn main() {
    let result = datalogic_rs::eval_str(r#"{"+": [1, 2]}"#, r#"{}"#).unwrap();

    println!("1 + 2 = {}", result);
    assert_eq!(result, "3");
}
```

Run with:

```bash
cargo run
```

You should see: `1 + 2 = 3`
