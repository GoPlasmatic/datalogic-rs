<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

# datalogic-rs

**A blazing fast, production-ready JSONLogic evaluation engine written in Rust, with cross-language bindings for Node.js, Python, Go, WebAssembly, and a companion React visual debugger.**

  [![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
  [![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
  [![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
  [![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
  [![npm (node)](https://img.shields.io/npm/v/@goplasmatic/datalogic-node?label=npm%20%40datalogic-node)](https://www.npmjs.com/package/@goplasmatic/datalogic-node)
  [![npm (wasm)](https://img.shields.io/npm/v/@goplasmatic/datalogic-wasm?label=npm%20%40datalogic-wasm)](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm)
  [![PyPI](https://img.shields.io/pypi/v/datalogic-py.svg)](https://pypi.org/project/datalogic-py/)

  [🚀 Try the Live Playground](https://goplasmatic.github.io/datalogic-rs/playground/) | [📖 Read the Documentation](https://goplasmatic.github.io/datalogic-rs/)
</div>

---

## What is datalogic-rs?

`datalogic-rs` is a high-performance Rust implementation of [JSONLogic](http://jsonlogic.com), a standard way to express logic rules in JSON. 

It functions as a safe, sandboxed expression evaluator and dynamic rules engine. While built as a native Rust crate, `datalogic-rs` provides official bindings that expose the exact same evaluation semantics and operators to **Node.js, WebAssembly, Python, Go, Java, .NET, and PHP**, alongside a **React-based companion debugger UI**.

<div align="center">
  <a href="https://goplasmatic.github.io/datalogic-rs/playground/">
    <img src="https://raw.githubusercontent.com/GoPlasmatic/datalogic-rs/main/docs/src/assets/demo.gif" alt="JSONLogic Online Debugger Demo" width="800">
  </a>
  <p><em>Try the online rule editor and debugger live in the <a href="https://goplasmatic.github.io/datalogic-rs/playground/">playground</a>.</em></p>
</div>

---

## Why datalogic-rs?

- 🔒 **100% Sandbox-Safe:** Evaluate user-submitted rules and formulas securely without arbitrary code execution (no dangerous `eval()` or scripting runtimes).
- 🌐 **Single Source of Truth:** Author rules in JSON and evaluate them with 100% semantic parity across your backend services (Rust/Go/Node/Python) and frontend client (WASM).
- ⚡ **Extreme Performance:** Compiles JSON logic into optimized bytecode. Evaluates rules in a hot loop using `bumpalo` arena allocation for zero-copy variables and minimal heap allocations.
- 🛠️ **Ready-Made Rule Builder:** Ship a visual rule editor directly to your product dashboard using the companion React UI package, saving weeks of custom UI engineering.

---

## Pick your package

The core Rust engine handles compilation and execution. Choose the language binding matching your stack below to view its specific installation guide and API reference:

| Language / Environment | Package / Dependency | Install Command | Deep-dive Guide |
| :--- | :--- | :--- | :--- |
| **Rust** application or library | [`datalogic-rs`](https://crates.io/crates/datalogic-rs) | `cargo add datalogic-rs` | [crates/README.md](./crates/datalogic-rs/README.md) |
| **Node.js** (Native prebuilds) | [`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node) | `npm i @goplasmatic/datalogic-node` | [bindings/node/README.md](./bindings/node/README.md) |
| **Browser, Edge, Bun, Deno** | [`@goplasmatic/datalogic-wasm`](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm) | `npm i @goplasmatic/datalogic-wasm` | [bindings/wasm/README.md](./bindings/wasm/README.md) |
| **Python** application or pipeline | [`datalogic-py`](https://pypi.org/project/datalogic-py/) | `pip install datalogic-py` | [bindings/python/README.md](./bindings/python/README.md) |
| **Go** microservice | `datalogic-go` | `go get github.com/GoPlasmatic/datalogic-rs/bindings/go/v5` | [bindings/go/README.md](./bindings/go/README.md) |
| **React** Visual Editor / Debugger | [`@goplasmatic/datalogic-ui`](https://www.npmjs.com/package/@goplasmatic/datalogic-ui) | `npm i @goplasmatic/datalogic-ui` | [ui/README.md](./ui/README.md) |
| **Java / JVM** (Kotlin, Scala) | `io.github.goplasmatic:datalogic` | Maven / Gradle Dependency | [bindings/jvm/README.md](./bindings/jvm/README.md) |
| **.NET** (C#, F#) | `Goplasmatic.Datalogic` | `dotnet add package Goplasmatic.Datalogic` | [bindings/dotnet/README.md](./bindings/dotnet/README.md) |
| **PHP** service | `goplasmatic/datalogic` | `composer require goplasmatic/datalogic` | [bindings/php/README.md](./bindings/php/README.md) |
| **C / FFI** ABI | `datalogic-c` | Built locally | [bindings/c/README.md](./bindings/c/README.md) |

---

## Three things you can build with Rust

### 1. Dynamic Business Rules

Encode pricing logic, access control, or form validation rules as JSON. Store them in databases or fetch them from APIs, changing logic dynamically without redeploying code.

```rust
let result = datalogic_rs::eval_str(
    r#"{"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "status"}, "active"]}]}"#,
    r#"{"age": 25, "status": "active"}"#,
).unwrap();
assert_eq!(result, "true");
```

### 2. JSON Response Templates

Shape data payloads on the fly. Enable templating mode so JSON key-value structures flow directly through to the output, mapping template operators to computed fields.

```rust
// Cargo.toml: datalogic-rs = { version = "5", features = ["templating"] }
use datalogic_rs::Engine;

let engine = Engine::builder().with_templating(true).build();
let result = engine.eval_str(
    r#"{"greeting": {"cat": ["Hello ", {"var": "name"}]},
        "isAdult": {">=": [{"var": "age"}, 18]}}"#,
    r#"{"name": "Jane", "age": 25}"#,
).unwrap();
// Output: {"greeting":"Hello Jane","isAdult":true}
```

### 3. Safe User Expressions

Allow power users or admins to write mathematical and conditional formulas (e.g. `subtotal + tax + shipping`). Evaluate them securely without standard scripting engines.

```rust
let result = datalogic_rs::eval_str(
    r#"{"+": [{"var": "subtotal"}, {"var": "tax"}, {"var": "shipping"}]}"#,
    r#"{"subtotal": 100, "tax": 8.5, "shipping": 5}"#,
).unwrap();
assert_eq!(result, "113.5");
```

---

## One rule, every runtime

Because all bindings run the same underlying Rust engine, a rule written in JSON evaluates with identical semantics across every tier of your architecture:

**Rust (Backend Core):**
```rust
let result = datalogic_rs::eval_str(r#"{">": [{"var": "x"}, 10]}"#, r#"{"x": 42}"#).unwrap();
// Returns "true"
```

**Node.js (Native binding):**
```javascript
import { apply } from '@goplasmatic/datalogic-node';
const result = apply({ '>': [{ var: 'x' }, 10] }, { x: 42 }); // true
```

**Browser / WebAssembly:**
```javascript
import init, { evaluate } from '@goplasmatic/datalogic-wasm';
await init();
const result = evaluate('{">": [{"var": "x"}, 10]}', '{"x": 42}', false); // "true"
```

**Python:**
```python
from datalogic_py import apply
result = apply({">": [{"var": "x"}, 10]}, {"x": 42}) # True
```

**Go:**
```go
import datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
out, _ := datalogic.Apply(`{">": [{"var": "x"}, 10]}`, `{"x": 42}`) // "true"
```

---

## 🎨 Visual Rule Debugger Companion

For building admin portals or dashboards where non-technical operators author rules, drop `@goplasmatic/datalogic-ui` into your React application. It uses the WASM core internally to compile and trace execution live.

```tsx
import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

<DataLogicEditor
  value={{ ">": [{ "var": "x" }, 10] }}
  data={{ x: 42 }}
  onChange={(newRule) => console.log('Rules modified:', newRule)}
/>
```

---

## Choosing your API: five tiers, one engine

The Rust crate provides a fine-grained API ladder depending on your performance budget, allocation constraints, and trace requirements.

| Tier | API Entry Point | When to Use |
| :--- | :--- | :--- |
| **Tier 0** | `eval_str`, `eval`, `eval_into`, `compile` | Quick scripts, simple tasks, or lazy one-off execution. |
| **Tier 1** | `Engine::eval*` | Using custom operators, non-default configs, or templating mode. |
| **Tier 2** | `Engine::session()` + `Session::eval*` | Evaluating rules in a hot loop (APIs, message queues, bulk pipelines). Reuses internal bump arenas to limit allocations. |
| **Tier 3** | `Engine::evaluate(&Logic, data, &Bump)` | Zero-copy evaluation using a caller-owned `bumpalo::Bump` arena. |
| **Tier 4** | `Engine::trace()` | Generating full AST execution paths for debugging or visualizer tools. |

Read the [Rust crate deep-dive](./crates/datalogic-rs/README.md) for detailed descriptions, performance profiles, and code snippets for each tier.

---

## Highlights

- **Cross-platform** — Native Rust engine wrapped for Node.js (napi), browsers + edge (WASM), Python, Go, Java, .NET, and PHP.
- **59 built-in operators** — 100% JSONLogic compliance out of the box, with opt-in support for OpenFeature flagd-compatible operators (`fractional`, `sem_ver`) via the `flagd` feature.
- **Thread-safe evaluation** — Compiled `Logic` is `Send + Sync`; share compiled logic across threads via `Arc`.
- **Zero `unsafe`** — Explicitly built with `#![forbid(unsafe_code)]` for maximum safety.
- **Zero-copy variables** — Employs `bumpalo`-backed evaluation; read-through operations like `var` borrow directly from the input representation.
- **Serde-optional** — The default builder has no dependency on `serde_json`. Enable the `serde_json` feature only when you need direct interop or typed JSON deserialization.
- **Highly Configurable** — Customize division-by-zero behaviors, NaN handling, truthiness rules, and numeric coercions.
- **Extensible custom operators** — Register custom operations easily via a Rust trait, and expose them to all downstream bindings.

---

## Performance

`datalogic-rs` is optimized for microsecond-scale hot-path execution. Compiled rules parse into a simple AST with OpCode dispatch (no runtime string matches) and execute inside a reusable memory arena.

Geomean execution time across 44 benchmark suites (Apple M2 Pro, macOS 26.3, Rust 1.93, Node 24; median of 3 samples, see [`tools/benchmark/BENCHMARK.md`][bench] for methodology):

```text
datalogic-rs (native Rust)              | 9.7 ns   (■) 1x
json-logic-engine (JS, compiled)        | 47.2 ns  (■■■■■) 4.9x
json-logic-engine (JS, interpreted)     | 160.3 ns (■■■■■■■■■■■■■■■■) 16.5x
jsonlogic-rs (bestowinc Rust engine)    | 218.0 ns (■■■■■■■■■■■■■■■~22.5x
json-logic-js (Reference JS library)    | 423.5 ns (■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■) 43.7x
@goplasmatic/datalogic-wasm (in Node)   | 855.6 ns (■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■) 88.2x
```

*Note: Node.js consumers should use `@goplasmatic/datalogic-node` for production, which relies on the Rust core via native N-API and runs close to native Rust performance (rather than the WASM engine).*

[bench]: ./tools/benchmark/BENCHMARK.md

---

## Migrating from v4

v5 contains breaking API updates: `DataLogic` is renamed to `Engine`, `CompiledLogic` to `Logic`, and `Operator` to `CustomOperator`. One-shot evaluation now uses `eval_str` (returning a `String`) or `eval_into::<T>` (for deserializing typed values). See the [MIGRATION.md](./MIGRATION.md) file for a detailed upgrade guide.

---

## Resources

- [Full Documentation](https://goplasmatic.github.io/datalogic-rs/) — Operator reference, config guides, and developer documentation
- [Online Playground](https://goplasmatic.github.io/datalogic-rs/playground/) — Build and test rules in your browser
- [Rust API Docs on docs.rs](https://docs.rs/datalogic-rs)
- [JSONLogic Specification](https://jsonlogic.com)
- [Architecture Overview](./ARCHITECTURE.md) — Module layout and binding structure
- [Development Guide](./DEVELOPMENT.md) — Local builds, testing, and contribution setup

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for contribution rules, [DEVELOPMENT.md](./DEVELOPMENT.md) for environment setup, and [ARCHITECTURE.md](./ARCHITECTURE.md) for structural diagrams.

## About Plasmatic

Created by [Plasmatic](https://github.com/GoPlasmatic), building open-source tools for financial infrastructure and data processing.

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE) for details.
