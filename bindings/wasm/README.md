# @goplasmatic/datalogic-wasm

[![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-wasm)](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs): one engine, every runtime.

[JSONLogic](https://jsonlogic.com/) engine for **browsers, Deno, Bun,
Cloudflare Workers, and other edge / non-Node JS runtimes**, compiled to
WebAssembly from [`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs).
You compile a rule once and evaluate it against many payloads, with the
same rules and semantics as the Rust crate. Every binding runs the same
core and passes the same 2,128-case conformance battery (66 suites).

For the cross-runtime overview and the API-tier model that every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **On Node.js, use
> [`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node)**,
> a native per-platform build that runs faster than WASM under Node.
> This package fits browsers, edge, Deno, Bun, or a single artifact
> across Node and browser. If you are coming from
> `@goplasmatic/datalogic` (v4), this package is the v5 rename: one
> flag changed (`preserve_structure` → `templating`); see
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md#javascript--npm-consumers).

## Install

```bash
npm install @goplasmatic/datalogic-wasm
```

The published package is **pre-built**, so you need no Rust or WASM
toolchain to consume it. It declares Node 18 or newer
(`engines.node`). To build from source instead, see
[Building from source](#building-from-source).

## Quick start

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';

// Browser / ES modules: initialise the WASM module once on startup.
// On Node.js the default import is not a function (see "Usage by
// environment" below), so guard the call when the same code runs there.
if (typeof init === 'function') await init();

const engine = new Engine();

// One-shot evaluation: rule and data go in as JSON strings, the result
// comes back as a JSON string
engine.evalStr('{"==": [1, 1]}', '{}');                         // "true"
engine.evalStr('{"var": "user.age"}', '{"user": {"age": 25}}'); // "25"

// Compile once, evaluate many: faster for repeated calls
const rule = engine.compile('{"+": [{"var": "a"}, {"var": "b"}]}');
rule.evaluate('{"a": 1,  "b": 2}');  // "3"
rule.evaluate('{"a": 10, "b": 20}'); // "30"
```

## Usage by environment

### Browser (ES modules)

```html
<script type="module">
  import init, { Engine } from '@goplasmatic/datalogic-wasm';
  await init();
  const engine = new Engine();
  const result = engine.evalStr('{"and": [true, {"var": "active"}]}',
                                '{"active": true}');
  console.log(result); // "true"
</script>
```

### Node.js (WASM path)

For most Node workloads, prefer the native binding,
[`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node).
Reach for the WASM path when you want a single artifact shared between
a Node backend and a browser frontend, or when your deployment cannot
use per-platform native prebuilds.

```javascript
import { Engine } from '@goplasmatic/datalogic-wasm';

// No init() on Node.js
const engine = new Engine();
const result = engine.evalStr('{"==": [1, 1]}', '{}');
```

Under Node the bare specifier resolves (via the `node` export
condition) to the CommonJS `nodejs` target, which has no loader: a
default import (`import init from '@goplasmatic/datalogic-wasm'`)
binds `init` to the module namespace object, and `await init()` throws
`TypeError: init is not a function`. Code shared with a browser build
should guard the call (`if (typeof init === 'function') await init();`)
or import `@goplasmatic/datalogic-wasm/nodejs`.

### Bundlers (Webpack, Vite, …)

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';
await init();
const engine = new Engine();
const result = engine.evalStr('{">=": [{"var": "score"}, 80]}', '{"score": 85}');
```

### Explicit target imports

If you need a specific target build:

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm/web';      // web target (call init() first)
import { Engine }       from '@goplasmatic/datalogic-wasm/bundler';  // bundler target (instantiates on import)
import { Engine }       from '@goplasmatic/datalogic-wasm/nodejs';   // nodejs target (no init)
```

The bundler target has no `init`: it imports `datalogic_wasm_bg.wasm`
as an ES module and instantiates on load, which needs the bundler's
WASM ESM integration (Webpack's `experiments.asyncWebAssembly`, for
example).

## Compile once, evaluate many

`engine.compile(logic)` parses the rule once and returns a `Rule`. A
`Rule` keeps its engine's config and custom operators, and each
`rule.evaluate(data)` skips the parse:

```javascript
const engine = new Engine();
const rule = engine.compile('{">=": [{"var": "age"}, 18]}');
rule.evaluate('{"age": 21}'); // "true"
rule.evaluate('{"age": 16}'); // "false"
```

Every class in this package holds WASM memory. A `FinalizationRegistry`
reclaims it when the JS object is collected (best-effort); call
`rule.free()` when you stop using a rule you create often (per render,
per request).

### Templating mode per compile

`new Engine({ templating: true })` makes every `compile` treat a
multi-key object as an output template. To choose per rule instead,
`engine.compileTemplate(logic)` compiles in templating mode and
`engine.compileStrict(logic)` outside it, whatever the engine was built
with. Both use the engine's custom operators and config:

```javascript
const engine = new Engine();
const template = engine.compileTemplate('{"name": {"var": "user"}, "active": true}');
template.evaluate('{"user": "Alice"}'); // '{"name":"Alice","active":true}'
```

### Checking a rule before it runs

`compile` checks the JSON and the rule's structure only: an unknown
operator compiles and fails at evaluation with `InvalidOperator`.
`engine.check(logic, mode?)` reports every problem the engine can see
before the rule runs, as a JSON array of
`{ code, severity, message, pointer, operator }` objects. `pointer` is an
RFC 6901 JSON Pointer into the rule, and `mode` is `"engine"` (the
default), `"strict"` or `"template"`:

```javascript
JSON.parse(engine.check('{"if": [true, {"vr": "x"}, {"map": [1]}]}'));
// [
//   { code: "UnknownOperator", severity: "error", pointer: "/if/1", operator: "vr",
//     message: "unknown operator `vr`; did you mean `var`?" },
//   { code: "ArgumentCount", severity: "error", pointer: "/if/2", operator: "map",
//     message: "`map` takes exactly 2 arguments, not 1" }
// ]
```

Errors are what will fail: an unknown operator, a multi-key object
outside templating mode, an argument count the operator rejects, a
literal timezone that does not exist. Warnings run but are probably
mistakes, such as arguments an operator never evaluates.
`engine.compileChecked(logic)` compiles only a rule with no error and
otherwise throws an `Error` named `CompileError` whose `diagnostics`
property holds the same objects:

```javascript
try {
  engine.compileChecked('{"vr": "x"}');
} catch (e) {
  e.name;                 // "CompileError"
  e.diagnostics[0].code;  // "UnknownOperator"
}
```

## Sessions: hot-loop arena reuse

`engine.session()` opens a `Session`, the hot-loop tier. A session owns
one bump arena and resets it at the start of each `evaluate` call, so a
tight loop reuses the same memory chunks instead of allocating and
dropping a fresh arena per call (which is what `rule.evaluate(data)`
does):

```javascript
const engine = new Engine();
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
- `reset(): void`: reset the arena, returning its chunks to their start
  position without freeing memory. Optional, since every `evaluate*`
  call resets first.
- `allocatedBytes(): number`: bytes held by the arena's chunks. Useful
  for sizing and diagnostics.

Use a session within the Worker that created it, never across Workers.

## DataHandle: parse-once data

Every string-taking evaluation above copies the data JSON across the
JS↔WASM boundary and re-parses it inside the module **on every call**.
On kilobyte payloads that copy + parse dominates the round trip. A
`DataHandle` removes it: you parse the payload once and it stays resident
in WASM linear memory, so per call only the rule dispatch and the
result string cross the boundary. Measured on the
repo's [boundary harness](https://github.com/GoPlasmatic/datalogic-rs/tree/main/tools/benchmark/boundary)
(default build, Apple M2 Pro, Node 24, median of 5), the hot-loop
session path on an 8 KB payload drops from ~30.6 µs/op through strings
to ~3.97 µs/op through a handle (**7.7×**); a ~1 KB payload goes
3.21 µs → 0.73 µs (4.4×), and a 68-byte payload gains 1.6×
(592 ns → 363 ns).

```javascript
import init, { DataHandle, Engine } from '@goplasmatic/datalogic-wasm';
await init();

const engine = new Engine();
const rule = engine.compile('{">=": [{"var": "user.age"}, 18]}');
const session = engine.session();

// Parse once...
const handle = new DataHandle('{"user": {"age": 34}}');

// ...evaluate many times (or against many rules): no per-call data copy.
session.evaluateData(rule, handle);   // "true"  (JSON string out)
session.evaluateBool(rule, handle);   // true    (real boolean, no JSON at all)

handle.free(); // release the resident copy after the last evaluation
```

**`new DataHandle(json: string)`**: parses `json` into a resident
document; throws an `Error` named `ParseError` on malformed input.
Handles are **immutable**, never consumed by evaluation, and
independent of any `Engine`: one handle can feed rules and sessions of
different engines, as long as everything lives in the same module
instance (WASM modules are isolated per Worker). Call `free()` after
the last evaluation to release the linear memory; without it, the
`FinalizationRegistry` reclaims the handle when the JS object is
collected (best-effort).

- `allocatedBytes` *(getter)*: bytes held by the handle's backing
  arena (input copy + parsed tree). Sizing and diagnostics.

**Handle-taking evaluations** (string result out, same errors as the
string path):

- `rule.evaluateData(handle)`: fresh arena per call.
- `session.evaluateData(rule, handle)`: the hot path, with session arena
  reuse *and* no per-call data work.

### Typed results

Predicate-heavy flows (feature flags, eligibility checks) want a
boolean or a number, not a JSON string. The session exposes typed
evaluations over data handles that skip result serialization:

- `session.evaluateBool(rule, handle): boolean`: result must be a
  strict JSON boolean; any other type throws an `Error` named
  `TypeMismatch` (e.g. `"result is not a boolean (got number)"`).
- `session.evaluateInt(rule, handle): number`: a whole JSON number a JS
  number holds exactly (`|n| <= 2^53 - 1`); otherwise throws
  `TypeMismatch`.
- `session.evaluateFloat(rule, handle): number`: accepts any JSON
  number; otherwise throws `TypeMismatch`.
- `session.evaluateTruthy(rule, handle): boolean`: collapses **any**
  result through the engine's configured truthiness rules (the same
  coercion `if` / `and` / `or` apply). Never type-mismatches.

The strictness split (`evaluateBool` strict, `evaluateTruthy`
coercing) and the `TypeMismatch` wording are shared with the C ABI,
Go, JVM, .NET, and PHP bindings. `evaluateNumber` is the deprecated
name of `evaluateFloat`.

### Batch evaluation

Two batch shapes evaluate N times per boundary call and return one
array of `Promise.allSettled`-style plain objects (**item failures
never fail the call**):

```javascript
// Bulk scoring: one rule × many payloads.
const results = session.evaluateBatch(rule, [handleA, handleB, handleC]);

// Rule set / feature-flag shape: many rules × one payload.
// `rules` are Rules from engine.compile(...).
const flags = session.evaluateMany([rule1, rule2, rule3], handle);

for (const outcome of flags) {
  if (outcome.status === 'fulfilled') {
    console.log(outcome.value);        // the item's result as a JSON string
  } else {
    // {tag, message, operator?}: same item-error shape as every binding
    console.warn(outcome.reason.tag, outcome.reason.message);
  }
}
```

Each element of the returned array is one of:

| Shape | Meaning |
|-------|---------|
| `{ status: "fulfilled", value: string }` | Item succeeded; `value` is its result as a JSON string |
| `{ status: "rejected", reason: { tag, message, operator? } }` | Item failed; `tag` is the stable error-kind tag (`"Thrown"`, `"InvalidArgument"`, …), `operator` the innermost failing operator when known |

Per-item failures include evaluation errors *and* invalid elements (a
non-`DataHandle` in `handles`, a non-`Rule` in `rules`; tag
`"InvalidArgument"`). The call itself only throws for argument-level
problems, such as passing something that isn't an array. Inputs are
borrowed, never consumed: you can reuse the same rules and handles
arrays across calls.

## API reference

The WASM binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding):

| Tier          | Entry point                                   | Use when                                                     |
|---------------|-----------------------------------------------|--------------------------------------------------------------|
| One-shot      | `engine.evalStr(logic, data)`                 | Ad-hoc evaluation, one rule + one data shape                 |
| Compile once  | `engine.compile(logic)` → `Rule`              | Same rule evaluated against many data inputs                 |
| Hot loop      | `engine.session()` → `Session`                | Tight loops; one arena reused across evaluations             |
| Data handle   | `new DataHandle(json)`                        | Same payload evaluated repeatedly (rule sets, bulk scoring)  |
| Typed         | `session.evaluateBool` / `evaluateInt` / `evaluateFloat` / `evaluateTruthy` | A boolean or number result without JSON |
| Batch         | `session.evaluateBatch` / `session.evaluateMany` | Many payloads or many rules in one boundary call          |
| Metered       | `engine.evalMetered` / `rule.evaluateMetered` / `session.evaluateMetered` | What an evaluation costs, with an optional cap |
| Traced        | `engine.evaluateWithTrace(logic, data, mode?)` | Debugging, inspector UIs, anything that visualises execution |
| Checked       | `engine.check(logic, mode?)` / `engine.compileChecked(logic)` | Finding a rule's mistakes before it runs   |
| Introspection | `builtinOperatorNames()`, `engine.operators()`, `rule.facts()`, `engine.customOperatorNames()`, `engine.truthy(value)` | Tooling that works with the engine's vocabulary and a rule's reads |

Rules, data and results cross the boundary as JSON strings, except the
typed session methods, `truthy`, the name lists and the batch outcome
arrays. Every class has `free()`.

| Class | Members |
|-------|---------|
| `Engine` | `new Engine(options?)`, `compile`, `compileTemplate`, `compileStrict`, `compileChecked`, `check`, `evalStr`, `evalMetered`, `evaluateWithTrace`, `session`, `operators`, `truthy`, `customOperatorNames` |
| `Rule` | `evaluate(data)`, `evaluateData(handle)`, `evaluateMetered(data, budget?)`, `facts()` |
| `Session` | `evaluate`, `evaluateData`, `evaluateMetered`, `evaluateBool`, `evaluateInt`, `evaluateFloat`, `evaluateTruthy`, `evaluateBatch`, `evaluateMany`, `reset`, `allocatedBytes` |
| `DataHandle` | `new DataHandle(json)`, `allocatedBytes` |

The `Engine` options bag is
`{ templating?, templateKeyEscape?, customOperators?, config?, strictOperatorNames?, families? }`;
every key is optional.

### Deprecated APIs

These still work in 5.x and are removed in 6.0:

| Deprecated | Use instead |
|------------|-------------|
| `evaluate(logic, data, templating)` | `new Engine({ templating }).evalStr(logic, data)`, or `engine.compileTemplate(logic)` for one template |
| `evaluateWithTrace(logic, data, templating)` | `engine.evaluateWithTrace(logic, data, mode?)`, which also applies the engine's config and custom operators |
| `new CompiledRule(logic, templating, config?, templateKeyEscape?)` | `new Engine({ templating, config, templateKeyEscape }).compile(logic)`; the `Rule` has the same `evaluate` / `evaluateData` methods |
| `session.evaluateNumber(rule, handle)` | `session.evaluateFloat(rule, handle)` |

## Custom operators

Construct an `Engine` with `customOperators` to register JS functions as
operators. Each callback receives the pre-evaluated arguments as a
JSON-array string and returns a JSON-value string:

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

Returning `null` or `undefined` counts as JSON `null`; a thrown
exception or a non-string return becomes a runtime evaluation error.
**Built-ins win**: a custom registration of a built-in name (`+`, `if`,
`var`, ...) never dispatches. Pass `strictOperatorNames: true` to make
that registration throw a `ConfigurationError` at construction instead.
A custom-operator engine is confined to the Worker that created it (see
[Threading & Web Workers](#threading--web-workers)).

### Emitting keys that are operator names

A single-key object is an operator invocation, so in templating mode a key
naming a built-in (`type`, `map`, `if`, `length`, ...) or a registered
custom operator runs the operator instead of becoming an output field.
There is no error, only the wrong result. Set `templateKeyEscape` to a
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
construction.

## Introspection

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
guaranteed). For an engine with every operator family, the union of the
two is its full vocabulary, which matters under templating mode, where
an unknown key is not an error but echoes back as data. On an engine
built with `families`, `engine.operators()` lists the built-ins that
engine evaluates.

### Operator catalogue

`engine.operators()` describes every built-in operator the engine
evaluates, as a JSON array in the schema of the
[operator catalogue](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json):
canonical name and aliases, family and gating feature, the argument
counts it reads, whether it reads the data context, its effect
(`pure`, `clock`, `throws`, `catches`), its cost class, and which
argument runs once per element:

```javascript
const catalogue = JSON.parse(engine.operators());
catalogue.length; // 84
catalogue.find((op) => op.name === 'reduce');
// { name: "reduce", aliases: [], family: "Core", feature: null, min_args: 2,
//   max_args: 3, reads_context: false, effect: "pure", cost: "per_item", scoped_arg: 1 }
```

### Rule facts

`rule.facts()` reports, as JSON, what a compiled rule reads and calls:

```javascript
const rule = engine.compile('{"+": [{"var": "a.b"}, {"var": "c"}]}');
JSON.parse(rule.facts());
// { reads: [["a", "b"], ["c"]], computed_reads: false, reads_complete: true,
//   reads_data: true, operators: ["+", "val"], custom_operators: [],
//   deterministic: true }
```

`reads` lists each data path read from the root as its segments.
`computed_reads` flags a path known only at runtime, and
`reads_complete` is `true` when `reads` is everything the rule can read
(no computed path, no custom operator). `operators` uses canonical names
(`val` for `var`), matching `engine.operators()`. `deterministic` is
`false` for `now` and any custom operator. The facts describe the rule
after the optimizer, so a folded-away branch is neither read nor listed.

### Truthiness

`engine.truthy(value)` applies the engine's configured truthiness to a
JSON value, the same rules `if`, `and`, `or` and `evaluateTruthy` use.
Under the default rules an empty object is falsy, like an empty array:

```javascript
engine.truthy('{}');   // false
engine.truthy('[0]');  // true
engine.truthy('"0"');  // true (a non-empty string)
```

## Engine configuration

`new Engine({ config })` accepts an optional evaluation config, either
as a JSON string or as a plain JS object. It maps 1:1 to the Rust
engine's `EvaluationConfig::from_json_str`. All keys are optional;
unknown keys or values throw a `ConfigurationError`:

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | `bool` |
| `missing_var` | `"null"` (default: a missing variable reads as `null`), `"error"` (raises `VariableNotFound`) |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `null` for unbounded (caps the work one evaluation may do; crossing it raises `BudgetExceeded`) |

`preset` applies first; the remaining keys override it individually.

```javascript
// Strict semantics: no null-to-zero coercion. The default engine
// evaluates {"+": [null, 1]} to "1"; the strict one throws instead.
const engine = new Engine({ config: { preset: 'strict' } });
engine.evalStr('{"+": [null, 1]}', '{}');
// throws Error { name: "Thrown", thrown: { type: "NaN" }, operator: "+", ... }

// The same config as a JSON string.
const strict = new Engine({ config: '{"preset": "strict"}' });
```

With `missing_var: "error"`, a `var` / `val` read that finds nothing
raises `VariableNotFound` naming the path, so a typo fails instead of
flowing on as `null`. A default (`{"var": ["x", 0]}`), a present `null`,
`missing`, `missing_some` and `exists` are not misses, and `try` catches
the error:

```javascript
const checked = new Engine({ config: { missing_var: 'error' } });
checked.evalStr('{"var": "user.nmae"}', '{"user": {"name": "Ana"}}');
// throws Error { name: "VariableNotFound", variable: "user.nmae", operator: "var", ... }
checked.evalStr('{"var": ["user.nmae", "anonymous"]}', '{"user": {"name": "Ana"}}'); // '"anonymous"'
```

### Operator families

`families` keeps an engine to the JSONLogic core plus the operator
families you name (the `family` of each `engine.operators()` row:
`DateTime`, `ExtString`, `ExtArray`, `ExtObject`, `ExtControl`,
`ErrorHandling`, `ExtMath`, `Tensor`, `Flagd`). Unset, the engine has
every family. A family left out is not there for that engine: its names
compile as unknown operators, count as output fields in templating
mode, and are free for a custom operator. An unknown family name throws
a `ConfigurationError`:

```javascript
const engine = new Engine({ families: ['ExtString'] });
engine.evalStr('{"upper": ["a"]}', '{}'); // '"A"'
engine.evalStr('{"now": []}', '{}');      // throws InvalidOperator (DateTime is left out)
```

### Metering: what a rule costs

`evalMetered` returns a JSON envelope `{"result": ..., "ops": N}` instead
of a bare result, so you can see what an evaluation charged whether or not
a budget is set. `Rule` and `Session` have the same method as
`evaluateMetered`.

```javascript
const engine = new Engine();
const logic = '{"map":[{"var":"xs"},{"*":[{"var":""},2]}]}';
JSON.parse(engine.evalMetered(logic, '{"xs":[1,2,3]}'));
// { result: [2, 4, 6], ops: 10 }

// An optional third argument caps the operations for that one call,
// overriding the engine's `config.ops_budget`.
engine.evalMetered(logic, '{"xs":[1,2,3]}', 100_000);
```

One operation is one node the engine dispatches, one item an iterator
walks, or whatever an operator charges for the data it moves (the tensor
family prices itself in elements). Literals and constant-folded subtrees
cost nothing. A budget must be a whole number >= 1. Exceeding it throws
an `Error` named `BudgetExceeded` carrying `budget` and `spent`. The
engine refuses the evaluation before doing the work, and a `try` in the
rule cannot recover from it.

## Error handling

Every API throws a real `Error` object (`e instanceof Error` is `true`)
with the structured fields attached as own properties:

| Property | Contents |
|----------|----------|
| `name` | Stable error-kind tag: `"ParseError"`, `"InvalidOperator"`, `"InvalidArguments"`, `"VariableNotFound"`, `"TypeError"`, `"ArithmeticError"`, `"Thrown"`, `"IndexOutOfBounds"`, `"ConfigurationError"`, `"BudgetExceeded"`, `"Custom"`, ... plus this binding's `"TypeMismatch"` (typed evaluations whose result has the wrong type) and `"CompileError"` (`compileChecked`) |
| `message` | Human-readable message, including `(in operator: ...)` when the failing operator is known |
| `type` | Same tag as `name` (mirrors the wire JSON) |
| `operator` | Innermost failing operator, custom operators included (runtime errors only) |
| `node_ids` | Breadcrumb of compiled-node ids from the failure site toward the root (runtime errors only) |
| variant extras | Kind-specific fields: `thrown` (Thrown, as a parsed JS value), `variable` (VariableNotFound), `index` / `length` (IndexOutOfBounds), `budget` / `spent` (BudgetExceeded), `diagnostics` (CompileError), `stage` (boundary input errors, e.g. `"parse-data"`) |
| `detailJson` | The structured error as a JSON string (what 5.0.0 threw as a bare string) |

```javascript
try {
  engine.evalStr('{"throw": "limit_exceeded"}', '{}');
} catch (e) {
  e instanceof Error; // true
  e.name;             // "Thrown"
  e.message;          // 'Thrown: {"type":"limit_exceeded"} (in operator: throw)'
  e.thrown;           // { type: "limit_exceeded" }  (a real JS object)
  e.operator;         // "throw"
}
```

The broad categories:

- **Parse errors** (`e.name === "ParseError"`): malformed JSON in the
  rule, the data or an option, or an invalid budget or mode, raised
  before the engine runs.
- **Compile errors** (`"CompileError"`): `compileChecked` found at least
  one error diagnostic; `e.diagnostics` lists them all.
- **Runtime errors** (everything else): unknown operator names
  (`"InvalidOperator"`; `compile` does not reject them), arithmetic on
  non-numbers (`"Thrown"` with `thrown: { type: "NaN" }`, e.g.
  `{"+": ["abc", 1]}`), explicit `throw` operators, invalid operator
  arguments (`"InvalidArguments"`). They carry the innermost failing
  `operator` and the `node_ids` path through the compiled tree.

A missing variable reads as `null` unless the engine's config sets
`missing_var: "error"` (see [Engine configuration](#engine-configuration)).

**Panics are not errors.** The release build compiles with
`panic = "abort"`, so a bug that makes the engine panic cannot be turned
into one of the errors above. It surfaces as a
`WebAssembly.RuntimeError: unreachable` (the panic message goes to
`console.error`), and that WASM instance is unusable afterwards: reload
the module before evaluating again. Please report any such panic as a bug.

## Threading & Web Workers

The WASM module is **isolated per Web Worker**: each Worker loads its
own copy of the module, so an `Engine` or `Rule` created in one Worker
cannot be transferred to another. Within a single Worker, evaluation
is synchronous and single-threaded; share an engine and its rules
across calls in the same context, not across Workers.

For parallelism, spawn N Workers and build the engine and compile the
rule once per Worker. The compile cost is small relative to the
isolation benefit.

## Tracing

`engine.evaluateWithTrace(logic, data, mode?)` evaluates with a
step-by-step execution trace, honouring the engine's templating flag,
config and custom operators. It returns a JSON string; the React
debugger
([`@goplasmatic/datalogic-ui`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/ui/README.md))
reads this envelope as is:

```javascript
const engine = new Engine();
const trace = engine.evaluateWithTrace('{"and": [true, {"var": "x"}]}', '{"x": true}');
JSON.parse(trace);
// {
//   "result": true,
//   "expression_tree": { "id": 4, "expression": "{\"and\": [true, {\"var\": \"x\"}]}",
//                        "children": [ { "id": 3, "expression": "{\"var\": \"x\"}", "children": [] } ] },
//   "steps": [ /* 2 steps: the var lookup (node 3), then and (node 4) */ ],
//   "pointers": { "1": "/and/0", "2": "/and/1/var", "3": "/and/1", "4": "" }
// }
```

Node ids are assigned at compile time, children first, so the root is
the highest id (`id: 0` with an empty `expression` is only the
placeholder returned when compilation fails). Literal operands (the
`true` above) never record a step. `pointers` maps each node id to the
RFC 6901 JSON Pointer of the rule value it was compiled from, so a
debugger can place every step in the rule as written; it is absent when
the rule does not compile. The result keeps its object key order.

`mode` is `"engine"` (the default), `"strict"` or `"template"`, as for
`check`, so a rule you compile with `compileTemplate` can be traced as
one. Runtime failures do not throw: `result` is `null`, `error` carries
the message, and `structured_error` the structured form
(`{ type, message, operator?, node_ids?, ... }`):

```javascript
const strict = new Engine({ config: { preset: 'strict' } });
const run = JSON.parse(strict.evaluateWithTrace('{"+": [null, 1]}', '{}'));
run.result;                 // null
run.structured_error.type;  // "Thrown"  (strict mode rejects the null operand)
run.steps[0].error;         // 'Thrown: {"type":"NaN"} (in operator: +)'
```

## Supported operators

This binding exposes all 84 built-in operators from the Rust engine:

**Logical**: `and`, `or`, `!`, `!!`
**Comparison**: `==`, `===`, `!=`, `!==`, `<`, `<=`, `>`, `>=`
**Arithmetic**: `+`, `-`, `*`, `/`, `%`, `min`, `max`, `abs`, `ceil`, `floor`
**Control flow**: `if`, `?:`, `??` (coalesce), `switch` / `match`
**Array**: `map`, `filter`, `reduce`, `all`, `some`, `none`, `merge`, `in`, `sort`, `slice`, `group_by`, `distinct`
**Object**: `keys`, `values`, `entries`
**String**: `cat`, `substr`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`, `length`
**Data access**: `var`, `val`, `exists`, `missing`, `missing_some`
**Date/time**: `now`, `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`
**Error handling**: `try`, `throw`
**Type**: `type`
**Feature flags (flagd)**: `fractional`, `sem_ver`
**Tensor**: `tensor`, `zeros`, `full`, `scatter`, `rle_expand`, `one_hot`, `stack`, `concat`, `unstack`, `reshape`, `transpose`, `pad`, `crop`, `gather`, `cast`, `normalize`, `argmax`, `to_list`, `shape`, `dtype`

An engine built with `families` evaluates the core plus the families
it names (see [Operator families](#operator-families)).

> **Tensors on the wire:** a tensor crosses the JSON boundary as the
> tagged `{"tensor": {"dtype", "shape", "data"}}` form, with `data`
> little-endian base64, and the engine accepts that same form back as a rule,
> so a result pasted into a new rule evaluates to the tensor it came
> from. The family is marshalling-only (no arithmetic): it moves JSON
> into a model's inputs and its outputs back into JSON.

> **Templating mode:** v5 removed the `preserve` *operator*. For JSON
> templates with embedded JSONLogic (multi-key objects become
> output-shaping templates), pass `templating: true` to `new Engine`, or
> compile one rule with `engine.compileTemplate(logic)`.

For the full operator reference and semantics, see the
[documentation site](https://goplasmatic.github.io/datalogic-rs/).

## Performance

<!-- canonical-bench v5.1 -->
Geomean across 51 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **10.3 ns/op**, 7.0× faster than json-logic-engine (compiled, the fastest JS engine), 28.1× faster than jsonlogic-rs (the closest Rust alternative), and 83.6× faster than the json-logic-js reference implementation. The WASM build under Node measures 900.5 ns geomean (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

WASM-specific notes:

- **Compiled rules** are faster for repeated evaluations
- **The wasm-bindgen boundary copies strings in both directions**
  (encode in, decode out), so per-call overhead scales with payload
  size; budget for that on large data. **`DataHandle` removes the
  input half of that cost** when the same payload is evaluated more
  than once: on the boundary harness the hot session loop over an 8 KB
  payload measures ~30.6 µs/op via strings vs ~3.97 µs/op via a handle
  (7.7×), ~1 KB payloads gain 4.4×, tiny ones ~1.6×. Parsing the
  handle costs about one string-path evaluation, so it pays for itself
  from the second evaluation onward. For one-off payloads, stay on
  the string path.
- **Self-contained module**: 5,197,441 bytes (about 5.2 MB)
  uncompressed and 1,002,235 bytes (about 1.0 MB) gzipped in the 5.8.0
  release build. It compiles in the IANA timezone database behind the
  `datetime` feature's timezone arguments; see
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
building to keep only those:

```bash
CHRONO_TZ_TIMEZONE_FILTER='(UTC|Europe/.*|America/New_York)' ./build.sh
```

The published package is built without a filter so every zone resolves.

### Build profiles

The published package (and a plain `./build.sh`) uses the
size-optimized **release** profile: `opt-level = "z"` plus
`wasm-opt -Oz`. If module size matters less to you than ns/op (e.g. a
server-side WASM deployment that fetches the artifact once), an
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

This profile does not change the default build; the speed variant only
exists if you build it yourself.

### Tests

```bash
wasm-pack test --node
```

CI runs this command. The suite uses no DOM APIs and is node-configured
on purpose: adding `wasm_bindgen_test_configure!(run_in_browser)` back
would make the node runner skip every test.

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [Documentation: JavaScript](https://goplasmatic.github.io/datalogic-rs/javascript/installation.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
