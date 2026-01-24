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

> **Note:** The `@goplasmatic/datalogic` WASM package is bundled internally for evaluation.

## CSS Setup

Import the required styles in your application entry point or component:

```tsx
// React Flow base styles (required)
import '@xyflow/react/dist/style.css';

// DataLogicEditor styles (required)
import '@goplasmatic/datalogic-ui/styles.css';
```

### Style Import Order

Import order matters. Always import React Flow styles before DataLogicEditor styles:

```tsx
// Correct order
import '@xyflow/react/dist/style.css';
import '@goplasmatic/datalogic-ui/styles.css';

// Then import components
import { DataLogicEditor } from '@goplasmatic/datalogic-ui';
```

## Minimal Example

```tsx
import '@xyflow/react/dist/style.css';
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
  DataLogicEditorMode,
  JsonLogicValue,
} from '@goplasmatic/datalogic-ui';
```

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

import '@xyflow/react/dist/style.css';
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
