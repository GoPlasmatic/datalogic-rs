# @goplasmatic/datalogic-node

[![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-node.svg)](https://www.npmjs.com/package/@goplasmatic/datalogic-node)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Native Node.js bindings for
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs), a fast
Rust implementation of [JSONLogic](http://jsonlogic.com). Same rules,
same semantics as the Rust crate, with the **compile-once /
evaluate-many** pattern exposed natively — compile a rule once and
evaluate it against thousands of data inputs without re-parsing. Every
binding runs the same core and passes the same 1,565-case conformance
battery (54 suites).

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

## Data handles, typed results, and batch evaluation

New in 5.0.1, mirroring the C ABI v2 tiers. A `DataHandle` is an
immutable, pre-parsed JSON document: parse a payload once with
`new DataHandle(json)` and every evaluation against it skips JSON
parsing entirely. Handles are engine-independent (one handle can feed
rules compiled by different engines) and are never consumed or mutated
by evaluation. They are per-JS-thread — the underlying parsed tree is
`Send` but not `Sync`, which matches JS single-threaded semantics: a
handle cannot be shared across worker threads, so parse one per worker.

```js
import { Engine, DataHandle } from '@goplasmatic/datalogic-node';

const handle = new DataHandle('{"age": 25, "status": "active"}'); // throws ParseError on bad JSON
handle.allocatedBytes;              // arena bytes (input copy + tree)

rule.evaluateData(handle);          // JS value out, no parse per call
rule.evaluateDataStr(handle);       // JSON string out
sess.evaluateData(rule, handle);    // hot path: session arena + no parse
sess.evaluateDataStr(rule, handle); // fastest: no parse, no JS materialisation
```

For predicates and scalar results, the typed session evaluations skip
the JSON result round trip too:

```js
sess.evaluateBool(rule, handle);   // strict JSON boolean
sess.evaluateNumber(rule, handle); // any JSON number (JS has one number type)
sess.evaluateTruthy(rule, handle); // JSONLogic truthiness, never mismatches
```

`evaluateBool` and `evaluateNumber` throw an `EvaluateError` with
`errorType: 'TypeMismatch'` when the rule evaluates fine but the result
is not of the requested type (the message names the actual type).
`evaluateTruthy` coerces any result through the engine's configured
truthiness rules (the same coercion `if`/`and`/`or` apply).

The batch entry points evaluate a whole set in one native call and
report outcomes per item in the `Promise.allSettled` shape, so one bad
input never poisons its neighbours:

```js
// One rule, many payloads:
const outcomes = sess.evaluateBatch(rule, [h0, h1, h2]);
// Many rules, one payload (the rule-set / feature-flag shape):
const flags = sess.evaluateMany([r0, r1], handle);

for (const [i, o] of outcomes.entries()) {
  if (o.status === 'rejected') {
    console.log(`item ${i} failed: ${o.reason.message} (${o.reason.tag})`);
    continue;
  }
  console.log(`item ${i}: ${o.value}`); // result as a JSON string
}
```

Item failures land in `{ status: 'rejected', reason: { tag, message,
operator? } }` and never throw; argument errors (a non-handle in the
array, a null rule, ...) do throw. The session arena is reset between
items.

One Node-specific note on engines: a `Rule` carries a reference to the
engine that compiled it, but every `Session` method evaluates the rule's
compiled logic with the **session's** engine — its configuration and
custom operators apply, and (unlike the C ABI) no engine-identity check
is performed. Compile rules and open sessions on the same engine unless
you specifically want that substitution.

## Async evaluation

`Rule.evaluateStrAsync(dataJson)` evaluates on the libuv thread pool and
returns a `Promise<string>`:

```js
const result = await rule.evaluateStrAsync('{"age": 25}');
```

It is **not** faster per operation than `evaluateStr` — the win is
event-loop hygiene: a large payload's parse + evaluate + serialize runs
off the JS thread, so reach for it when payloads are big enough to
cause noticeable event-loop stalls or to overlap evaluation with other
work. String input only (a `DataHandle` is pinned to the JS thread and
cannot cross to the pool). Rejections carry the same structured fields
as synchronous throws (`name`, `errorType`, `operator`, `nodeIds`,
`path`). Rules from engines with custom operators reject if evaluation
reaches a JS-backed operator — the callback is pinned to the JS thread.

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
| `new DataHandle(json)` | Parse a payload once into a reusable handle |
| `DataHandle.allocatedBytes` | Arena bytes held by the handle |
| `Rule.evaluate(data)` | Evaluate, returns JS value |
| `Rule.evaluateStr(data)` | Evaluate, returns JSON string |
| `Rule.evaluateData(handle)` | Evaluate a pre-parsed handle, returns JS value |
| `Rule.evaluateDataStr(handle)` | Same, returns JSON string |
| `Rule.evaluateStrAsync(dataJson)` | Evaluate on the libuv pool, returns `Promise<string>` |
| `Session.evaluate(rule, data)` | Evaluate with arena reuse |
| `Session.evaluateStr(rule, data)` | Same, returns JSON string |
| `Session.evaluateData(rule, handle)` | Handle in, JS value out, arena reuse |
| `Session.evaluateDataStr(rule, handle)` | Handle in, JSON string out — fastest path |
| `Session.evaluateBool(rule, handle)` | Strict boolean result (`TypeMismatch` otherwise) |
| `Session.evaluateNumber(rule, handle)` | Any JSON number result (`TypeMismatch` otherwise) |
| `Session.evaluateTruthy(rule, handle)` | Engine-truthiness boolean; never mismatches |
| `Session.evaluateBatch(rule, handles)` | One rule × many handles, allSettled-style items |
| `Session.evaluateMany(rules, handle)` | Many rules × one handle, allSettled-style items |
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
Geomean across 50 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **8.9 ns/op**, 7.9× faster than json-logic-engine (compiled, the fastest JS engine), 30.6× faster than jsonlogic-rs (the closest Rust alternative), and 104.2× faster than the json-logic-js reference implementation. The WASM build under Node measures 901.1 ns geomean (101× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The napi-rs boundary adds a small per-call marshalling cost on top of
the core numbers; the measured per-call boundary overhead for this
binding, per API tier, lives in
[BINDINGS-OVERHEAD.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BINDINGS-OVERHEAD.md).

**Pick the path by your data's shape.** Three rules of thumb: compile
once and reuse the `Rule`; when your data is already a JSON string, call
`evaluateStr` — the string path parses directly into the engine and is
the fastest way across the boundary at every payload size; and when the
same payload feeds multiple evaluations, parse it once into a
`DataHandle` — the handle paths skip the per-call parse entirely and are
the fastest tier of all. If your data
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
