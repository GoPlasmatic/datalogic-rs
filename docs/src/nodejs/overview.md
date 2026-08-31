# Node.js (Native Binding)

`@goplasmatic/datalogic-node` is the **native** Node.js binding: the Rust core compiled per platform and loaded through [napi-rs](https://napi.rs/), with no WebAssembly in between. On Node servers it is the fast path, running close to native Rust throughput.

> **Two npm packages, one engine.** This package is for Node services that want maximum throughput. [`@goplasmatic/datalogic-wasm`](../javascript/installation.md) is the WebAssembly build: it also runs under Node, but its home turf is browsers, edge runtimes, Deno, and Bun. Same core, same semantics, same conformance battery either way.

## Install

```bash
npm install @goplasmatic/datalogic-node
```

Prebuilt platform binaries are published as `optionalDependencies`, so npm pulls only the `.node` file matching your platform:

| Platform | Architectures |
|---|---|
| Linux (glibc) | x64, arm64 |
| Linux (musl)  | x64, arm64 |
| macOS         | x64, arm64 |
| Windows       | x64, arm64 |

Node 18 and newer are supported. There is no build step and no WASM initialization: import and call.

## Quick start

Rules and data are plain JavaScript objects; results come back as JavaScript values:

```js
import { apply } from '@goplasmatic/datalogic-node';

const result = apply(
  { if: [{ '>': [{ var: 'score' }, 50] }, 'pass', 'fail'] },
  { score: 75 }
);
// -> "pass"
```

Both arguments also accept JSON text: a JS string passed as `rule` or `data` is parsed as JSON, not treated as a string value. To evaluate against a data document that *is* a JSON string, pass it encoded (`rule.evaluate(JSON.stringify('hello'))`), or hand the JSON text to the string-in methods (`evaluateStr`) or a `DataHandle`.

## Compile once, evaluate many

For repeated evaluations of the same rule, compile once and hold the `Rule` instance:

```js
import { Engine } from '@goplasmatic/datalogic-node';

const engine = new Engine();
const rule = engine.compile({ '+': [{ var: 'x' }, 1] });

for (const payload of inputs) {
  console.log(rule.evaluate(payload));
}
```

`Rule` has no thread affinity on the Rust side, but napi class instances cannot be posted or transferred between `worker_threads`: a `Rule` sent through `postMessage` arrives as an empty plain object with no `evaluate`. Each worker must load the module and compile its own `Rule` (compiling is cheap). For parallelism from a single thread, `rule.evaluateStrAsync(json)` evaluates on the libuv pool (see [Async evaluation](#async-evaluation)).

The constructor takes an options bag and, separately, a map of custom operators:

```js
const strict = new Engine({ config: { preset: 'strict' } });
const templated = new Engine({ templating: true });
const custom = new Engine({}, {
  double: (argsJson) => String(JSON.parse(argsJson)[0] * 2),
});
custom.compile({ double: [21] }).evaluate({}); // 42
```

Note the nesting: `preset` and the other evaluation options go under `config`, not at the top level. The options bag only reads `templating`, `templateKeyEscape` and `config`; other top-level keys are ignored, so `new Engine({ preset: 'strict' })` silently builds a default engine.

`templateKeyEscape` is a single-character prefix, unset by default, that lets a template emit a key which would otherwise be swallowed as an operator: with `new Engine({ templating: true, templateKeyEscape: '$' })`, `{ $type: { var: 'x' } }` yields `{ type: 1 }` rather than running the `type` operator, and `{ $$type: 1 }` yields `{ $type: 1 }`. Anything other than a one-character string throws `errorType: 'InvalidArguments'`. See [Structured Objects](../advanced/structured-objects.md#emitting-keys-that-are-operator-names). Unknown keys *inside* `config` throw `errorType: 'ConfigurationError'`. [Configuration](../advanced/configuration.md) covers what each option means.

## Sessions: hot-loop arena reuse

A `Session` reuses one bump arena across evaluations and resets between calls to bound peak memory. Open one per worker thread:

```js
const sess = engine.session();
for (const payload of inputs) {
  sess.evaluate(rule, payload);
}
```

Sessions hold non-`Sync` state and must not be shared between worker threads.

## Data handles and typed results

A `DataHandle` is an immutable, pre-parsed JSON document. Parse a payload once and every evaluation against it skips JSON parsing entirely; the typed session methods also skip the result round trip:

```js
import { DataHandle } from '@goplasmatic/datalogic-node';

const handle = new DataHandle('{"age": 25, "status": "active"}'); // throws ParseError on bad JSON
const isAdult = engine.compile({ '>=': [{ var: 'age' }, 18] });
const nextAge = engine.compile({ '+': [{ var: 'age' }, 1] });

nextAge.evaluateData(handle);         // 26   (JS value out, no parse per call)
nextAge.evaluateDataStr(handle);      // "26" (JSON string out)
sess.evaluateData(isAdult, handle);   // true (hot path: session arena + no parse)
sess.evaluateBool(isAdult, handle);   // true (strict JSON boolean, else TypeMismatch)
sess.evaluateNumber(nextAge, handle); // 26   (any JSON number, else TypeMismatch)
sess.evaluateTruthy(isAdult, handle); // true (JSONLogic truthiness, never mismatches)
```

`evaluateBool` and `evaluateNumber` throw an `EvaluateError` with `errorType: 'TypeMismatch'` when the rule evaluated fine but the result has another type. Handles are per-thread like sessions: parse one per worker.

## Batch evaluation

The batch entry points evaluate a whole set in one native call and report outcomes per item in the `Promise.allSettled` shape, so one bad input never poisons its neighbours:

```js
const outcomes = sess.evaluateBatch(rule, [h0, h1, h2]); // one rule, many payloads
const flags = sess.evaluateMany([r0, r1], handle);        // many rules, one payload

for (const [i, o] of flags.entries()) {
  if (o.status === 'rejected') {
    console.log(`rule ${i} failed: ${o.reason.message} (${o.reason.tag})`);
  } else {
    console.log(`rule ${i}: ${o.value}`); // result as a JSON string
  }
}
```

## Async evaluation

`rule.evaluateStrAsync(dataJson)` evaluates on the libuv thread pool and returns a `Promise<string>`. It is not faster per call than `evaluateStr`; the win is keeping large payloads' parse + evaluate + serialize off the event loop:

```js
const result = await rule.evaluateStrAsync('{"age": 25}');
```

String input only (a `DataHandle` is pinned to the JS thread). Rejections carry the same structured fields as synchronous throws.

## Tracing

`engine.evaluateWithTrace(logic, data)` returns a JSON string with the same `{ result, expression_tree, steps, error?, structured_error? }` envelope the WASM package produces, so the [React debugger](../react-ui/installation.md) accepts output from either package:

```js
const run = JSON.parse(engine.evaluateWithTrace('{"+": [1, 2, 3]}', 'null'));
run.result;       // 6
run.steps.length; // 1 (one step per operator node; literals record no step)
```

Failures do not throw: `result` is `null`, `error` carries the message, and `structured_error` the structured form. The rule is compiled with optimization disabled so every operator surfaces a step; use it for debugging, not hot paths.

## Operator names

Tooling that validates or autocompletes rules can ask the binding for its vocabulary instead of keeping a hand-maintained list:

```js
import { builtinOperatorNames, Engine } from '@goplasmatic/datalogic-node';

const names = builtinOperatorNames();
names.length;               // 67: the 64 built-in operators plus the aliases var, ?:, match
names.includes('group_by'); // true

const engine = new Engine({}, { double: (a) => String(JSON.parse(a)[0] * 2) });
engine.customOperatorNames(); // ['double']
```

`builtinOperatorNames()` is derived from the compiler's own lookup table, so it cannot drift from dispatch; `engine.customOperatorNames()` lists the operators registered on that engine. The union is the engine's full vocabulary.

## Error handling

Failures throw plain JS `Error` instances with structured fields attached:

```js
try {
  rule.evaluate(data);
} catch (e) {
  if (e.name === 'ParseError') {
    // Malformed rule or data JSON
  } else if (e.name === 'EvaluateError') {
    console.log(e.errorType);  // stable tag (e.g. "InvalidOperator", "Thrown")
    console.log(e.operator);   // outermost failing operator
    console.log(e.nodeIds);    // leaf-to-root breadcrumb
    console.log(e.path);       // resolved root-to-leaf step list
  }
}
```

`compile` rejects malformed JSON and structurally invalid rules (a multi-key object when templating is off), but it does not verify operator names: an unknown operator compiles and surfaces as `errorType: 'InvalidOperator'` when the rule is evaluated. Missing data is not an error: `{ var: 'missing' }` evaluates to `null`.

## API surface

| Symbol | Description |
|---|---|
| `apply(rule, data)` | One-shot compile + evaluate |
| `builtinOperatorNames()` | Every built-in operator name this build accepts (includes aliases) |
| `new Engine({ templating?, templateKeyEscape?, config? }, customOperators?)` | Engine with optional templating, template-key escape, evaluation config, and custom operators |
| `engine.compile(rule)` | Compile to a reusable `Rule` |
| `engine.eval(rule, data)` / `engine.evalStr(rule, data)` | One-shot, JS value / JSON string out |
| `engine.evaluateWithTrace(logic, data)` | One-shot with execution trace (JSON strings in and out) |
| `engine.session()` | Arena-reusing `Session` for hot loops |
| `engine.customOperatorNames()` | Names of the custom operators registered on this engine |
| `new DataHandle(json)` / `handle.allocatedBytes` | Parse a payload once; arena bytes held by the handle |
| `rule.evaluate(data)` / `rule.evaluateStr(data)` | Evaluate against one payload, JS value / JSON string out |
| `rule.evaluateData(handle)` / `rule.evaluateDataStr(handle)` | Evaluate a pre-parsed handle |
| `rule.evaluateStrAsync(dataJson)` | Evaluate on the libuv pool, returns `Promise<string>` |
| `sess.evaluate(rule, data)` / `sess.evaluateStr(rule, data)` | Evaluate with arena reuse |
| `sess.evaluateData(rule, handle)` / `sess.evaluateDataStr(rule, handle)` | Handle in, arena reuse; the `Str` form is the fastest path |
| `sess.evaluateBool` / `evaluateNumber` / `evaluateTruthy` `(rule, handle)` | Typed results (`TypeMismatch` for the strict two) |
| `sess.evaluateBatch(rule, handles)` / `sess.evaluateMany(rules, handle)` | Batch evaluation, allSettled-style items |
| `sess.reset()` / `sess.allocatedBytes()` | Explicit arena reset (optional); bytes currently held by the arena's chunks |

The [package README](https://www.npmjs.com/package/@goplasmatic/datalogic-node) carries the same surface with the configuration key table; [Configuration](../advanced/configuration.md) covers what each option means.

## When to choose WASM instead

Choose [`@goplasmatic/datalogic-wasm`](../javascript/installation.md) when the code must run in a browser or edge runtime, or when you want one artifact across Node + browser. Choose this native package for Node services where throughput matters: the WASM build measures roughly 88× slower than the native core on the same benchmark workload.

## Next steps

- [Integration: Express](../integrations/express.md) for the compile-once, cache-by-version service pattern
- [Framework integration patterns](../javascript/frameworks.md) for the React/Vue/worker recipes (they apply to both JS packages)
- [Use cases & examples](../use-cases/examples.md)
- [Thread safety](../advanced/threading.md)
