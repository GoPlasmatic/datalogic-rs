# Installation

The `@goplasmatic/datalogic` package provides WebAssembly bindings for the datalogic-rs engine, bringing high-performance JSONLogic evaluation to JavaScript and TypeScript.

## Package Installation

```bash
# npm
npm install @goplasmatic/datalogic

# yarn
yarn add @goplasmatic/datalogic

# pnpm
pnpm add @goplasmatic/datalogic
```

## Build Targets

The package includes three build targets optimized for different environments:

| Target | Use Case | Init Required |
|--------|----------|---------------|
| `web` | Browser ES Modules, CDN | Yes |
| `bundler` | Webpack, Vite, Rollup | Yes |
| `nodejs` | Node.js (CommonJS/ESM) | No |

### Automatic Target Selection

The package's `exports` field automatically selects the appropriate target:

```javascript
// Browser/Bundler - uses web or bundler target
import init, { evaluate } from '@goplasmatic/datalogic';

// Node.js - uses nodejs target
const { evaluate } = require('@goplasmatic/datalogic');
```

### Explicit Target Import

If you need a specific target:

```javascript
// Web target (ES modules with init)
import init, { evaluate } from '@goplasmatic/datalogic/web';

// Bundler target
import init, { evaluate } from '@goplasmatic/datalogic/bundler';

// Node.js target
import { evaluate } from '@goplasmatic/datalogic/nodejs';
```

## WASM Initialization

For browser and bundler environments, you must initialize the WASM module before using any functions:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic';

// Initialize once at application startup
await init();

// Now you can use evaluate, CompiledRule, etc.
const result = evaluate('{"==": [1, 1]}', '{}', false);
```

> **Note:** Node.js does not require initialization - you can use functions immediately after import.

## TypeScript Support

The package includes TypeScript declarations. No additional `@types` package is needed.

```typescript
import init, { evaluate, CompiledRule, evaluate_with_trace } from '@goplasmatic/datalogic';

// Full type inference for all exports
const result: string = evaluate('{"==": [1, 1]}', '{}', false);
```

## Bundle Size

The WASM binary is approximately 50KB gzipped, making it suitable for web applications where performance is critical.

## CDN Usage

For quick prototyping or simple pages, you can load directly from a CDN:

```html
<script type="module">
  import init, { evaluate } from 'https://unpkg.com/@goplasmatic/datalogic@latest/web/datalogic_wasm.js';

  async function run() {
    await init();
    console.log(evaluate('{"==": [1, 1]}', '{}', false)); // "true"
  }

  run();
</script>
```

## Next Steps

- [Quick Start](quick-start.md) - Basic usage examples
- [API Reference](api-reference.md) - Complete API documentation
- [Framework Integration](frameworks.md) - React, Vue, and bundler setup
