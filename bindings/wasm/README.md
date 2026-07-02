# @goplasmatic/datalogic-wasm

[![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-wasm)](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

High-performance [JSONLogic](https://jsonlogic.com/) engine for
**browsers, Deno, Bun, Cloudflare Workers, and other edge / non-Node JS
runtimes** — powered by WebAssembly. WASM bindings for
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs).

Same rules, same semantics as the Rust crate. For the cross-runtime
overview and the API-tier model that every binding implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **Coming from `@goplasmatic/datalogic` (v4)?** This package is the v5
> rename — same WASM engine, one JS-surface flag renamed
> (`preserve_structure` → `templating`). See
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md#javascript--npm-consumers)
> for the cookbook.

> **On Node.js? Use the native binding instead.**
> [`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node)
> ships a per-platform native build via [napi-rs](https://napi.rs) and is
> materially faster than the WASM path for Node workloads. This package
> still works under Node (and is the right pick when you want a single
> artifact across Node + browser), but production Node services should
> reach for `@goplasmatic/datalogic-node` first.

## Install

```bash
npm install @goplasmatic/datalogic-wasm
```

The published package is **pre-built** — no Rust or WASM toolchain
required to consume it. If you want to build from source instead, see
[Building from source](#building-from-source).

## Quick start

```javascript
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic-wasm';

// Browser / ES modules — initialise the WASM module once on startup.
// (Skip this on Node.js — see "Usage by environment" below.)
await init();

// One-shot evaluation
const result = evaluate('{"==": [1, 1]}', '{}', false);
console.log(result); // "true"

// With data
const score = evaluate('{"var": "user.age"}', '{"user": {"age": 25}}', false);
console.log(score); // "25"

// Compile once, evaluate many — faster for repeated calls
const rule = new CompiledRule('{"+": [{"var": "a"}, {"var": "b"}]}', false);
console.log(rule.evaluate('{"a": 1,  "b": 2}'));  // "3"
console.log(rule.evaluate('{"a": 10, "b": 20}')); // "30"
```

## Usage by environment

### Browser (ES modules)

```html
<script type="module">
  import init, { evaluate } from '@goplasmatic/datalogic-wasm';
  await init();
  const result = evaluate('{"and": [true, {"var": "active"}]}',
                          '{"active": true}', false);
  console.log(result); // "true"
</script>
```

### Node.js (WASM path)

For most Node workloads you should prefer the native binding —
[`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node).
The WASM path below is supported and works fine; reach for it when you
want a single artifact shared between a Node backend and a browser
frontend, or when per-platform native prebuilds are a non-starter for
your deployment.

```javascript
import { evaluate, CompiledRule } from '@goplasmatic/datalogic-wasm';

// No init() needed for Node.js
const result = evaluate('{"==": [1, 1]}', '{}', false);
```

### Bundlers (Webpack, Vite, …)

```javascript
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic-wasm';
await init();
const result = evaluate('{">=": [{"var": "score"}, 80]}', '{"score": 85}', false);
```

### Explicit target imports

If you need a specific target build:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic-wasm/web';      // web target
import init, { evaluate } from '@goplasmatic/datalogic-wasm/bundler';  // bundler target
import { evaluate }       from '@goplasmatic/datalogic-wasm/nodejs';   // nodejs target
```

## API reference

The WASM binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#choosing-your-api-five-tiers-one-engine).
JavaScript surfaces three of the five tiers:

| Tier        | Entry point                            | Use when                                                     |
|-------------|----------------------------------------|--------------------------------------------------------------|
| One-shot    | `evaluate(logic, data, templating)`    | Ad-hoc evaluation, one rule + one data shape                 |
| Compile once | `new CompiledRule(logic, templating)` | Same rule evaluated against many data inputs                 |
| Traced       | `evaluateWithTrace(logic, data, …)`   | Debugging, inspector UIs, anything that visualises execution |

### `evaluate(logic, data, templating)`

One-shot evaluation. Parses the rule each call — fine for ad-hoc use,
but reach for `CompiledRule` if you call this in a loop.

**Parameters**

- `logic` *(string)* — JSON string containing the JSONLogic expression.
- `data` *(string)* — JSON string containing the data to evaluate against.
- `templating` *(boolean)* — If `true`, enables templating mode: multi-key
  objects compile to output-shaping templates with embedded JSONLogic.

**Returns** — JSON string with the result.

**Throws** — `Error` (with a string message) on invalid JSON or
evaluation failure.

```javascript
evaluate('{"==": [{"var": "x"}, 5]}', '{"x": 5}', false);             // "true"
evaluate('{"+": [1, 2, 3]}', '{}', false);                            // "6"
evaluate('{"map": [[1,2,3], {"+": [{"var": ""}, 1]}]}', '{}', false); // "[2,3,4]"

// Templating mode — multi-key object becomes a response template
evaluate('{"name": {"var": "user"}, "active": true}',
         '{"user": "Alice"}', true);
// '{"name":"Alice","active":true}'
```

### `CompiledRule`

A compiled JSONLogic rule for repeated evaluation. Pre-compiling pays
off as soon as you evaluate the same rule against more than one data
input.

```javascript
const rule = new CompiledRule('{">=": [{"var": "age"}, 18]}', false);
rule.evaluate('{"age": 21}'); // "true"
rule.evaluate('{"age": 16}'); // "false"
```

**Constructor** — `new CompiledRule(logic, templating)`

- `logic` *(string)* — JSON string containing the JSONLogic expression.
- `templating` *(boolean)* — Enable templating mode.

**Methods**

- `evaluate(data: string): string` — evaluate the compiled rule against
  a JSON data string. Returns a JSON string.

### `evaluateWithTrace(logic, data, templating)`

Evaluate and return a step-by-step execution trace. Useful for
inspector UIs and debugging — the React debugger
([`@goplasmatic/datalogic-ui`](../../ui/README.md)) consumes this shape
directly.

**Returns** — JSON string containing a `TracedResult`:

```javascript
const trace = evaluateWithTrace('{"and": [true, {"var": "x"}]}',
                                '{"x": true}', false);
JSON.parse(trace);
// {
//   "result": true,
//   "expression_tree": { "id": 0, "expression": "{\"and\": [...]}", ... },
//   "steps": [ /* per-node execution steps */ ]
// }
```

## Error handling

Both `evaluate` and `CompiledRule.evaluate` throw on failure. The thrown
`Error.message` carries a JSON-formatted error shape — useful for
distinguishing parse errors from runtime errors programmatically:

```javascript
try {
  evaluate('not valid json', '{}', false);
} catch (e) {
  // e.message contains the engine's error shape
  console.error(e.message);
}
```

The two broad categories:

- **Parse errors** — malformed JSON in either argument, or unsupported
  operator names. Surface immediately.
- **Runtime errors** — `var` misses (under a strict config),
  arithmetic on non-numbers, explicit `throw` operators. Carry the
  failing operator and the node path through the compiled tree.

## Threading & Web Workers

The WASM module is **isolated per Web Worker**: each Worker loads its
own copy of the module, so a `CompiledRule` created in one Worker
cannot be transferred to another. Within a single Worker, evaluation
is synchronous and single-threaded — share a `CompiledRule` across
calls in the same context, not across Workers.

If you need true parallelism, spawn N Workers and compile the rule N
times (once per Worker). The compile cost is small relative to the
isolation benefit.

## Supported operators

This binding exposes all 59 built-in operators from the Rust engine:

**Logical** — `and`, `or`, `!`, `!!`
**Comparison** — `==`, `===`, `!=`, `!==`, `<`, `<=`, `>`, `>=`
**Arithmetic** — `+`, `-`, `*`, `/`, `%`, `min`, `max`, `abs`, `ceil`, `floor`
**Control flow** — `if`, `?:`, `??` (coalesce), `switch` / `match`
**Array** — `map`, `filter`, `reduce`, `all`, `some`, `none`, `merge`, `in`, `sort`, `slice`
**String** — `cat`, `substr`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`, `length`
**Data access** — `var`, `val`, `exists`, `missing`, `missing_some`
**Date/time** — `now`, `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`
**Error handling** — `try`, `throw`
**Type** — `type`
**Feature flags (flagd)** — `fractional`, `sem_ver`

> **Templating mode:** v5 removed the `preserve` *operator*. To enable
> JSON templates with embedded JSONLogic (multi-key objects become
> output-shaping templates), pass `templating: true` to `evaluate` or
> `new CompiledRule(logic, true)`.

For the full operator reference and semantics, see the
[documentation site](https://goplasmatic.github.io/datalogic-rs/).

## Performance

- **Compiled rules** are significantly faster for repeated evaluations
- **Zero-copy** between JS strings and WASM where possible
- **Self-contained module** — roughly 1.6 MB uncompressed, around 400 to 500 KB gzipped

For numbers, see the cross-library benchmark matrix in
[`tools/benchmark/BENCHMARK.md`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md).
The WASM subject is included as `dlrs:wasm:compiled` — slower than
the native Rust engine by design (the JS↔WASM boundary has a fixed
cost) but still competitive with pure-JS implementations.

## Building from source

```bash
# Prerequisites
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# Build
cd bindings/wasm
./build.sh   # produces pkg/{web,bundler,nodejs}
```

### Tests

```bash
wasm-pack test --headless --chrome
wasm-pack test --headless --firefox
```

## Learn more

- [Repo README](https://github.com/GoPlasmatic/datalogic-rs#readme) — cross-runtime overview, all binding READMEs
- [Rust crate README](../../crates/datalogic-rs/README.md) — engine design, the 5-tier API model, custom operators
- [React debugger](../../ui/README.md) — `@goplasmatic/datalogic-ui`, consumes this binding
- [Full documentation](https://goplasmatic.github.io/datalogic-rs/) — long-form guide, operator reference
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/) — try rules live
- [JSONLogic specification](https://jsonlogic.com/)

## License

Apache-2.0
