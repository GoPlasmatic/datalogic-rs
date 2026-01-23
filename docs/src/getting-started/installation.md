# Installation

## Adding to Your Project

Add `datalogic-rs` to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "4.0"
serde_json = "1.0"
```

Or use cargo add:

```bash
cargo add datalogic-rs serde_json
```

## Version Selection

- **v4.x** (recommended): Ergonomic API with `serde_json::Value`, simpler to use
- **v3.x**: Arena-based allocation for maximum raw performance

Both versions are actively maintained. Choose v4 for ease of use, v3 if you need every bit of performance.

## Feature Flags

datalogic-rs has minimal dependencies by default. All features are included in the base crate.

## WebAssembly Support

For WebAssembly targets, use the npm package:

```bash
npm install @goplasmatic/datalogic
```

Or build from source:

```bash
cd wasm
wasm-pack build --target web
```

## Minimum Rust Version

datalogic-rs requires Rust 1.70 or later.

## Verifying Installation

Create a simple test:

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

fn main() {
    let engine = DataLogic::new();
    let rule = json!({ "+": [1, 2] });
    let compiled = engine.compile(&rule).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();

    println!("1 + 2 = {}", result);
    assert_eq!(result, json!(3));
}
```

Run with:

```bash
cargo run
```

You should see: `1 + 2 = 3`
