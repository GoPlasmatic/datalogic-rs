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

`Rule` is safe to share across worker threads: share one instance and evaluate concurrently.

## Sessions: hot-loop arena reuse

A `Session` reuses one bump arena across evaluations and resets between calls to bound peak memory. Open one per worker thread:

```js
const sess = engine.session();
for (const payload of inputs) {
  sess.evaluate(rule, payload);
}
```

Sessions hold non-`Sync` state and must not be shared between worker threads.

## Error handling

Failures throw plain JS `Error` instances with structured fields attached:

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
| `apply(rule, data)` | One-shot compile + evaluate |
| `new Engine(config?)` | Engine with optional configuration and custom operators |
| `engine.compile(rule)` | Compile to a shareable `Rule` |
| `rule.evaluate(data)` | Evaluate against one payload |
| `engine.session()` | Arena-reusing session for hot loops |

Engine configuration, custom operators, and tracing follow the same shapes as every other binding; the [package README](https://www.npmjs.com/package/@goplasmatic/datalogic-node) documents the full surface, and [Configuration](../advanced/configuration.md) covers what each option means.

## When to choose WASM instead

Choose [`@goplasmatic/datalogic-wasm`](../javascript/installation.md) when the code must run in a browser or edge runtime, or when you want one artifact across Node + browser. Choose this native package for Node services where throughput matters: the WASM build measures roughly 88× slower than the native core on the same benchmark workload.

## Next steps

- [Framework integration patterns](../javascript/frameworks.md) — the React/Vue/worker recipes apply to both JS packages
- [Use cases & examples](../use-cases/examples.md)
- [Thread safety](../advanced/threading.md)
