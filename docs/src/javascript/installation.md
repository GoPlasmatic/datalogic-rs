# Installation

> **Two npm packages, one engine.** This chapter covers `@goplasmatic/datalogic-wasm`, the WASM build: pick it for browsers, edge runtimes, Deno, and anywhere portability matters. For Node.js servers, prefer the native [`@goplasmatic/datalogic-node`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/node) package (napi), which calls the Rust core directly and runs at native speed.

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

## Build Targets

The package includes three build targets, one per environment:

| Target | Use Case | Init Required |
|--------|----------|---------------|
| `web` | Browser ES Modules, CDN | Yes (`await init()`) |
| `bundler` | Webpack, Vite, Rollup | No (instantiates on import; needs the bundler's WASM ESM integration) |
| `nodejs` | Node.js (CommonJS/ESM) | No |

### Automatic Target Selection

The package's `exports` field automatically selects the appropriate target:

```javascript
// Browser/Bundler - the `import` condition resolves to the web target
import init, { evaluate } from '@goplasmatic/datalogic-wasm';

// Node.js - the `node` condition resolves to the nodejs target
const { evaluate } = require('@goplasmatic/datalogic-wasm');
```

### Explicit Target Import

If you need a specific target:

```javascript
// Web target (ES modules with init)
import init, { evaluate } from '@goplasmatic/datalogic-wasm/web';

// Bundler target (no init; the module instantiates on import)
import { evaluate } from '@goplasmatic/datalogic-wasm/bundler';

// Node.js target
import { evaluate } from '@goplasmatic/datalogic-wasm/nodejs';
```

The bundler target imports `datalogic_wasm_bg.wasm` as an ES module, so it needs Webpack's `experiments.asyncWebAssembly` (or the equivalent in your bundler); see [Bundler Configuration](frameworks.md#bundler-configuration).

## WASM Initialization

For browser and bundler environments, you must initialize the WASM module before using any functions:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic-wasm';

// Initialize once at application startup
await init();

// Now you can use evaluate, CompiledRule, etc.
const result = evaluate('{"==": [1, 1]}', '{}', false);
```

> **Note:** Node.js does not require initialization; you can use functions immediately after import. Do not call `init()` there: with the bare specifier, Node resolves to the CommonJS `nodejs` target, so a default import binds `init` to the module namespace object and `await init()` throws `TypeError: init is not a function`. Code that has to run in both places should guard the call:
>
> ```javascript
> import init, { evaluate } from '@goplasmatic/datalogic-wasm';
> if (typeof init === 'function') await init(); // browser: loads the module; Node: no-op
> ```
>
> Or import from `@goplasmatic/datalogic-wasm/nodejs` explicitly and skip `init` altogether.

## TypeScript Support

The package includes TypeScript declarations. No additional `@types` package is needed.

```typescript
import init, { evaluate, CompiledRule, evaluateWithTrace } from '@goplasmatic/datalogic-wasm';

// Full type inference for all exports
const result: string = evaluate('{"==": [1, 1]}', '{}', false);
```

## Bundle Size

The WASM binary is a single self-contained module: approximately 2.84 MB uncompressed, around 600 KB gzipped (measured on the 5.3.0 release build). Most of the growth since 5.1 is the compiled-in IANA timezone database that the `datetime` feature's `format_date` / `parse_date` timezone arguments use (5.2.0). Size-sensitive embedders building from source can shrink it by setting `CHRONO_TZ_TIMEZONE_FILTER` to the zones they need; see [Building from source](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/wasm#building-from-source).

## CDN Usage

For prototypes or simple pages, you can load the module from a CDN:

```html
<script type="module">
  import init, { evaluate } from 'https://unpkg.com/@goplasmatic/datalogic-wasm@latest/web/datalogic_wasm.js';

  async function run() {
    await init();
    console.log(evaluate('{"==": [1, 1]}', '{}', false)); // "true"
  }

  run();
</script>
```

## Next Steps

- [Quick Start](quick-start.md) - Basic usage examples
- [API Reference](api-reference.md) - Functions, classes, configuration, and error shapes
- [Framework Integration](frameworks.md) - React, Vue, and bundler setup
