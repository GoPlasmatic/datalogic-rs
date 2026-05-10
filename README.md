<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

# datalogic-rs
**A fast, production-ready engine for JSONLogic — Rust core, WASM bindings, React debugger.**

  [![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
  [![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)
  [![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
  [![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
  [![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic)](https://www.npmjs.com/package/@goplasmatic/datalogic)

</div>

---

## What is datalogic-rs?

`datalogic-rs` is a fast Rust engine for [JSONLogic](http://jsonlogic.com),
a JSON-shaped language for evaluating logical rules against data. Use it
as a **rule engine** for business logic, a **JSON template engine** for
response shaping, or a **safe expression evaluator** for user-supplied
formulas — and run the same rules in Rust, in Node.js, in the browser
via WebAssembly, or in a React visual debugger.

<div align="center">
  <a href="https://goplasmatic.github.io/datalogic-rs/playground/">
    <img src="https://raw.githubusercontent.com/GoPlasmatic/datalogic-rs/main/docs/src/assets/demo.gif" alt="JSONLogic Online Debugger Demo" width="800">
  </a>
  <p><em>Try it live in the <a href="https://goplasmatic.github.io/datalogic-rs/playground/">online playground</a> — no install required.</em></p>
</div>

## Repository layout

This is a monorepo. Every package lives under `packages/`:

| Package                                                                                | Path                  | Language    | Install                              |
|----------------------------------------------------------------------------------------|-----------------------|-------------|--------------------------------------|
| [`datalogic-rs`](https://crates.io/crates/datalogic-rs)                                | `packages/core`       | Rust        | `cargo add datalogic-rs`             |
| [`@goplasmatic/datalogic`](https://www.npmjs.com/package/@goplasmatic/datalogic)       | `packages/wasm`       | Rust → WASM | `npm i @goplasmatic/datalogic`       |
| [`@goplasmatic/datalogic-ui`](https://www.npmjs.com/package/@goplasmatic/datalogic-ui) | `packages/ui`         | React       | `npm i @goplasmatic/datalogic-ui`    |
| `datalogic-bench` (internal)                                                           | `packages/benchmark`  | Rust        | _dev-only, not published_            |

For the cross-package design, dependency flow, and feature-flag matrix,
see [ARCHITECTURE.md](./ARCHITECTURE.md). For local setup, build order,
and per-package commands, see [DEVELOPMENT.md](./DEVELOPMENT.md).

## Which package do I want?

- **JSONLogic in a Rust app** → `packages/core` (`cargo add datalogic-rs`)
- **JSONLogic in Node.js or the browser** → `packages/wasm` (`npm i @goplasmatic/datalogic`)
- **Visual rule editor / debugger in a React app** → `packages/ui` (`npm i @goplasmatic/datalogic-ui`)
- **Compare engines or measure performance** → `packages/benchmark` (dev-only, not published)

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

The same JSONLogic rule runs unchanged across three execution targets.
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

**JavaScript / TypeScript** — Node.js + browser, via WebAssembly:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic';

await init();
const result = evaluate('{">": [{"var": "x"}, 10]}', '{"x": 42}', false);
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

## Quick start (Rust)

```bash
cargo add datalogic-rs
```

```rust
fn main() {
    let result = datalogic_rs::eval_str(r#"{"+": [1, 2, 3]}"#, r#"{}"#).unwrap();
    println!("{}", result); // 6
}
```

That's it. `eval_str` parses the rule, parses the data, evaluates, and
hands you back a JSON string. For typed results use `eval_into::<T>`;
for repeated evaluation see [Compile once, evaluate many](#compile-once-evaluate-many)
below. The `datalogic_rs::` module functions wrap a default `Engine` —
construct one explicitly when you need custom operators, non-default
config, or templating.

## Highlights

- **Cross-platform** — same engine, same rules in Rust, Node.js, browsers (WASM), and React UI
- **59 built-in operators** with full JSONLogic spec compliance
- **Compile once, evaluate millions of times** — `Logic` is `Send + Sync`; share via `Arc`
- **Zero `unsafe`** — built with `#![forbid(unsafe_code)]`
- **Arena-allocated evaluation** — `bumpalo`-backed; read-through ops borrow zero-copy from the input
- **`serde_json` is optional** — opt in only when you need the value boundary
- **Configurable** — NaN handling, division-by-zero, truthiness modes, numeric coercion
- **Custom operators** via a simple trait
- **Visual debugger + execution tracing** for diagnosing rules

## Compile once, evaluate many

For high-throughput callers, compile the rule once and reuse a `Session` —
it owns a reusable arena and resets it between calls, so peak memory
tracks the largest single evaluation. The full pattern (with `Engine`,
`compile`, `session`, and `reset`) lives in
[`examples/compile_once_evaluate_many.rs`](./packages/core/examples/compile_once_evaluate_many.rs).
Power users who want zero-copy `&DataValue<'a>` results can call
`Engine::evaluate` directly with a caller-managed `bumpalo::Bump`.

## Custom operators

Register your own operators on `Engine::builder().add_operator(...)` and
call them from rules just like the built-ins. Arguments arrive
pre-evaluated as arena-resident `&DataValue<'a>` borrows; you allocate
the result back into the arena. Runnable example:
[`examples/custom_operator.rs`](./packages/core/examples/custom_operator.rs).
Full guide:
[Custom Operators](https://goplasmatic.github.io/datalogic-rs/advanced/custom-operators.html).

## Configuration

`EvaluationConfig` controls edge-case behaviour — non-numeric arithmetic,
division by zero, truthiness model, numeric coercion. See the
[Configuration guide](https://goplasmatic.github.io/datalogic-rs/advanced/configuration.html)
for presets (`safe_arithmetic`, `strict`) and per-field options.

## Debugging with traces

Enable the `trace` feature to record every evaluation step. From Rust,
`engine.trace().eval_str(rule, data)` returns a `TracedRun` with
`result` + `steps`. From JavaScript / TypeScript, call
`evaluate_with_trace(logic, data)` from `@goplasmatic/datalogic`. For an
interactive trace view, drop in the React debugger or use the
[online playground](https://goplasmatic.github.io/datalogic-rs/playground/).
See [`examples/tracing.rs`](./packages/core/examples/tracing.rs) for the
full Rust pattern.

## Performance & Benchmarks

`datalogic-rs` is built for repeated evaluation. Compiled rules
dispatch through a single `OpCode` enum (no string lookups), values
live in a `bumpalo::Bump` arena (no per-result heap allocation), and
read-through operators like `var` borrow zero-copy from the caller's
input.

The benchmark harness lives in its own dev-only crate,
`datalogic-bench`, under `packages/benchmark/`. Two binaries share a
common harness:

```bash
# Single-engine benchmark (datalogic-rs alone, fast arena path)
cargo run --release -p datalogic-bench --bin self                 # one suite
cargo run --release -p datalogic-bench --bin self -- --all        # every suite + JSON report

# Cross-library comparison (only datalogic-rs ships by default; add subjects
# behind feature flags — see packages/benchmark/README.md).
cargo run --release -p datalogic-bench --bin compare -- --all
```

Reports land in `packages/benchmark/output/` (gitignored).

### Comparison with other JSONLogic engines

The cross-library matrix in **[`packages/benchmark/BENCHMARK.md`][bench]**
runs every operator suite against every available subject (datalogic-rs
native, our WASM via Node, plus competing Rust and JS libraries) and
reports avg ns/op per cell with arithmetic + geometric mean
aggregation rows.

[bench]: ./packages/benchmark/BENCHMARK.md

Geomeans across 44 suites (Apple M2 Pro, macOS 26.3, Rust 1.93, Node 24;
median of 3 samples per cell, ~200 ms wall budget — see [`BENCHMARK.md`][bench]
for the per-suite matrix, methodology, and caveats):

| Subject                                              | Geomean ns/op | vs `dlrs:engine` |
|------------------------------------------------------|--------------:|-----------------:|
| `dlrs:engine` (datalogic-rs native, precompiled)     |           9.7 |               1× |
| `json-logic-engine:compiled` (TotalTechGeek, JS)     |          47.2 |             4.9× |
| `json-logic-engine` (interpreted, JS)                |         160.3 |            16.5× |
| `jsonlogic-rs` (bestowinc, native Rust)              |         218.0 |            22.5× |
| `json-logic-js` (jwadhams reference, JS)             |         423.5 |            43.7× |
| `dlrs:wasm:compiled` (`@goplasmatic/datalogic`, Node)|         855.6 |            88.2× |

Numbers are macOS / Apple Silicon — Linux x86_64 will distribute
differently. Quote ratios, not absolute ns/op, when citing.

## Migrating from v4

v5 is a breaking release with a hard cliff: no `compat` feature, no
deprecated method shims inside the v5 crate. Headline changes:
`DataLogic` → `Engine`, `CompiledLogic` → `Logic`, `Operator` →
`CustomOperator`; one-shot evaluation is now `eval_str` (returns
`String`) or `eval_into::<T>` (returns a typed value); custom operators
receive **pre-evaluated** `&DataValue<'a>` args and an `EvalContext`;
operator registration is builder-only; `serde_json` lives behind the
`serde_json` feature.

See [MIGRATION.md](./MIGRATION.md) for the full v4 → v5 cookbook with
side-by-side method translations and code examples.

## Resources

- [Full Documentation](https://goplasmatic.github.io/datalogic-rs/)
- [Online Playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [Rust API (docs.rs)](https://docs.rs/datalogic-rs)
- [JSONLogic Specification](https://jsonlogic.com)
- [Architecture overview](./ARCHITECTURE.md)
- [Development guide](./DEVELOPMENT.md)

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for the contribution workflow,
[DEVELOPMENT.md](./DEVELOPMENT.md) for local setup and per-package
commands, and [ARCHITECTURE.md](./ARCHITECTURE.md) for the cross-package
design.

## About Plasmatic

Created by [Plasmatic](https://github.com/GoPlasmatic), building open-source tools for financial infrastructure and data processing.

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE) for details.
