<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

# datalogic-rs
**A fast, production-ready engine for JSONLogic — Rust core, Node-native, WASM, Python, Go, React debugger.**

  [![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
  [![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
  [![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
  [![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
  [![npm (node)](https://img.shields.io/npm/v/@goplasmatic/datalogic-node?label=npm%20%40datalogic-node)](https://www.npmjs.com/package/@goplasmatic/datalogic-node)
  [![npm (wasm)](https://img.shields.io/npm/v/@goplasmatic/datalogic?label=npm%20%40datalogic)](https://www.npmjs.com/package/@goplasmatic/datalogic)
  [![PyPI](https://img.shields.io/pypi/v/datalogic-py.svg)](https://pypi.org/project/datalogic-py/)

</div>

---

## What is datalogic-rs?

`datalogic-rs` is a fast Rust engine for [JSONLogic](http://jsonlogic.com),
a JSON-shaped language for evaluating logical rules against data. Use it
as a **rule engine** for business logic, a **JSON template engine** for
response shaping, or a **safe expression evaluator** for user-supplied
formulas — and run the same rules in **Rust, Node.js (native via napi),
the browser (WebAssembly), Python, Go, or a React visual debugger**.

<div align="center">
  <a href="https://goplasmatic.github.io/datalogic-rs/playground/">
    <img src="https://raw.githubusercontent.com/GoPlasmatic/datalogic-rs/main/docs/src/assets/demo.gif" alt="JSONLogic Online Debugger Demo" width="800">
  </a>
  <p><em>Try it live in the <a href="https://goplasmatic.github.io/datalogic-rs/playground/">online playground</a> — no install required.</em></p>
</div>

## Pick your package

The Rust crate is the engine. Every other package wraps it for a
specific runtime — same rules, same semantics, same operators. Click
through to the binding's own README for install, quick start, and the
full API reference for that language.

| Your stack                            | Package                                                                          | Install                                                     | Deep-dive                                       |
|---------------------------------------|----------------------------------------------------------------------------------|-------------------------------------------------------------|-------------------------------------------------|
| **Rust** application or service       | [`datalogic-rs`](https://crates.io/crates/datalogic-rs)                          | `cargo add datalogic-rs`                                    | [crates/datalogic-rs/README.md](./crates/datalogic-rs/README.md) |
| **Node.js** service (TypeScript or JS) | [`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node) | `npm i @goplasmatic/datalogic-node`             | [bindings/node/README.md](./bindings/node/README.md)            |
| **Browser, Deno, Bun, Cloudflare Workers, edge runtimes** | [`@goplasmatic/datalogic`](https://www.npmjs.com/package/@goplasmatic/datalogic) (WebAssembly) | `npm i @goplasmatic/datalogic` | [bindings/wasm/README.md](./bindings/wasm/README.md)            |
| **Python** service or data pipeline   | [`datalogic-py`](https://pypi.org/project/datalogic-py/)                         | `pip install datalogic-py`                                  | [bindings/python/README.md](./bindings/python/README.md)        |
| **Go** service                        | `datalogic-go`                                                                   | `go get github.com/GoPlasmatic/datalogic-rs/bindings/go`    | [bindings/go/README.md](./bindings/go/README.md)                |
| **React** visual rule editor / debugger | [`@goplasmatic/datalogic-ui`](https://www.npmjs.com/package/@goplasmatic/datalogic-ui) | `npm i @goplasmatic/datalogic-ui`                           | [ui/README.md](./ui/README.md)                                  |
| **C / PHP / JVM** via FFI             | `datalogic-c` (in-tree)                                                          | build locally — consumed by Go and future PHP/JVM bindings  | [bindings/c/README.md](./bindings/c/README.md)                  |

Not sure which one? If you're writing the rules and evaluating them in
the same service, pick the binding for that service's language.

**On Node.js, reach for `@goplasmatic/datalogic-node`** — it's the
native build (per-platform `.node` prebuild via
[napi-rs](https://napi.rs)), which is materially faster than the WASM
path. The WASM package is the right pick when you need a single
artifact across browser + edge runtimes (Deno, Bun, Cloudflare Workers)
or when you'd rather avoid per-platform prebuilt binaries.

If you're building a UI that lets humans author rules, also pull in
[`@goplasmatic/datalogic-ui`](./ui/README.md) — it consumes the WASM
binding and gives you a visual editor and step-through debugger.

## Three things you can build with it

### 1. Business rules

Encode access control, feature flags, and validation as JSON. Rules are
data — store them in a database, send them over an API, change them
without redeploys.

```rust
let result = datalogic_rs::eval_str(
    r#"{"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "status"}, "active"]}]}"#,
    r#"{"age": 25, "status": "active"}"#,
).unwrap();
assert_eq!(result, "true");
```

### 2. JSON templates

Shape one JSON payload into another. With templating mode, object keys
flow through to the output and operator values become computed fields —
the template's structure mirrors the response you want.

```rust
// Cargo.toml: datalogic-rs = { version = "5", features = ["templating"] }
use datalogic_rs::Engine;

let engine = Engine::builder().with_templating(true).build();
let result = engine.eval_str(
    r#"{"greeting": {"cat": ["Hello ", {"var": "name"}]},
        "isAdult": {">=": [{"var": "age"}, 18]}}"#,
    r#"{"name": "Jane", "age": 25}"#,
).unwrap();
// {"greeting":"Hello Jane","isAdult":true}
```

### 3. Expression evaluation

Let users author formulas; evaluate them safely without `eval()`.
Arithmetic, comparisons, and array reductions are all built in.

```rust
let result = datalogic_rs::eval_str(
    r#"{"+": [{"var": "subtotal"}, {"var": "tax"}, {"var": "shipping"}]}"#,
    r#"{"subtotal": 100, "tax": 8.5, "shipping": 5}"#,
).unwrap();
assert_eq!(result, "113.5");
```

`reduce`, `map`, `filter`, and `sort` extend the same pattern to
aggregations over arrays.

## One rule, every runtime

The same JSONLogic rule runs unchanged across every supported runtime.
Author the rule once; evaluate it on the server, in the browser, or
inside a visual editor.

**Rust** — server-side, native:

```rust
let result = datalogic_rs::eval_str(
    r#"{">": [{"var": "x"}, 10]}"#,
    r#"{"x": 42}"#,
).unwrap();
// "true"
```

**Node.js (native)** — services, scripts, CLIs:

```javascript
import { apply } from '@goplasmatic/datalogic-node';

const result = apply({ '>': [{ var: 'x' }, 10] }, { x: 42 });
// true
```

**Browser / Deno / Bun / Cloudflare Workers** — via WebAssembly:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic';

await init();
const result = evaluate('{">": [{"var": "x"}, 10]}', '{"x": 42}', false);
// "true"
```

**Python** — services, scripts, data pipelines:

```python
from datalogic_py import apply

result = apply({">": [{"var": "x"}, 10]}, {"x": 42})
# True
```

**Go** — services, CLIs:

```go
import datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go"

out, _ := datalogic.Apply(`{">": [{"var": "x"}, 10]}`, `{"x": 42}`)
// "true"
```

**React** — drop-in visual debugger / editor:

```tsx
import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

<DataLogicEditor
  value={{ ">": [{ "var": "x" }, 10] }}
  data={{ x: 42 }}
/>
```

See the rule run live in your browser at the
[online playground](https://goplasmatic.github.io/datalogic-rs/playground/).

## Choosing your API: five tiers, one engine

Every binding exposes the same conceptual ladder — pick the entry
point that matches how often you evaluate and how much control you
want over allocation.

| Tier | What it is                                                  | Use when                                                                            |
|------|-------------------------------------------------------------|-------------------------------------------------------------------------------------|
| **0** | **Module-level one-shot** — `eval_str`, `eval`, `eval_into`, `compile` | Quick scripts, ad-hoc evaluation, no custom configuration                           |
| **1** | **Engine one-shot** — `Engine::eval*`                       | You need custom operators, non-default config, or templating mode                   |
| **2** | **Session (hot loop)** — `Engine::session()` + `Session::eval*` | You're evaluating compiled rules many times — services, batch jobs, request handlers |
| **3** | **Zero-copy evaluate** — `Engine::evaluate(&Logic, data, &Bump)` | You want results that borrow directly into a caller-owned arena (specialised use)  |
| **4** | **Traced evaluation** — `Engine::trace()`                   | Debugging, visualising execution, building inspector UIs                            |

**Most callers want Tier 0 or Tier 2.** Tier 0 is the right default
for trying something out; reach for Tier 2 once the same rule is being
evaluated repeatedly. Bindings expose the same ladder under
language-idiomatic names — see each binding's README for the exact
call sites. For the Rust deep-dive, including code per tier and
runnable examples, see [crates/datalogic-rs/README.md](./crates/datalogic-rs/README.md).

## Highlights

- **Cross-platform** — same engine, same rules in Rust, Node.js (native), browsers + edge runtimes (WASM), Python, Go, and a React UI
- **59 built-in operators** with full JSONLogic spec compliance
- **Compile once, evaluate millions of times** — `Logic` is `Send + Sync`; share via `Arc`
- **Zero `unsafe`** — built with `#![forbid(unsafe_code)]`
- **Arena-allocated evaluation** — `bumpalo`-backed; read-through ops borrow zero-copy from the input
- **`serde_json` is optional** — opt in only when you need the value boundary
- **Configurable** — NaN handling, division-by-zero, truthiness modes, numeric coercion
- **Custom operators** via a simple trait — same idea exposed in every binding
- **Visual debugger + execution tracing** for diagnosing rules

## Performance

`datalogic-rs` is built for repeated evaluation. Compiled rules
dispatch through a single `OpCode` enum (no string lookups), values
live in a `bumpalo::Bump` arena (no per-result heap allocation), and
read-through operators like `var` borrow zero-copy from the caller's
input.

Geomeans across 44 suites (Apple M2 Pro, macOS 26.3, Rust 1.93, Node 24;
median of 3 samples per cell, ~200 ms wall budget — see [`tools/benchmark/BENCHMARK.md`][bench]
for the per-suite matrix, methodology, and caveats):

| Subject                                              | Geomean ns/op | vs `dlrs:engine` |
|------------------------------------------------------|--------------:|-----------------:|
| `dlrs:engine` (datalogic-rs native, precompiled)     |           9.7 |               1× |
| `json-logic-engine:compiled` (TotalTechGeek, JS)     |          47.2 |             4.9× |
| `json-logic-engine` (interpreted, JS)                |         160.3 |            16.5× |
| `jsonlogic-rs` (bestowinc, native Rust)              |         218.0 |            22.5× |
| `json-logic-js` (jwadhams reference, JS)             |         423.5 |            43.7× |
| `dlrs:wasm:compiled` (`@goplasmatic/datalogic` WASM, run under Node)|         855.6 |            88.2× |

The WASM row above measures the WebAssembly build running in Node — the
artifact you'd ship to browsers / Deno / Bun / Cloudflare Workers, not
the Node-native package. Node consumers should reach for
[`@goplasmatic/datalogic-node`](./bindings/node/README.md) (per-platform
napi-rs prebuilds) for production workloads; it shares the same Rust
core as the `dlrs:engine` row above with only the napi boundary added,
so its ceiling sits much closer to native Rust than to WASM. Native-Node
benchmark numbers will land here once the suite is wired up against the
`@goplasmatic/datalogic-node` prebuild.

Numbers are macOS / Apple Silicon — Linux x86_64 will distribute
differently. Quote ratios, not absolute ns/op, when citing.

[bench]: ./tools/benchmark/BENCHMARK.md

## Migrating from v4

v5 is a breaking release with a hard cliff: no `compat` feature, no
deprecated method shims inside the v5 crate. Headline changes:
`DataLogic` → `Engine`, `CompiledLogic` → `Logic`, `Operator` →
`CustomOperator`; one-shot evaluation is now `eval_str` (returns
`String`) or `eval_into::<T>` (returns a typed value); custom operators
receive **pre-evaluated** `&DataValue<'a>` args and an `EvalContext`;
operator registration is builder-only; `serde_json` lives behind the
`serde_json` feature. See [MIGRATION.md](./MIGRATION.md) for the full
v4 → v5 cookbook.

## Resources

- [Full Documentation](https://goplasmatic.github.io/datalogic-rs/) — long-form guide, operator reference, advanced topics
- [Online Playground](https://goplasmatic.github.io/datalogic-rs/playground/) — try rules live
- [Rust API on docs.rs](https://docs.rs/datalogic-rs)
- [JSONLogic Specification](https://jsonlogic.com)
- [Architecture overview](./ARCHITECTURE.md) — how the packages depend on each other
- [Development guide](./DEVELOPMENT.md) — build, test, run, link

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for the contribution workflow,
[DEVELOPMENT.md](./DEVELOPMENT.md) for local setup and per-package
commands, and [ARCHITECTURE.md](./ARCHITECTURE.md) for the cross-package
design.

## About Plasmatic

Created by [Plasmatic](https://github.com/GoPlasmatic), building open-source tools for financial infrastructure and data processing.

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE) for details.
