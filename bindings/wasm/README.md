# @goplasmatic/datalogic-wasm

[![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-wasm)](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

High-performance [JSONLogic](https://jsonlogic.com/) engine for
**browsers, Deno, Bun, Cloudflare Workers, and other edge / non-Node JS
runtimes** — powered by WebAssembly. WASM bindings for
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs).

Same rules, same semantics as the Rust crate: every binding runs the
same core and passes the same 1,532-case conformance battery
(53 suites). For the cross-runtime overview and the API-tier model
that every binding implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **On Node.js? Use
> [`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node)**
> — a native per-platform build that is materially faster than WASM
> under Node; this package is the right pick for browsers, edge, Deno,
> Bun, or a single artifact across Node + browser. Coming from
> `@goplasmatic/datalogic` (v4)? This package is the v5 rename: one
> flag changed (`preserve_structure` → `templating`), see
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md#javascript--npm-consumers).

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
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).
JavaScript surfaces four of the five tiers:

| Tier        | Entry point                            | Use when                                                     |
|-------------|----------------------------------------|--------------------------------------------------------------|
| One-shot    | `evaluate(logic, data, templating)`    | Ad-hoc evaluation, one rule + one data shape                 |
| Compile once | `new CompiledRule(logic, templating)` | Same rule evaluated against many data inputs                 |
| Hot loop    | `engine.session()`                     | Tight loops; one arena reused across evaluations             |
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

**Throws:** a real `Error` object on invalid JSON or evaluation
failure; see [Error handling](#error-handling) for the shape.

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

**Constructor:** `new CompiledRule(logic, templating, config?)`

- `logic` *(string)* — JSON string containing the JSONLogic expression.
- `templating` *(boolean)* — Enable templating mode.
- `config` *(string | object, optional)*: Evaluation config for this
  rule's internal engine, as a JSON string or a plain object. Same keys
  as the engine-level config; see
  [Engine configuration](#engine-configuration). `CompiledRule` is the
  engine-free fast path, so this is how you get, say, strict semantics
  without constructing an `Engine`:

  ```javascript
  const rule = new CompiledRule('{"+": [null, 1]}', false, { preset: 'strict' });
  rule.evaluate('{}'); // throws: strict mode rejects the null operand
  ```

**Methods**

- `evaluate(data: string): string` — evaluate the compiled rule against
  a JSON data string. Returns a JSON string.

### `evaluateWithTrace(logic, data, templating)`

Evaluate and return a step-by-step execution trace. Useful for
inspector UIs and debugging — the React debugger
([`@goplasmatic/datalogic-ui`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/ui/README.md)) consumes this shape
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

## Engine and custom operators

For custom operators (or templating without the boolean flag), construct an
`Engine` with an options object. Each operator callback receives the
pre-evaluated arguments as a JSON-array string and returns a JSON-value
string:

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';
await init();

const engine = new Engine({
  customOperators: {
    double: (argsJson) => String(JSON.parse(argsJson)[0] * 2),
  },
});
engine.evalStr('{"double": [21]}', '{}'); // "42"
```

`Engine` also exposes `compile(logic)` returning a `Rule` for compile-once
reuse. **Built-ins win**: a custom registration of a built-in name (`+`,
`if`, `var`, ...) never dispatches. A custom-operator engine is confined to
the Worker that created it (see Threading below).

The full options bag is
`{ templating?: boolean, customOperators?: Record<string, fn>, config?: string | object }`.
See [Engine configuration](#engine-configuration) for `config`.

### Sessions: hot-loop arena reuse

`engine.session()` opens a `Session`, the hot-loop tier that every other
binding already ships. A session owns one bump arena and resets it at the
start of each `evaluate` call, so a tight loop reuses the same memory
chunks instead of allocating and dropping a fresh arena per call (which
is what `rule.evaluate(data)` does):

```javascript
const engine = new Engine({});
const rule = engine.compile('{"+": [{"var": "a"}, {"var": "b"}]}');
const session = engine.session();

for (const item of batch) {
  const out = session.evaluate(rule, JSON.stringify(item)); // JSON string
  // ...
}
```

**Methods**

- `evaluate(rule: Rule, data: string): string`: evaluate a compiled
  `Rule` against a JSON data string, reusing the session's arena. The
  arena is reset at the start of each call; results are returned as
  owned JSON strings, so they stay valid across later calls.
- `reset(): void`: reset the arena explicitly, returning its chunks to
  their start position without freeing memory. Optional, since
  `evaluate` resets automatically.
- `allocatedBytes(): number`: bytes currently held by the arena's
  chunks. Useful for sizing and diagnostics.

Sessions follow the same threading rule as everything else here: use a
session within the Worker that created it, never across Workers.

## Engine configuration

Both `new Engine({ config })` and `new CompiledRule(logic, templating,
config)` accept an optional evaluation config, either as a JSON string or
as a plain JS object. It maps 1:1 to the Rust engine's
`EvaluationConfig::from_json_str`. All keys are optional; unknown keys or
values throw a `ConfigurationError`:

| Key | Value |
|-----|-------|
| `preset` | `"default"` \| `"safe_arithmetic"` \| `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"` \| `"ignore_value"` \| `"coerce_to_zero"` \| `"return_null"` |
| `division_by_zero` | `"return_saturated"` \| `"throw_error"` \| `"return_null"` \| `"return_infinity"` |
| `loose_equality_errors` | boolean |
| `truthy_evaluator` | `"javascript"` \| `"python"` \| `"strict_boolean"` |
| `numeric_coercion` | object of booleans: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

`preset` applies first; the remaining keys override it individually.

```javascript
// Strict semantics: no silent null-to-zero coercion. The default engine
// evaluates {"+": [null, 1]} to "1"; the strict one throws instead.
const engine = new Engine({ config: { preset: 'strict' } });
engine.evalStr('{"+": [null, 1]}', '{}');
// throws Error { name: "Thrown", thrown: { type: "NaN" }, operator: "+", ... }

// The same config as a JSON string, through the engine-free fast path.
const rule = new CompiledRule('{"+": [null, 1]}', false, '{"preset": "strict"}');
```

## Error handling

Every API throws a real `Error` object. This behavior ships in the
next release (a semver-major bump: 5.0.x rejected with a plain JSON
string, so `e instanceof Error` was `false`) and is tracked in the
[changelog](https://github.com/GoPlasmatic/datalogic-rs/blob/main/CHANGELOG.md).
The thrown object carries:

| Property | Contents |
|----------|----------|
| `name` | Stable error-kind tag: `"ParseError"`, `"InvalidArguments"`, `"VariableNotFound"`, `"TypeError"`, `"ArithmeticError"`, `"Thrown"`, `"IndexOutOfBounds"`, `"ConfigurationError"`, `"Custom"`, ... |
| `message` | Human-readable message, including the failing operator when known |
| `type` | Same tag as `name` (mirrors the wire JSON, kept for migration) |
| `operator` | Outermost failing operator (runtime errors only) |
| `node_ids` | Breadcrumb of compiled-node ids from the failure site toward the root (runtime errors only) |
| variant extras | Kind-specific fields: `variable` (VariableNotFound), `thrown` (Thrown, as a parsed JS value), `index` / `length` (IndexOutOfBounds), `stage` (boundary input errors, e.g. `"parse-data"`) |
| `detailJson` | The exact JSON string that 5.0.x used as the rejection value |

```javascript
try {
  evaluate('{"throw": "limit_exceeded"}', '{}', false);
} catch (e) {
  e instanceof Error; // true
  e.name;             // "Thrown"
  e.message;          // 'Thrown: {"type":"limit_exceeded"} (in operator: throw)'
  e.thrown;           // { type: "limit_exceeded" }  (a real JS object)
  e.operator;         // "throw"
}
```

The two broad categories:

- **Parse errors** (`e.name === "ParseError"`): malformed JSON in either
  argument, or unsupported operator names. Surface immediately.
- **Runtime errors** (everything else): `var` misses (under a strict
  config), arithmetic on non-numbers, explicit `throw` operators. Carry
  the failing `operator` and the `node_ids` path through the compiled
  tree.

### Migrating from 5.0.x

Code that parsed the rejection value keeps working with one property
access:

```javascript
try {
  evaluate(logic, data, false);
} catch (e) {
  // Before (5.0.x): the rejection value was the JSON string itself.
  // const info = JSON.parse(e);

  // After: the fields are already on the error...
  console.error(e.name, e.operator, e.node_ids);

  // ...or re-parse the old payload verbatim if you prefer.
  const info = JSON.parse(e.detailJson);
}
```

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

<!-- canonical-bench v5.0 -->
Geomean across 50 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **9.0 ns/op**, 7.9× faster than json-logic-engine (compiled, the fastest JS engine), 30.3× faster than jsonlogic-rs (the closest Rust alternative), and 102.8× faster than the json-logic-js reference implementation. The WASM build under Node measures 881.9 ns geomean (98× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

WASM-specific notes:

- **Compiled rules** are significantly faster for repeated evaluations
- **Zero-copy** between JS strings and WASM where possible
- **Self-contained module** — roughly 1.6 MB uncompressed, around 400 to 500 KB gzipped
- Measured as `dlrs:wasm:compiled` in the benchmark report

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
wasm-pack test --node
```

This is exactly what CI runs. The suite uses no DOM APIs and is
node-configured on purpose: adding
`wasm_bindgen_test_configure!(run_in_browser)` back would make the node
runner skip every test.

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [Documentation — JavaScript](https://goplasmatic.github.io/datalogic-rs/javascript/installation.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)

## License

Apache-2.0
