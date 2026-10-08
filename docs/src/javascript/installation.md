# Installation

> **Two npm packages, one engine.** This chapter covers `@goplasmatic/datalogic-wasm`, the WASM build: pick it for browsers, edge runtimes, Deno, and anywhere portability matters. For Node.js servers, prefer the native [`@goplasmatic/datalogic-node`](../nodejs/overview.md) package (napi), which calls the Rust core without a WebAssembly layer.

The `@goplasmatic/datalogic-wasm` package provides WebAssembly bindings for the datalogic-rs engine, so you can evaluate JSONLogic from JavaScript and TypeScript.

## Package Installation

```bash
# npm
npm install @goplasmatic/datalogic-wasm

# yarn
yarn add @goplasmatic/datalogic-wasm

# pnpm
pnpm add @goplasmatic/datalogic-wasm
```

The package declares Node 18 or newer (`engines.node`) for the Node.js target.

## Build Targets

The package includes three build targets, one per environment:

| Target | Use Case | Init Required |
|--------|----------|---------------|
| `web` | Browser ES Modules, CDN | Yes (`await init()`) |
| `bundler` | Webpack, Vite, Rollup | No (instantiates on import; needs the bundler's WASM ESM integration) |
| `nodejs` | Node.js (CommonJS/ESM) | No |

### Automatic Target Selection

The package's `exports` field selects a target by export condition:

```javascript
// Browser/Bundler - the `import` condition resolves to the web target
import init, { Engine } from '@goplasmatic/datalogic-wasm';

// Node.js - the `node` condition resolves to the nodejs target
const { Engine } = require('@goplasmatic/datalogic-wasm');
```

### Explicit Target Import

If you need a specific target:

```javascript
// Web target (ES modules with init)
import init, { Engine } from '@goplasmatic/datalogic-wasm/web';

// Bundler target (no init; the module instantiates on import)
import { Engine } from '@goplasmatic/datalogic-wasm/bundler';

// Node.js target
import { Engine } from '@goplasmatic/datalogic-wasm/nodejs';
```

The bundler target imports `datalogic_wasm_bg.wasm` as an ES module, so it needs Webpack's `experiments.asyncWebAssembly` (or the equivalent in your bundler); see [Bundler Configuration](frameworks.md#bundler-configuration).

## WASM Initialization

For browser and bundler environments, initialize the WASM module before you use any export:

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';

// Initialize once at application startup
await init();

// Then build an engine and evaluate
const engine = new Engine();
const result = engine.evalStr('{"==": [1, 1]}', '{}'); // "true"
```

> On Node.js, use the exports right after import, with no initialization. Do not call `init()` there: with the bare specifier, Node resolves to the CommonJS `nodejs` target, so a default import binds `init` to the module namespace object and `await init()` throws `TypeError: init is not a function`. Code that has to run in both places should guard the call:
>
> ```javascript
> import init, { Engine } from '@goplasmatic/datalogic-wasm';
> if (typeof init === 'function') await init(); // browser: loads the module; Node: no-op
> ```
>
> Or import from `@goplasmatic/datalogic-wasm/nodejs` and skip `init`.

## TypeScript Support

The package includes TypeScript declarations. No additional `@types` package is needed.

```typescript
import init, { Engine, Rule } from '@goplasmatic/datalogic-wasm';

await init();
const engine = new Engine();
const rule: Rule = engine.compile('{"==": [1, 1]}');
const result: string = rule.evaluate('{}'); // "true"
```

The declarations mark the deprecated exports (`evaluate`, `evaluateWithTrace`, `CompiledRule`, `Session.evaluateNumber`) with `@deprecated`, so your editor flags them; see [Deprecated APIs](api-reference.md#deprecated-apis).

## Bundle Size

The WASM binary is a single self-contained module: 5,197,441 bytes (about 5.2 MB) uncompressed and 1,002,235 bytes (about 1.0 MB) gzipped in the 5.8.0 release build. It compiles in the IANA timezone database that the `datetime` feature's `format_date` / `parse_date` timezone arguments use. Size-sensitive embedders building from source can shrink it by setting `CHRONO_TZ_TIMEZONE_FILTER` to the zones they need; see [Building from source](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/wasm#building-from-source).

## CDN Usage

For prototypes or simple pages, you can load the module from a CDN:

```html
<script type="module">
  import init, { Engine } from 'https://unpkg.com/@goplasmatic/datalogic-wasm@latest/web/datalogic_wasm.js';

  async function run() {
    await init();
    const engine = new Engine();
    console.log(engine.evalStr('{"==": [1, 1]}', '{}')); // "true"
  }

  run();
</script>
```

## Next Steps

- [Quick Start](quick-start.md) - Basic usage examples
- [API Reference](api-reference.md) - Functions, classes, configuration, and error shapes
- [Framework Integration](frameworks.md) - React, Vue, and bundler setup
