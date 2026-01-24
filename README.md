<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

# datalogic-rs
**A fast, production-ready Rust engine for JSONLogic.**

  [![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
  [![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
  [![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
  [![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
  [![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic)](https://www.npmjs.com/package/@goplasmatic/datalogic)

</div>

---

## Quick Example

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();
let logic = json!({ ">": [{ "var": "age" }, 18] });
let compiled = engine.compile(&logic).unwrap();

let result = engine.evaluate_owned(&compiled, json!({ "age": 21 })).unwrap();
assert_eq!(result, json!(true));
```

## Packages

| Package | Description | Install |
|---------|-------------|---------|
| [datalogic-rs](https://crates.io/crates/datalogic-rs) | Rust library | `cargo add datalogic-rs` |
| [@goplasmatic/datalogic](https://www.npmjs.com/package/@goplasmatic/datalogic) | WASM/JavaScript | `npm i @goplasmatic/datalogic` |
| [@goplasmatic/datalogic-ui](https://www.npmjs.com/package/@goplasmatic/datalogic-ui) | React visual debugger | `npm i @goplasmatic/datalogic-ui` |

## Resources

- [Full Documentation](https://goplasmatic.github.io/datalogic-rs/)
- [Online Playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [Rust API (docs.rs)](https://docs.rs/datalogic-rs)
- [JSONLogic Specification](https://jsonlogic.com)

## Online Debugger

<div align="center">
  <a href="https://goplasmatic.github.io/datalogic-rs/playground/">
    <img src="https://raw.githubusercontent.com/GoPlasmatic/datalogic-rs/main/docs/src/assets/demo.gif" alt="JSONLogic Online Debugger Demo" width="800">
  </a>
  <p><em>Try the <a href="https://goplasmatic.github.io/datalogic-rs/playground/">JSONLogic Online Debugger</a> to interactively test your rules</em></p>
</div>

## Key Features

- **Thread-Safe** - Compile once, evaluate anywhere with zero-copy `Arc` sharing
- **Fully Compliant** - Passes the official JSONLogic test suite
- **50+ Operators** - Including datetime, regex, and extended string/array operations
- **Extensible** - Add custom operators with a simple trait
- **Templating Mode** - Preserve object structures for dynamic JSON generation
- **Multi-Platform** - Rust, WASM (browser/Node.js), with visual React debugger

## JavaScript / TypeScript

```javascript
import init, { evaluate } from '@goplasmatic/datalogic';

await init();

const result = evaluate(
  '{">=": [{"var": "age"}, 18]}',
  '{"age": 21}',
  false
);
console.log(JSON.parse(result)); // true
```

## React Visual Debugger

```tsx
import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

<DataLogicEditor
  value={{ "and": [{ ">": [{ "var": "age" }, 18] }, { "var": "active" }] }}
  data={{ age: 25, active: true }}
  mode="debug"
/>
```

## About Plasmatic

Created by [Plasmatic](https://github.com/GoPlasmatic), building open-source tools for financial infrastructure and data processing.

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE) for details.
