# @goplasmatic/datalogic-node

[![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-node.svg)](https://www.npmjs.com/package/@goplasmatic/datalogic-node)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Native Node.js bindings for
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs), a fast
Rust implementation of [JSONLogic](http://jsonlogic.com). Same rules,
same semantics as the Rust crate, with the **compile-once /
evaluate-many** pattern exposed natively — compile a rule once and
evaluate it against thousands of data inputs without re-parsing.

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** This native Node binding is new — there is no v4 Node
> package. If you were running JSONLogic under Node via v4's
> `@goplasmatic/datalogic` (WASM), the v5 upgrade path for production
> Node services is to install **this** package. See
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md#npm-package-rename-jsts-consumers-only)
> for the full cookbook.

> **Two npm packages, one engine.** `@goplasmatic/datalogic-wasm` is the
> WebAssembly build — runs in browsers, Node, Deno, Bun. This package
> (`@goplasmatic/datalogic-node`) is the **native** Node build via
> [napi-rs](https://napi.rs/), pulling in the same Rust engine through a
> per-platform prebuilt `.node` artifact. Pick this one when you're on
> Node and want maximum throughput; pick the WASM package when you need
> to run in the browser or want a single artifact across runtimes.

## Install

```bash
npm install @goplasmatic/datalogic-node
```

Prebuilt platform binaries are published as `optionalDependencies`, so
npm pulls only the `.node` file matching the consumer's platform:

| Platform | Architectures |
|---|---|
| Linux (glibc) | x64, arm64 |
| Linux (musl)  | x64, arm64 |
| macOS         | x64, arm64 |
| Windows       | x64, arm64 |

Node 18 and newer are supported.

## Quick start

```js
import { apply } from '@goplasmatic/datalogic-node';

const result = apply(
  { if: [{ '>': [{ var: 'score' }, 50] }, 'pass', 'fail'] },
  { score: 75 }
);
// -> "pass"
```

## Compile-once / evaluate-many

For repeated evaluations of the same rule, compile once and hold the
`Rule` instance:

```js
import { Engine } from '@goplasmatic/datalogic-node';

const engine = new Engine();
const rule = engine.compile({ '+': [{ var: 'x' }, 1] });

for (const payload of inputs) {
  console.log(rule.evaluate(payload));
}
```

`Rule` is safe to share across worker threads — share one instance and
evaluate concurrently.

## Sessions: hot-loop arena reuse

A `Session` reuses one bump arena across evaluations and resets between
calls to bound peak memory. Open one per worker thread:

```js
const sess = engine.session();
for (const payload of inputs) {
  sess.evaluate(rule, payload);
}
```

Sessions hold non-`Sync` state and must not be shared between worker
threads — open one per worker.

## Errors

Failures throw plain JS `Error` instances with structured fields
attached:

```js
try {
  rule.evaluate(data);
} catch (e) {
  if (e.name === 'ParseError') {
    // Malformed rule or data JSON
  } else if (e.name === 'EvaluateError') {
    console.log(e.errorType);  // stable tag (e.g. "TypeError", "Thrown")
    console.log(e.operator);   // outermost failing operator
    console.log(e.nodeIds);    // leaf-to-root breadcrumb
    console.log(e.path);       // resolved root-to-leaf step list
  }
}
```

## API surface

| Symbol | Description |
|---|---|
| `apply(rule, data)` | One-shot compile + evaluate; convenience |
| `Engine` | Construct once; holds compile state, opens sessions |
| `Engine.compile(rule)` → `Rule` | Parse a rule into a reusable handle |
| `Engine.eval(rule, data)` | One-shot, returns JS value |
| `Engine.evalStr(rule, data)` | One-shot, returns JSON string |
| `Engine.session()` → `Session` | Open a hot-loop arena |
| `Rule.evaluate(data)` | Evaluate, returns JS value |
| `Rule.evaluateStr(data)` | Evaluate, returns JSON string |
| `Session.evaluate(rule, data)` | Evaluate with arena reuse |
| `Session.evaluateStr(rule, data)` | Same, returns JSON string |
| `Session.reset()` | Explicit arena reset (optional) |
| `Session.allocatedBytes()` | High-water mark for the arena |

Constructor option:

```ts
new Engine({ templating: true })
```

`templating: true` enables the engine's output-shaping templating mode —
multi-key objects in a rule compile to templates with embedded JSONLogic.

## Custom operators

Register host-language operators by passing a `{ name: fn }` map as the
second constructor argument. Each callback receives the operator's
pre-evaluated arguments as a JSON-array string and returns a JSON-value
string:

```js
import { Engine } from '@goplasmatic/datalogic-node';

const engine = new Engine({}, {
  double: (argsJson) => String(JSON.parse(argsJson)[0] * 2),
});
const rule = engine.compile({ double: [21] });
rule.evaluate({}); // 42
```

Callbacks run synchronously on the thread that created the engine.
**Built-ins win**: registering a name that collides with a built-in
operator (`+`, `if`, `var`, ...) has no effect. An engine carrying custom
operators is **not** safe to share across worker threads (the JS callback
is pinned to its originating thread); create one per worker. A plain
engine or a compiled `Rule` with no custom operators is thread-safe.

## Performance

This package wraps the same Rust engine measured as `dlrs:engine` in the
[cross-library benchmark][bench] — geomean **9.7 ns/op across 44 operator
suites**, ~5× faster than `json-logic-engine` (compiled JS) and ~44×
faster than `json-logic-js`. The napi-rs boundary adds a small per-call
marshalling cost on top; native-Node measurements will land in the
benchmark report once the suite is wired up against the prebuild.

[bench]: https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md

## Building from source

```bash
cd bindings/node
npm install
npx napi build --platform --release
npm test
```

This produces a local `datalogic-node.<platform-triple>.node`, plus
`index.js` and `index.d.ts` loaders. The `.node`, `index.js`, and
`index.d.ts` files are gitignored — `napi build` regenerates them.
