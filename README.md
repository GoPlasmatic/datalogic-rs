<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

# datalogic-rs

**Business rules as data. One engine, every runtime.**

Write a [JSONLogic](https://jsonlogic.com) rule once and evaluate it with the exact same engine in Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP. Not eight reimplementations that drift apart: one Rust core under every binding, evaluating in nanoseconds. Store rules as JSON, change pricing, eligibility, and flag logic in production, and never redeploy to do it.

  [![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/GoPlasmatic/datalogic-rs?label=release)](https://github.com/GoPlasmatic/datalogic-rs/releases)
  [![Conformance](https://img.shields.io/badge/conformance-53_suites_%2F_1,532_cases-brightgreen)](./crates/datalogic-rs/tests/suites/)
  [![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

  [🚀 Try the Live Playground](https://goplasmatic.github.io/datalogic-rs/playground/) | [📖 Read the Documentation](https://goplasmatic.github.io/datalogic-rs/)
</div>

---

<div align="center">
  <a href="https://goplasmatic.github.io/datalogic-rs/playground/">
    <img src="https://raw.githubusercontent.com/GoPlasmatic/datalogic-rs/main/docs/src/assets/demo.gif" alt="JSONLogic Online Debugger Demo" width="800">
  </a>
  <p><em>Build, trace, and debug rules live in the <a href="https://goplasmatic.github.io/datalogic-rs/playground/">playground</a>.</em></p>
</div>

---

## Why datalogic-rs?

- 🌐 **One rule, every runtime:** every binding runs the same compiled Rust core, so a rule evaluates with identical semantics on your backend, your edge workers, and your frontend. No cross-language drift, verified by a 1,532-case conformance battery in CI.
- 🔒 **100% sandbox-safe:** evaluate user-submitted rules and formulas without arbitrary code execution. No `eval()`, no scripting runtime, no I/O; the core forbids unsafe code.
- ⚡ **Nanosecond evaluation:** rules compile to OpCode-dispatched programs that run in a reusable memory arena: 9.0 ns geomean, 7.9× the fastest JS engine, 102.8× the reference implementation.
- 🛠️ **Ready-made rule builder:** ship a visual editor and step-through debugger to your product dashboard with the companion React component, instead of building rule UI from scratch.

---

## One rule, every runtime

Rules are plain JSON, so there is exactly one of them, no matter how many languages you run:

```json
Rule:   {"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "status"}, "active"]}]}
Data:   {"age": 25, "status": "active"}
Result: true
```

The same evaluation, one line in each runtime:

| Runtime | One-shot evaluation |
| :--- | :--- |
| **Rust** | `datalogic_rs::eval_str(rule, data)?` |
| **Node.js** | `apply(rule, data)` — `@goplasmatic/datalogic-node` |
| **Browser / Edge (WASM)** | `evaluate(rule, data, false)` — `@goplasmatic/datalogic-wasm` |
| **Python** | `apply(rule, data)` — `datalogic_py` |
| **Go** | `datalogic.Apply(rule, data)` |
| **Java / Kotlin** | `engine.apply(rule, data)` |
| **.NET (C#)** | `engine.Apply(rule, data)` |
| **PHP** | `$engine->apply($rule, $data)` |

Same bytes in, same bytes out: every binding wraps the same core and passes the same 53-suite conformance battery. Each package README has the full quickstart for its language, and every binding ships the same three runnable programs under its `examples/` folder — the folders themselves are the parity demo.

---

## Pick your package

| Language / Environment | Version | Package | Install | Guide |
| :--- | :--- | :--- | :--- | :--- |
| **Rust** | [![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs) | `datalogic-rs` | `cargo add datalogic-rs` | [crate README](./crates/datalogic-rs/README.md) |
| **Node.js** (native prebuilds) | [![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-node)](https://www.npmjs.com/package/@goplasmatic/datalogic-node) | `@goplasmatic/datalogic-node` | `npm i @goplasmatic/datalogic-node` | [node README](./bindings/node/README.md) |
| **Browser, Edge, Bun, Deno** | [![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-wasm)](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm) | `@goplasmatic/datalogic-wasm` | `npm i @goplasmatic/datalogic-wasm` | [wasm README](./bindings/wasm/README.md) |
| **Python** | [![PyPI](https://img.shields.io/pypi/v/datalogic-py.svg)](https://pypi.org/project/datalogic-py/) | `datalogic-py` | `pip install datalogic-py` | [python README](./bindings/python/README.md) |
| **Go** | [![Go Reference](https://pkg.go.dev/badge/github.com/GoPlasmatic/datalogic-rs/bindings/go/v5.svg)](https://pkg.go.dev/github.com/GoPlasmatic/datalogic-rs/bindings/go/v5) | `datalogic-go` | `go get github.com/GoPlasmatic/datalogic-rs/bindings/go/v5` | [go README](./bindings/go/README.md) |
| **Java / JVM** (Kotlin, Scala) | first Maven release pending<!-- swap for maven-central badge once published --> | `io.github.goplasmatic:datalogic` | Maven / Gradle dependency | [jvm README](./bindings/jvm/README.md) |
| **.NET** (C#, F#) | [![NuGet](https://img.shields.io/nuget/v/Goplasmatic.Datalogic.svg)](https://www.nuget.org/packages/Goplasmatic.Datalogic) | `Goplasmatic.Datalogic` | `dotnet add package Goplasmatic.Datalogic` | [dotnet README](./bindings/dotnet/README.md) |
| **PHP** | [![Packagist](https://img.shields.io/packagist/v/goplasmatic/datalogic.svg)](https://packagist.org/packages/goplasmatic/datalogic) | `goplasmatic/datalogic` | `composer require goplasmatic/datalogic` | [php README](./bindings/php/README.md) |
| **C / FFI** (embed anywhere) | built in-tree | `datalogic-c` | built locally | [c README](./bindings/c/README.md) |
| **React** visual editor | [![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-ui)](https://www.npmjs.com/package/@goplasmatic/datalogic-ui) | `@goplasmatic/datalogic-ui` | `npm i @goplasmatic/datalogic-ui` | [ui README](./ui/README.md) |

---

## Three things you can build

### 1. Dynamic business rules

Encode pricing logic, fee schedules, eligibility and underwriting rules, transaction risk scoring, payment routing, access control, or form validation as JSON. Store rules in a database column, fetch them from an API, review them in a diff: logic changes ship without a deploy.

```json
Rule:   {"if": [
          {">": [{"var": "cart.total"}, 100]}, "free-shipping",
          {">": [{"var": "cart.total"}, 50]},  "flat-rate",
          "standard"
        ]}
Data:   {"cart": {"total": 127.5}}
Result: "free-shipping"
```

### 2. JSON response templates

Enable templating mode and JSON key-value structures flow through to the output, with operators computing fields in place:

```json
Template: {"greeting": {"cat": ["Hello ", {"var": "name"}]},
           "isAdult":  {">=": [{"var": "age"}, 18]}}
Data:     {"name": "Jane", "age": 25}
Output:   {"greeting": "Hello Jane", "isAdult": true}
```

Templating is an engine option in every binding (in Rust: `Engine::builder().with_templating(true)`, behind the `templating` feature).

### 3. Safe user expressions

Let power users and admins write formulas without handing them a scripting engine:

```json
Rule:   {"+": [{"var": "subtotal"}, {"var": "tax"}, {"var": "shipping"}]}
Data:   {"subtotal": 100, "tax": 8.5, "shipping": 5}
Result: 113.5
```

Try any of these live in the [playground](https://goplasmatic.github.io/datalogic-rs/playground/), or browse the [use-case cookbook](https://goplasmatic.github.io/datalogic-rs/use-cases/examples.html) for feature flags, fraud scoring, and data transformation recipes.

---

## 🎨 Visual rule builder and debugger

For admin portals and dashboards where non-engineers author rules, drop `@goplasmatic/datalogic-ui` into your React app. It runs the WASM core internally to compile and trace execution live, and it is the same component behind the online playground.

```tsx
import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

<DataLogicEditor
  value={{ ">": [{ "var": "x" }, 10] }}
  data={{ x: 42 }}
  onChange={(newRule) => console.log('Rules modified:', newRule)}
/>
```

---

## One API shape, every binding

Every binding exposes the same four patterns, so knowledge transfers across your stack:

| Pattern | Shape | Use when |
| :--- | :--- | :--- |
| **One-shot** | `apply(rule, data)` | ad-hoc evaluation, scripts, low volume |
| **Engine** | construct with config / custom operators | non-default semantics, extensions |
| **Compile once** | `engine.compile(rule)` → evaluate many | one rule, many payloads |
| **Session** | `engine.session()` | hot loops; reuses the internal arena across evaluations |

Rust adds two more tiers: zero-copy evaluation into a caller-owned arena, and traced evaluation powering the visual debugger. See the [Rust crate deep-dive](./crates/datalogic-rs/README.md) for the full ladder.

---

## Performance

<!-- canonical-bench v5.0 -->
Geomean execution time across 50 benchmark suites (Apple M2 Pro; median of 3 samples; ratios are pairwise shared-suite geomeans; methodology in [`tools/benchmark/BENCHMARK.md`][bench]):

```text
datalogic-rs (native Rust)              | 9.0 ns   (■) 1x
json-logic-engine (JS, compiled)        | 60.4 ns  (■■■■■■) 7.9x
json-logic-engine (JS, interpreted)     | 236.0 ns (■■■■■■■■■■■■■■■■■■■■■■■■) 30.7x
jsonlogic-rs (bestowinc Rust engine)    | 243.7 ns (■■■■■■■■■■■■■■■■■■■■■■■■) 30.3x
json-logic-js (Reference JS library)    | 433.5 ns (■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■) 102.8x
```

Rules compile to a simple AST with OpCode dispatch (no runtime string matching) and execute inside a reusable memory arena: single-digit nanoseconds for folded rules, 10-120 ns for context-dependent ones.

In Node.js, the native `@goplasmatic/datalogic-node` package is the fast path and runs close to native Rust. The WASM build trades speed for portability (881.9 ns geomean under Node, 98× native, but it runs anywhere JavaScript does). Use native on Node servers; use WASM in browsers, edge runtimes, Deno, and Bun.

Reproduce it yourself: `cargo run --release -p datalogic-bench --bin compare` — full matrix and caveats in [`tools/benchmark/BENCHMARK.md`][bench].

[bench]: ./tools/benchmark/BENCHMARK.md

---

## Engine guarantees

- **Conformance, enforced in CI** — passes the official JSONLogic suite plus an extended cross-binding battery: 1,532 cases across 53 suites, run against the same core every binding ships.
- **59 built-in operators** — comparison, arithmetic, logic, strings, arrays, datetime, error handling; extensible with custom operators authored per host language.
- **Thread-safe evaluation** — compiled `Logic` is `Send + Sync`; share it across threads via `Arc`.
- **Zero `unsafe`** — the core engine forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Zero-copy variables** — `bumpalo`-backed evaluation; read-through operations like `var` borrow directly from the input.
- **Serde-optional** — the default build has no `serde_json` dependency; enable the feature only for typed interop.
- **Configurable semantics** — division-by-zero behavior, NaN handling, truthiness rules, and numeric coercions are all engine options.
- **Verifiable supply chain** — npm packages publish from GitHub Actions with provenance attestation; check with `npm audit signatures`.

### OpenFeature / flagd

The opt-in `flagd` cargo feature (enabled in every language binding) ships the `fractional` and `sem_ver` operators used by [OpenFeature flagd](https://flagd.dev) flag definitions. `fractional` implements murmurhash3 bucketing byte-compatible with the canonical Go evaluator, so users land in the same variant buckets across implementations. That makes the engine usable as an in-process, flagd-compatible feature-flag evaluator in all eight runtimes.

---

## Migrating from v4

v5 contains breaking API updates: `DataLogic` is renamed to `Engine`, `CompiledLogic` to `Logic`, and `Operator` to `CustomOperator`. One-shot evaluation now uses `eval_str` (returning a `String`) or `eval_into::<T>` (for typed values). The npm WASM package moved from `@goplasmatic/datalogic` to `@goplasmatic/datalogic-wasm`. See [MIGRATION.md](./MIGRATION.md) for the step-by-step guide.

---

## Resources

- [Documentation site](https://goplasmatic.github.io/datalogic-rs/) — operator reference, per-language guides, configuration
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/) — build and debug rules in your browser
- [How it compares](https://goplasmatic.github.io/datalogic-rs/comparison.html) — vs json-logic-js, json-logic-engine, jsonlogic-rs, ZEN, CEL
- [Rust API docs on docs.rs](https://docs.rs/datalogic-rs)
- [JSONLogic specification](https://jsonlogic.com)
- [Architecture overview](./ARCHITECTURE.md) · [Development guide](./DEVELOPMENT.md) · [Changelog](./CHANGELOG.md)

---

## Who is using datalogic-rs?

- **[dataflow-rs](https://github.com/GoPlasmatic/dataflow-rs)** (Plasmatic) — workflow/rules automation engine; every route condition is a compiled datalogic rule.
- **[datafake-rs](https://github.com/GoPlasmatic/datafake-rs)** (Plasmatic) — mock JSON data generator configured with JSONLogic expressions.

Running datalogic-rs in production? [Add your project](https://github.com/GoPlasmatic/datalogic-rs/issues/new?title=Who%27s%20using:%20) — a one-line PR or issue is enough.

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for contribution rules, [DEVELOPMENT.md](./DEVELOPMENT.md) for environment setup, and [ARCHITECTURE.md](./ARCHITECTURE.md) for structural diagrams. Questions and ideas are welcome in [Discussions](https://github.com/GoPlasmatic/datalogic-rs/discussions).

## About Plasmatic

Created by [Plasmatic](https://github.com/GoPlasmatic), building open-source tools for financial infrastructure and data processing.

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE) for details.
