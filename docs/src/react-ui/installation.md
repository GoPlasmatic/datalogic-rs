# Installation

The `@goplasmatic/datalogic-ui` package provides a React component for visualizing and debugging JSONLogic expressions as interactive flow diagrams.

## Package Installation

```bash
# npm
npm install @goplasmatic/datalogic-ui @xyflow/react

# yarn
yarn add @goplasmatic/datalogic-ui @xyflow/react

# pnpm
pnpm add @goplasmatic/datalogic-ui @xyflow/react
```

## Peer Dependencies

The package requires:

| Package | Version | Purpose |
|---------|---------|---------|
| `react` | 18+ or 19+ | React framework |
| `react-dom` | 18+ or 19+ | React DOM renderer |
| `@xyflow/react` | 12+ | Flow diagram rendering |

> **Note:** The `@goplasmatic/datalogic-wasm` WASM package is bundled internally for evaluation.

## CSS Setup

One import, in your application entry point or component:

```tsx
import '@goplasmatic/datalogic-ui/styles.css';
```

React Flow's base styles are vendored into the package's `styles.css`, so
there is no separate `@xyflow/react/dist/style.css` import and no import-order
requirement. `@xyflow/react` itself stays a peer dependency because the
component's JavaScript uses it; only its stylesheet is bundled.

## Minimal Example

```tsx
import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

function App() {
  return (
    <div style={{ width: '100%', height: '500px' }}>
      <DataLogicEditor
        value={{ "==": [{ "var": "x" }, 1] }}
      />
    </div>
  );
}
```

## Container Requirements

The editor requires a container with defined dimensions:

```tsx
// Option 1: Explicit dimensions
<div style={{ width: '100%', height: '500px' }}>
  <DataLogicEditor value={expression} />
</div>

// Option 2: CSS class
<div className="editor-container">
  <DataLogicEditor value={expression} />
</div>

// CSS
.editor-container {
  width: 100%;
  height: 100vh;
}
```

## TypeScript Setup

Types are included in the package. Import types as needed:

```tsx
import type {
  DataLogicEditorProps,
  DataLogicEvaluationConfig,
  DataLogicCustomOperator,
  JsonLogicValue,
} from '@goplasmatic/datalogic-ui';
```

A JSONLogic literal whose array holds two different operator keys needs the
annotation, or TypeScript widens it into a union `JsonLogicValue` does not
accept:

```tsx
const expression: JsonLogicValue = {
  and: [
    { '>': [{ var: 'age' }, 18] },
    { '==': [{ var: 'status' }, 'active'] },
  ],
};
```

See [Props & API](props-api.md#types) for the full export list.

## Bundler Notes

### Vite

Works out of the box. No additional configuration needed.

### Webpack

Ensure CSS loaders are configured:

```javascript
module.exports = {
  module: {
    rules: [
      {
        test: /\.css$/,
        use: ['style-loader', 'css-loader'],
      },
    ],
  },
};
```

### Next.js

For App Router, use client components:

```tsx
'use client';

import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

export function LogicVisualizer({ expression }) {
  return <DataLogicEditor value={expression} />;
}
```

## Next Steps

- [Quick Start](quick-start.md) - Basic usage examples
- [Modes](modes.md) - Visualize, debug, and edit modes
- [Props & API](props-api.md) - Complete props reference
