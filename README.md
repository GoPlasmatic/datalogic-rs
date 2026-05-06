<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

# datalogic-rs
**A fast, production-ready Rust engine for JSONLogic.**

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

## Three things you can build with it

### 1. Business rules

Encode access control, feature flags, and validation as JSON. Rules are
data — store them in a database, send them over an API, change them
without redeploys.

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let result = engine.evaluate_str(
    r#"{"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "status"}, "active"]}]}"#,
    r#"{"age": 25, "status": "active"}"#,
).unwrap();
assert_eq!(result, "true");
```

### 2. JSON templates

Shape one JSON payload into another. With
`preserve_structure` mode, object keys flow through to the output and
operator values become computed fields — the template's structure
mirrors the response you want.

```rust
// Cargo.toml: datalogic-rs = { version = "5", features = ["preserve"] }
use datalogic_rs::Engine;

let engine = Engine::builder().preserve_structure(true).build();
let result = engine.evaluate_str(
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
use datalogic_rs::Engine;

let engine = Engine::new();
let result = engine.evaluate_str(
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
use datalogic_rs::Engine;

let engine = Engine::new();
let result = engine
    .evaluate_str(r#"{">": [{"var": "x"}, 10]}"#, r#"{"x": 42}"#)
    .unwrap();
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

| Package | Description | Install |
|---------|-------------|---------|
| [datalogic-rs](https://crates.io/crates/datalogic-rs) | Rust library | `cargo add datalogic-rs` |
| [@goplasmatic/datalogic](https://www.npmjs.com/package/@goplasmatic/datalogic) | WASM/JavaScript | `npm i @goplasmatic/datalogic` |
| [@goplasmatic/datalogic-ui](https://www.npmjs.com/package/@goplasmatic/datalogic-ui) | React visual debugger | `npm i @goplasmatic/datalogic-ui` |

See the rule run live in your browser at the
[online playground](https://goplasmatic.github.io/datalogic-rs/playground/).

## Quick start (Rust)

```bash
cargo add datalogic-rs
```

```rust
use datalogic_rs::Engine;

fn main() {
    let engine = Engine::new();
    let result = engine
        .evaluate_str(r#"{"+": [1, 2, 3]}"#, r#"{}"#)
        .unwrap();
    println!("{}", result); // 6
}
```

That's it. `evaluate_str` parses the rule, parses the data, evaluates,
and hands you back a JSON string. For repeated evaluation, see
[Compile once, evaluate many](#compile-once-evaluate-many) below.

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

For high-throughput callers, compile the rule once and reuse a
`Session` — it owns a reusable arena and resets it between calls so
peak memory tracks the largest single evaluation.

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
let mut session = engine.session();

for x in 0..3 {
    let payload = format!(r#"{{"x": {}}}"#, x);
    let result = session.evaluate_str(&compiled, &payload).unwrap();
    assert_eq!(result, (x + 1).to_string());
}
```

Power users who want zero-copy `&DataValue<'a>` results can call
`Engine::evaluate` directly with a caller-managed `bumpalo::Bump`.

## Custom operators

Register your own operators on an `EngineBuilder` and call them from
rules just like the built-ins. Arguments arrive pre-evaluated as
arena-resident `&DataValue<'a>` borrows; you allocate the result back
into the arena.

```rust
use bumpalo::Bump;
use datalogic_rs::operator::ContextStack;
use datalogic_rs::{CustomOperator, DataValue, Engine, Result};

struct Double;
impl CustomOperator for Double {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut ContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
    }
}

let engine = Engine::builder().add_operator("double", Double).build();
let result = engine.evaluate_str(r#"{"double": 21}"#, r#"{}"#).unwrap();
assert_eq!(result, "42");
```

See [Custom Operators](https://goplasmatic.github.io/datalogic-rs/advanced/custom-operators.html) in the docs for the full guide.

## Configuration

`EvaluationConfig` controls behaviour for edge cases — how arithmetic
treats non-numeric values, what division-by-zero returns, which
truthiness model `if`/`and`/`or` use, and how aggressively numeric
coercion runs. See the
[Configuration guide](https://goplasmatic.github.io/datalogic-rs/advanced/configuration.html)
for presets (`safe_arithmetic`, `strict`) and per-field options.

## Debugging with traces

When a rule returns something unexpected, enable the `trace` feature
to see every evaluation step — which branches were taken, which
sub-expressions short-circuited, and what each one returned.

```rust
// Cargo.toml: datalogic-rs = { version = "5", features = ["trace"] }
use datalogic_rs::Engine;

let engine = Engine::new();
let run = engine
    .with_trace()
    .evaluate_str(
        r#"{"if": [{">": [{"var": "age"}, 18]}, "adult", "minor"]}"#,
        r#"{"age": 21}"#,
    );

println!("result: {}", run.result.unwrap());   // "adult"
println!("{} steps recorded", run.steps.len());
```

From JavaScript / TypeScript:

```javascript
import init, { evaluate_with_trace } from '@goplasmatic/datalogic';

await init();
const traced = JSON.parse(evaluate_with_trace(logic, data));
console.log(traced.result, traced.steps);
```

For an interactive view of the trace, drop in the React debugger
(`@goplasmatic/datalogic-ui`) or use the
[online playground](https://goplasmatic.github.io/datalogic-rs/playground/).

## Performance & Benchmarks

`datalogic-rs` is built for repeated evaluation. Compiled rules
dispatch through a single `OpCode` enum (no string lookups), values
live in a `bumpalo::Bump` arena (no per-result heap allocation), and
read-through operators like `var` borrow zero-copy from the caller's
input.

Run the bundled benchmark:

```bash
cargo run --release --example benchmark
```

### Comparison with other JSONLogic engines

> **Coming soon.** Side-by-side benchmarks against `json-logic-js`,
> `json-logic-py`, and other JSONLogic implementations are in progress
> and will be published here. If there's a comparison you'd like to
> see, [open an issue](https://github.com/GoPlasmatic/datalogic-rs/issues).

| Engine          | Simple rule | Complex rule | Notes              |
|-----------------|-------------|--------------|--------------------|
| datalogic-rs    | _TBA_       | _TBA_        | Reference baseline |
| json-logic-js   | _TBA_       | _TBA_        |                    |
| json-logic-py   | _TBA_       | _TBA_        |                    |

## Migrating from v4

v5 is a breaking release. Headline changes: `DataLogic` → `Engine`,
`CompiledLogic` → `Logic`, `Operator` → `CustomOperator`; one-shot
evaluation is now string-based (`evaluate_str`); custom operators
receive **pre-evaluated** `&DataValue<'a>` args; operator registration
is builder-only; `serde_json` moved behind the `compat` feature.

See [docs/src/migration.md](./docs/src/migration.md) for the full
walkthrough — including a transitional `compat::LegacyApi` trait that
keeps v4 method names compiling while you migrate.

## Resources

- [Full Documentation](https://goplasmatic.github.io/datalogic-rs/)
- [Online Playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [Rust API (docs.rs)](https://docs.rs/datalogic-rs)
- [JSONLogic Specification](https://jsonlogic.com)

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for setup, test, and PR
guidelines. Architecture notes live in [CLAUDE.md](./CLAUDE.md).

## About Plasmatic

Created by [Plasmatic](https://github.com/GoPlasmatic), building open-source tools for financial infrastructure and data processing.

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE) for details.
