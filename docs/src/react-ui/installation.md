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

| Package | Version range | Purpose |
|---------|---------------|---------|
| `react` | `^18.0.0 \|\| ^19.0.0` | React framework |
| `react-dom` | `^18.0.0 \|\| ^19.0.0` | React DOM renderer |
| `@xyflow/react` | `^12.0.0` | Flow diagram rendering |

The package has no runtime dependencies beyond these. Do not install
`@goplasmatic/datalogic-wasm` for the editor: the WASM engine, built from the
same datalogic-rs release (5.8.0), ships inside the package's `dist` with the
`.wasm` binary inlined, so your bundler needs no WASM loader. `@dagrejs/dagre`,
`lucide-react` and `uuid` are bundled as well.

## CSS Setup

One import, in your application entry point or component:

```tsx
import '@goplasmatic/datalogic-ui/styles.css';
```

React Flow's base styles are vendored into the package's `styles.css`, so
there is no separate `@xyflow/react/dist/style.css` import and no import-order
requirement. `@xyflow/react` itself stays a peer dependency because the
component's JavaScript uses it; only its stylesheet is bundled.

The bundled React Flow rules and the editor's own handle and edge overrides
apply only inside the editor's `.logic-editor` root, so they leave any other
React Flow canvas on the page alone. If your app renders its own React Flow
canvas, import `@xyflow/react/dist/style.css` for that canvas as usual.

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

The editor fills its parent (`width: 100%; height: 100%`), so the parent needs
a height:

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

The package ships its own types. Import them as needed:

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

The package publishes an ES module build (`import`, `dist/index.js`) and a
CommonJS build (`require`, `dist/index.cjs`). Both start the WASM engine, so
Jest, CommonJS server rendering and older bundlers that pick the `require`
entry get evaluation and the debugger. If the engine fails to load, an editor
that has `data` shows a banner above the diagram with the load error and keeps
the static diagram.

### Vite

Needs no additional configuration.

### Webpack

Configure CSS loaders:

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

- [Quick Start](quick-start.md): basic usage examples
- [Modes](modes.md): visualize, debug, and edit modes
- [Props & API](props-api.md): complete props reference
