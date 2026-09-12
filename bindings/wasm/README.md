# @goplasmatic/datalogic-wasm

[![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-wasm)](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

High-performance [JSONLogic](https://jsonlogic.com/) engine for
**browsers, Deno, Bun, Cloudflare Workers, and other edge / non-Node JS
runtimes** — powered by WebAssembly. WASM bindings for
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs).

Same rules, same semantics as the Rust crate: every binding runs the
same core and passes the same 1,804-case conformance battery
(63 suites). For the cross-runtime overview and the API-tier model
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

// Browser / ES modules: initialise the WASM module once on startup.
// On Node.js the default import is not a function (see "Usage by
// environment" below), so guard the call when the same code runs there.
if (typeof init === 'function') await init();

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

Under Node the bare specifier resolves (via the `node` export
condition) to the CommonJS `nodejs` target, which has no loader: a
default import (`import init from '@goplasmatic/datalogic-wasm'`)
binds `init` to the module namespace object, and `await init()` throws
`TypeError: init is not a function`. Code shared with a browser build
should guard the call (`if (typeof init === 'function') await init();`)
or import `@goplasmatic/datalogic-wasm/nodejs` explicitly.

### Bundlers (Webpack, Vite, …)

```javascript
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic-wasm';
await init();
const result = evaluate('{">=": [{"var": "score"}, 80]}', '{"score": 85}', false);
```

### Explicit target imports

If you need a specific target build:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic-wasm/web';      // web target (call init() first)
import { evaluate }       from '@goplasmatic/datalogic-wasm/bundler';  // bundler target (instantiates on import)
import { evaluate }       from '@goplasmatic/datalogic-wasm/nodejs';   // nodejs target (no init)
```

The bundler target has no `init`: it imports `datalogic_wasm_bg.wasm`
as an ES module and instantiates on load, which needs the bundler's
WASM ESM integration (Webpack's `experiments.asyncWebAssembly`, for
example).

## API reference

The WASM binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding):

| Tier        | Entry point                            | Use when                                                     |
|-------------|----------------------------------------|--------------------------------------------------------------|
| One-shot    | `evaluate(logic, data, templating)`    | Ad-hoc evaluation, one rule + one data shape                 |
| Compile once | `new CompiledRule(logic, templating, config?, templateKeyEscape?)` | Same rule evaluated against many data inputs |
| Hot loop    | `engine.session()`                     | Tight loops; one arena reused across evaluations             |
| Parse once  | `new DataHandle(json)`                 | Same payload evaluated repeatedly (rule sets, bulk scoring); typed + batch results |
| Traced       | `evaluateWithTrace(logic, data, …)` / `engine.evaluateWithTrace(logic, data)` | Debugging, inspector UIs, anything that visualises execution |
| Introspection | `builtinOperatorNames()` / `engine.customOperatorNames()` | Tooling that validates rules against the engine's vocabulary |

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

**Constructor:** `new CompiledRule(logic, templating, config?, templateKeyEscape?)`

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
- `evaluateData(data: DataHandle): string` — evaluate against a
  [parse-once data handle](#datahandle-parse-once-data) instead of a
  string: no data copy or parse per call.

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
//   "expression_tree": { "id": 4, "expression": "{\"and\": [true, {\"var\": \"x\"}]}",
//                        "children": [ { "id": 3, "expression": "{\"var\": \"x\"}", "children": [] } ] },
//   "steps": [ /* 2 steps: the var lookup (node 3), then and (node 4) */ ]
// }
```

Node ids are assigned at compile time, children first, so the root is
the highest id (`id: 0` with an empty `expression` is only the
placeholder returned when compilation fails). Literal operands (the
`true` above) never record a step. Runtime failures do not throw:
`result` is `null`, `error` carries the message, and `structured_error`
the structured form (`{ type, message, operator?, node_ids?, ... }`).

This function uses default engine settings. To trace with a custom
config or custom operators, use `engine.evaluateWithTrace(logic, data)`
on an [`Engine`](#engine-and-custom-operators); it returns the same
envelope:

```javascript
const engine = new Engine({ config: { preset: 'strict' } });
const run = JSON.parse(engine.evaluateWithTrace('{"+": [null, 1]}', '{}'));
run.result;                 // null
run.structured_error.type;  // "Thrown"  (strict mode rejects the null operand)
run.steps[0].error;         // 'Thrown: {"type":"NaN"}'
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
reuse, `evaluateWithTrace(logic, data)` for a trace that honours the
engine's config and operators, and `customOperatorNames()` (below).
**Built-ins win**: a custom registration of a built-in name (`+`,
`if`, `var`, ...) never dispatches. A custom-operator engine is confined to
the Worker that created it (see Threading below).

The full options bag is
`{ templating?: boolean, templateKeyEscape?: string, customOperators?: Record<string, fn>, config?: string | object }`.
See [Engine configuration](#engine-configuration) for `config`.

### Emitting keys that are operator names

A single-key object is an operator invocation, so in templating mode a key
naming a built-in (`type`, `map`, `if`, `length`, ...) or a registered
custom operator runs the operator instead of becoming an output field.
There is no error, just the wrong result. Set `templateKeyEscape` to a
single-character prefix to recover those keys: exactly one leading prefix
is stripped from every template key, and an escaped key is never resolved
as an operator.

```javascript
const engine = new Engine({ templating: true, templateKeyEscape: '$' });

engine.evalStr('{"$type": {"var": "x"}}', '{"x": 1}'); // {"type":1}
engine.evalStr('{"$$type": 1}', '{}');                 // {"$type":1}
engine.evalStr('{"type": {"var": "x"}}', '{"x": 1}');  // "number" (operator)
```

Unset by default, so `$`-prefixed keys otherwise pass through verbatim.
The prefix is one character of your choosing, so payloads that already use
`$` keys (MongoDB documents, JSON Schema output) can pick `~` or `#`
instead. Anything other than a one-character string throws at
construction. `CompiledRule` takes the same value as its fourth argument.

### Operator names

Tooling that validates or autocompletes rules (the visual editor,
linters, palettes) can ask the module for its vocabulary instead of
keeping a hand-maintained list:

```javascript
import init, { builtinOperatorNames, Engine } from '@goplasmatic/datalogic-wasm';
await init();

const names = builtinOperatorNames();
names.length;               // 87: the 84 built-in operators plus the aliases var, ?:, match
names.includes('group_by'); // true
names.includes('preserve'); // false (removed in v5)

const engine = new Engine({ customOperators: { double: (a) => String(JSON.parse(a)[0] * 2) } });
engine.customOperatorNames(); // ["double"]
```

`builtinOperatorNames()` is derived from the compiler's own lookup table
for this build, so it cannot drift from dispatch. `engine.customOperatorNames()`
lists the operators registered through `customOperators` (order not
guaranteed); the union of the two is that engine's full vocabulary,
which matters under templating mode, where an unknown key is not an
error but echoes back as data.

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

## DataHandle: parse-once data

Every string-taking evaluation above copies the data JSON across the
JS↔WASM boundary and re-parses it inside the module **on every call**
— on kilobyte payloads that copy + parse dominates the round trip. A
`DataHandle` removes it: the payload is parsed once and stays resident
in WASM linear memory, so per call only the rule dispatch and the
(usually small) result string cross the boundary. Measured on the
repo's [boundary harness](https://github.com/GoPlasmatic/datalogic-rs/tree/main/tools/benchmark/boundary)
(default build, Apple M2 Pro, Node 24, median of 5), the hot-loop
session path on an 8 KB payload drops from ~30.6 µs/op through strings
to ~3.97 µs/op through a handle (**7.7×**); a ~1 KB payload goes
3.21 µs → 0.73 µs (4.4×), and even a 68-byte payload gains 1.6×
(592 ns → 363 ns).

```javascript
import init, { DataHandle, Engine } from '@goplasmatic/datalogic-wasm';
await init();

const engine = new Engine({});
const rule = engine.compile('{">=": [{"var": "user.age"}, 18]}');
const session = engine.session();

// Parse once...
const handle = new DataHandle('{"user": {"age": 34}}');

// ...evaluate many times (or against many rules): no per-call data copy.
session.evaluateData(rule, handle);   // "true"  (JSON string out)
session.evaluateBool(rule, handle);   // true    (real boolean, no JSON at all)

handle.free(); // release the resident copy after the last evaluation
```

**`new DataHandle(json: string)`** — parses `json` into a resident
document; throws an `Error` named `ParseError` on malformed input.
Handles are **immutable**, never consumed by evaluation, and
independent of any `Engine`: one handle can feed rules and sessions of
different engines, as long as everything lives in the same module
instance (WASM modules are isolated per Worker). Call `free()` after
the last evaluation to release the linear memory eagerly; if you
don't, the same `FinalizationRegistry` glue that backs every class in
this package reclaims it when the JS object is collected
(best-effort — `free()` is the deterministic option).

- `allocatedBytes` *(getter)* — bytes held by the handle's backing
  arena (input copy + parsed tree). Sizing and diagnostics.

**Handle-taking evaluations** (string result out, same errors as the
string path):

- `compiledRule.evaluateData(handle)` / `rule.evaluateData(handle)` —
  fresh arena per call.
- `session.evaluateData(rule, handle)` — the hot path: session arena
  reuse *and* no per-call data work.

### Typed results

Predicate-heavy flows (feature flags, eligibility checks) usually want
a boolean or a number, not a JSON string. The session exposes typed
evaluations over data handles that skip result serialization entirely:

- `session.evaluateBool(rule, handle): boolean` — result must be a
  strict JSON boolean; any other type throws an `Error` named
  `TypeMismatch` (e.g. `"result is not a boolean (got number)"`).
- `session.evaluateNumber(rule, handle): number` — accepts any JSON
  number (JS has one number type); otherwise throws `TypeMismatch`.
- `session.evaluateTruthy(rule, handle): boolean` — collapses **any**
  result through the engine's configured truthiness rules (the same
  coercion `if` / `and` / `or` apply). Never type-mismatches.

The strictness split (`evaluateBool` strict, `evaluateTruthy`
coercing) and the `TypeMismatch` wording are shared with the C ABI,
Go, JVM, .NET, and PHP bindings.

### Batch evaluation

Two batch shapes evaluate N times per boundary call and return one
array of `Promise.allSettled`-style plain objects (**item failures
never fail the call**):

```javascript
// Bulk scoring: one rule × many payloads.
const results = session.evaluateBatch(rule, [handleA, handleB, handleC]);

// Rule set / feature-flag shape: many rules × one payload.
// `rules` are engine.compile(...) Rules (not standalone CompiledRules).
const flags = session.evaluateMany([rule1, rule2, rule3], handle);

for (const outcome of flags) {
  if (outcome.status === 'fulfilled') {
    console.log(outcome.value);        // the item's result as a JSON string
  } else {
    // {tag, message, operator?} — same item-error shape as every binding
    console.warn(outcome.reason.tag, outcome.reason.message);
  }
}
```

Each element of the returned array is one of:

| Shape | Meaning |
|-------|---------|
| `{ status: "fulfilled", value: string }` | Item succeeded; `value` is its result as a JSON string |
| `{ status: "rejected", reason: { tag, message, operator? } }` | Item failed; `tag` is the stable error-kind tag (`"Thrown"`, `"InvalidArgument"`, …), `operator` the outermost failing operator when known |

Per-item failures include evaluation errors *and* invalid elements (a
non-`DataHandle` in `handles`, a non-`Rule` in `rules` — tag
`"InvalidArgument"`). The call itself only throws for argument-level
problems, e.g. passing something that isn't an array. Inputs are
borrowed, never consumed: the same rules/handles arrays can be reused
across calls.

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

Every API throws a real `Error` object. This behavior ships in 5.0.1
(5.0.0 rejected with a plain JSON string, so `e instanceof Error` was
`false`) and is tracked in the
[changelog](https://github.com/GoPlasmatic/datalogic-rs/blob/main/CHANGELOG.md).
The thrown object carries:

| Property | Contents |
|----------|----------|
| `name` | Stable error-kind tag: `"ParseError"`, `"InvalidOperator"`, `"InvalidArguments"`, `"TypeError"`, `"ArithmeticError"`, `"Thrown"`, `"IndexOutOfBounds"`, `"ConfigurationError"`, `"Custom"`, ... plus this binding's `"TypeMismatch"` (typed evaluations whose result has the wrong type) |
| `message` | Human-readable message, including the failing operator when known |
| `type` | Same tag as `name` (mirrors the wire JSON, kept for migration) |
| `operator` | Outermost failing operator (runtime errors only) |
| `node_ids` | Breadcrumb of compiled-node ids from the failure site toward the root (runtime errors only) |
| variant extras | Kind-specific fields: `thrown` (Thrown, as a parsed JS value), `index` / `length` (IndexOutOfBounds), `stage` (boundary input errors, e.g. `"parse-data"`) |
| `detailJson` | The exact JSON string that 5.0.0 used as the rejection value |

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
  argument. Surface immediately, before the engine runs.
- **Runtime errors** (everything else): unknown operator names
  (`"InvalidOperator"`; compiling does not reject them), arithmetic on
  non-numbers (`"Thrown"` with `thrown: { type: "NaN" }`, e.g.
  `{"+": ["abc", 1]}`), explicit `throw` operators, invalid operator
  arguments (`"InvalidArguments"`). Carry the failing `operator` and the
  `node_ids` path through the compiled tree.

A missing variable is not an error under any configuration:
`{"var": "missing"}` evaluates to `null`.

### Migrating from 5.0.0

Code that parsed the rejection value keeps working with one property
access:

```javascript
try {
  evaluate(logic, data, false);
} catch (e) {
  // Before (5.0.0): the rejection value was the JSON string itself.
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

This binding exposes all 84 built-in operators from the Rust engine:

**Logical** — `and`, `or`, `!`, `!!`
**Comparison** — `==`, `===`, `!=`, `!==`, `<`, `<=`, `>`, `>=`
**Arithmetic** — `+`, `-`, `*`, `/`, `%`, `min`, `max`, `abs`, `ceil`, `floor`
**Control flow** — `if`, `?:`, `??` (coalesce), `switch` / `match`
**Array** — `map`, `filter`, `reduce`, `all`, `some`, `none`, `merge`, `in`, `sort`, `slice`, `group_by`, `distinct`
**Object** — `keys`, `values`, `entries`
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

<!-- canonical-bench v5.1 -->
Geomean across 51 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **10.3 ns/op**, 7.0× faster than json-logic-engine (compiled, the fastest JS engine), 28.1× faster than jsonlogic-rs (the closest Rust alternative), and 83.6× faster than the json-logic-js reference implementation. The WASM build under Node measures 900.5 ns geomean (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

WASM-specific notes:

- **Compiled rules** are significantly faster for repeated evaluations
- **Strings are copied across the JS↔WASM boundary in both directions**
  (encode in, decode out), so per-call overhead scales with payload
  size — budget for that on large data. **`DataHandle` removes the
  input half of that cost** when the same payload is evaluated more
  than once: on the boundary harness the hot session loop over an 8 KB
  payload measures ~30.6 µs/op via strings vs ~3.97 µs/op via a handle
  (7.7×), ~1 KB payloads gain 4.4×, tiny ones ~1.6×. Parsing the
  handle costs about one string-path evaluation, so it pays for itself
  from the second evaluation onward — for one-off payloads, stay on
  the string path.
- **Self-contained module**: approximately 2.84 MB uncompressed, around
  600 KB gzipped (5.3.0 release build). Most of the growth since 5.1 is
  the compiled-in IANA timezone database behind the `datetime`
  feature's timezone arguments (5.2.0); see
  [Building from source](#building-from-source) for the
  `CHRONO_TZ_TIMEZONE_FILTER` knob that shrinks it and the measured
  per-profile sizes
- Measured as `dlrs:wasm:compiled` in the benchmark report
- If your data already lives as JS objects and your rules are small, a
  pure-JS engine (e.g. `json-logic-engine`'s compiled mode) runs with
  zero boundary cost and can be faster on raw ns/op for that shape.
  This package earns its keep on full conformance (including the
  extension operators), deterministic latency, sandboxed evaluation,
  and identical behaviour across every runtime

## Building from source

```bash
# Prerequisites
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# Build
cd bindings/wasm
./build.sh   # produces pkg/{web,bundler,nodejs}
```

The `datetime` feature compiles in the full IANA timezone database
(`chrono-tz`), which accounts for roughly 1 MB of the module. If your
rules only ever name a handful of zones, set `chrono-tz`'s build-time
filter (a regular expression matched against zone names) before
building to keep just those:

```bash
CHRONO_TZ_TIMEZONE_FILTER='(UTC|Europe/.*|America/New_York)' ./build.sh
```

The published package is built without a filter so every zone resolves.

### Build profiles

The published package (and a plain `./build.sh`) uses the
size-optimized **release** profile: `opt-level = "z"` plus
`wasm-opt -Oz`. If module size matters less to you than ns/op — e.g. a
server-side WASM deployment where the artifact is fetched once — an
opt-in **speed** profile builds the same code with `opt-level = 3` and
`wasm-opt -O3`:

```bash
WASM_PROFILE=speed ./build.sh   # same pkg/ layout, speed-optimized
```

Measured tradeoff (Apple M2 Pro, Node 24; sizes are the per-target
`.wasm`, speeds from the repo's boundary harness, median of 5). The
release sizes are from the 5.3.0 build, which carries the IANA timezone
table; the speed-profile sizes were measured before that table landed
(5.1, when the release build was 1,746,378 B / 409,269 B gzipped) and
are kept for the relative +8% raw / -1% gzipped tradeoff:

| Measure | release (default) | speed (opt-in) |
|---------|-------------------|----------------|
| `.wasm` size, per target | 2,843,821 B (2.84 MB) | 1,887,386 B pre-tz (+8.1% vs. release at the time) |
| `.wasm` gzipped | 603,466 B | 403,550 B pre-tz (−1.4%) |
| `session.evaluate`, string, 68 B data | 592 ns/op | 439 ns/op (1.35×) |
| `session.evaluate`, string, 8 KB data | 30.6 µs/op | 27.0 µs/op (1.13×) |
| `session.evaluateData`, handle, 8 KB data | 3.97 µs/op | 2.15 µs/op (1.85×) |
| `session.evaluateMany` ×100, handle, 8 KB data | 4.39 µs/eval | 2.38 µs/eval (1.85×) |

(The gzipped transfer size is marginally *smaller* under the speed
profile; the raw-size cost shows up in instantiation memory and
uncompressed serving.)

The default build is unchanged by the existence of this profile — the
speed variant only ships if you build it yourself.

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
