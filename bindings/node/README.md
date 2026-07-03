# @goplasmatic/datalogic-node

[![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-node.svg)](https://www.npmjs.com/package/@goplasmatic/datalogic-node)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Native Node.js bindings for
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs), a fast
Rust implementation of [JSONLogic](http://jsonlogic.com). Same rules,
same semantics as the Rust crate, with the **compile-once /
evaluate-many** pattern exposed natively — compile a rule once and
evaluate it against thousands of data inputs without re-parsing. Every
binding runs the same core and passes the same 1,532-case conformance
battery (53 suites).

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
| `Engine.evaluateWithTrace(logic, data)` | One-shot with execution trace, returns JSON string |
| `Engine.session()` → `Session` | Open a hot-loop arena |
| `Rule.evaluate(data)` | Evaluate, returns JS value |
| `Rule.evaluateStr(data)` | Evaluate, returns JSON string |
| `Session.evaluate(rule, data)` | Evaluate with arena reuse |
| `Session.evaluateStr(rule, data)` | Same, returns JSON string |
| `Session.reset()` | Explicit arena reset (optional) |
| `Session.allocatedBytes()` | High-water mark for the arena |

Constructor options:

```ts
new Engine({ templating: true, config: { preset: 'strict' } })
```

`templating: true` enables the engine's output-shaping templating mode —
multi-key objects in a rule compile to templates with embedded JSONLogic.
`config` sets the engine's evaluation configuration; see
[Engine configuration](#engine-configuration).

## Engine configuration

The `config` constructor option changes evaluation semantics. It accepts
a plain object or a JSON-encoded string; both use the wire format every
binding shares, parsed by the core crate's
[`EvaluationConfig::from_json_str`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html).
All keys are optional:

| Key | Values |
|---|---|
| `preset` | `'default'`, `'safe_arithmetic'`, `'strict'` |
| `arithmetic_nan_handling` | `'throw_error'`, `'ignore_value'`, `'coerce_to_zero'`, `'return_null'` |
| `division_by_zero` | `'return_saturated'`, `'throw_error'`, `'return_null'`, `'return_infinity'` |
| `loose_equality_errors` | boolean |
| `truthy_evaluator` | `'javascript'`, `'python'`, `'strict_boolean'` |
| `numeric_coercion` | object of booleans: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

`preset` selects the starting point and the remaining keys override
individual fields on top of it:

```js
const engine = new Engine({ config: { preset: 'strict' } });

// The default engine coerces booleans to numbers; strict rejects them:
engine.evalStr('{"+": [1, true]}', 'null'); // throws EvaluateError
```

Unknown keys or values throw at construction with
`errorType: 'ConfigurationError'`, so typos fail loudly instead of being
silently ignored.

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
is pinned to its originating thread); create one per worker. If a custom
operator is ever invoked from a different thread than the one that
registered it, evaluation fails with an `EvaluateError` naming the
operator rather than risking undefined behavior. A plain engine or a
compiled `Rule` with no custom operators is thread-safe.

## Tracing

`Engine.evaluateWithTrace(logic, data)` evaluates with a step-by-step
execution trace. Both arguments are JSON strings. The return value is a
JSON string with the same envelope the WASM package
(`@goplasmatic/datalogic-wasm`) produces, so trace consumers such as the
React debugger component accept output from either package:

```js
const engine = new Engine();
const run = JSON.parse(
  engine.evaluateWithTrace('{"+": [1, 2, 3]}', 'null')
);
run.result;          // 6
run.steps;           // per-node log: { step_id, node_id, context, result, ... }
run.expression_tree; // compile-time tree: { id, expression, children }
```

Failures do not throw. Instead `result` is `null`, `error` carries the
message, and `structured_error` the structured form. The rule is
compiled with optimization disabled so every operator surfaces a step;
expect it to be slower than `evalStr`. Use it for debugging, not hot
paths.

## Performance

<!-- canonical-bench v5.0 -->
Geomean across 50 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **9.0 ns/op**, 7.9× faster than json-logic-engine (compiled, the fastest JS engine), 30.3× faster than jsonlogic-rs (the closest Rust alternative), and 102.8× faster than the json-logic-js reference implementation. The WASM build under Node measures 881.9 ns geomean (98× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The napi-rs boundary adds a small per-call marshalling cost on top of
the core numbers; the measured per-call boundary overhead for this
binding, per API tier, lives in
[BINDINGS-OVERHEAD.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BINDINGS-OVERHEAD.md).

**Pick the path by your data's shape.** Two rules of thumb: compile once
and reuse the `Rule`, and when your data is already a JSON string, call
`evaluateStr` — the string path parses directly into the engine and is
the fastest way across the boundary at every payload size. If your data
lives as plain JS objects and your rules are small, be aware that a
well-optimized pure-JS engine (e.g. `json-logic-engine`'s compiled mode)
runs with zero boundary cost and can beat any native binding on raw
ns/op for that shape. Reach for this package when you need string
payloads straight from the wire, full conformance including the
extension operators, deterministic latency and bounded memory, parallel
evaluation across worker threads, or the same engine behaving
identically across languages.

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

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [Documentation — Node.js](https://goplasmatic.github.io/datalogic-rs/nodejs/overview.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)
