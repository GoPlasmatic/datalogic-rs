# Customization

This guide covers theming, styling, and advanced customization of the DataLogicEditor.

## Theming

### System Theme (Default)

By default, the editor follows the system theme preference:

```tsx
<DataLogicEditor value={expression} />
```

### Explicit Theme

Override with the `theme` prop:

```tsx
// Always dark
<DataLogicEditor value={expression} theme="dark" />

// Always light
<DataLogicEditor value={expression} theme="light" />
```

### Theme Resolution

The component sets `data-theme` on its own `.logic-editor` root element based on the `theme` prop (or system preference when the prop is omitted). It does **not** read `data-theme` from a parent or ancestor element, so wrapping the editor in `<div data-theme="dark">` has no effect. To force a theme, use the `theme` prop:

```tsx
<DataLogicEditor value={expression} theme="dark" />
```

### Dynamic Theme Switching

```tsx
function ThemedEditor() {
  const [theme, setTheme] = useState<'light' | 'dark'>('light');

  return (
    <div>
      <button onClick={() => setTheme(t => t === 'light' ? 'dark' : 'light')}>
        Toggle Theme
      </button>
      <DataLogicEditor value={expression} theme={theme} />
    </div>
  );
}
```

## CSS Customization

### Container Styling

Use the `className` prop for container styling:

```tsx
<DataLogicEditor value={expression} className="custom-editor" />
```

```css
.custom-editor {
  border: 2px solid #3b82f6;
  border-radius: 12px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
}
```

### CSS Variables

The component's theme variables are scoped to its `.logic-editor` root element (not `:root`), so they do not leak into the rest of your app. To override them, target the same scope. The dark theme applies through `.logic-editor[data-theme="dark"]`.

The primary axis is the **signal palette**: a node is coloured by the type of
value it produces, not by its operator category. Everything else sits on a
neutral substrate, and the accent colour is reserved for selection, root and
focus. These are the token names with their light-theme values:

```css
.logic-editor {
  /* Signal palette: colour = the value that flows out of a node.
     Each has a matching --sig-*-bg used for fills. */
  --sig-bool-true: #1a7f37;
  --sig-bool-false: #cf222e;
  --sig-bool-rest: #57708a;   /* boolean-valued, not yet evaluated */
  --sig-number: #0959c0;
  --sig-string: #8a5a00;
  --sig-collection: #8250df;  /* arrays and objects */
  --sig-data: #1b7c83;        /* var / val / exists: the data tap */
  --sig-temporal: #bf3989;    /* datetimes and durations */
  --sig-null: #6e7781;

  /* Substrate */
  --board: #eef1f5;           /* canvas */
  --board-grid: rgba(20, 40, 70, 0.05);
  --surface: #ffffff;         /* node bodies, panels */
  --surface-2: #f6f8fb;
  --chip: #ffffff;
  --hairline: #d6dde6;
  --hairline-2: #e6ebf1;

  /* Ink */
  --ink: #0e1826;
  --ink-2: #33475e;
  --muted: #5b6a7d;
  --faint: #9aa9ba;           /* non-text only: idle wires, dot grid */

  /* Structural accent: selection, root, focus ring */
  --accent: #4b56d6;
  --accent-soft: #e7e9fb;
  --accent-hover: #3a44c0;

  /* Type */
  --font-ui: 'Space Grotesk', ui-sans-serif, system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', ui-monospace, 'SF Mono', monospace;

  /* Shape, elevation, motion */
  --radius-sm: 7px;  --radius-md: 10px; --radius-lg: 14px;
  --shadow-sm: 0 1px 2px rgba(16, 30, 54, 0.05);
  --shadow-md: 0 1px 2px rgba(16, 30, 54, 0.06), 0 2px 6px rgba(16, 30, 54, 0.06);
  --shadow-lg: 0 8px 30px rgba(16, 30, 54, 0.14), 0 2px 8px rgba(16, 30, 54, 0.08);
  --motion-fast: 120ms; --motion-base: 180ms; --motion-slow: 260ms;
}
```

The dark theme redefines the same tokens under
`.logic-editor[data-theme="dark"]` (for example `--board: #0a0f16`,
`--surface: #10161f`, `--ink: #e6edf5`).

The stylesheet keeps older token names (`--bg-primary`, `--bg-secondary`,
`--text-primary`, `--border-primary`, `--accent-blue`, `--node-bg`,
`--syntax-*`, `--debug-*`, and the `--success-*` / `--error-*` / `--warning-*`
families) as aliases mapped onto the tokens above, so existing overrides keep
working.
Prefer the tokens above for new work.

### Fonts

The default stacks name Space Grotesk and JetBrains Mono, but the package does
not ship the font files. Either install them yourself:

```bash
npm install @fontsource/space-grotesk @fontsource/jetbrains-mono
```

```tsx
import '@fontsource/space-grotesk';
import '@fontsource/jetbrains-mono';
```

or point the two tokens at fonts you already load:

```css
.logic-editor {
  --font-ui: 'Inter', system-ui, sans-serif;
  --font-mono: 'Fira Code', ui-monospace, monospace;
}
```

Without either step the stacks fall back to the system UI and monospace fonts.

### Node Styling

Target specific node types:

```css
/* All nodes */
.react-flow__node {
  font-family: 'Inter', sans-serif;
}

/* Operator nodes (and, or, if, var, val, ==, +, etc.) */
.react-flow__node-operator {
  border-width: 2px;
}

/* Literal nodes (strings, numbers, booleans, null) */
.react-flow__node-literal {
  font-weight: bold;
}

/* Structure nodes (JSON objects/arrays in templating mode) */
.react-flow__node-structure {
  font-style: italic;
}
```

> **Note:** There are three node types: `operator`, `literal`, and `structure`. There is no `variable` node type: variables (`var` / `val`) render as operator nodes, so a `.react-flow__node-variable` selector matches nothing.

### Edge Styling

Customize connection lines:

```css
.react-flow__edge-path {
  stroke: #6b7280;
  stroke-width: 2px;
}

.react-flow__edge.selected .react-flow__edge-path {
  stroke: #3b82f6;
}
```

## Layout Customization

### Container Dimensions

The editor requires explicit dimensions:

```tsx
// Fixed height
<div style={{ height: '500px' }}>
  <DataLogicEditor value={expression} />
</div>

// Viewport height
<div style={{ height: '100vh' }}>
  <DataLogicEditor value={expression} />
</div>

// Flexbox
<div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
  <header>...</header>
  <div style={{ flex: 1 }}>
    <DataLogicEditor value={expression} />
  </div>
</div>
```

## Using Utilities

### Custom Flow Rendering

For complete control, use the utility functions with your own React Flow instance:

```tsx
import { ReactFlow, Background, Controls } from '@xyflow/react';
import { jsonLogicToNodes, applyTreeLayout, CATEGORY_COLORS } from '@goplasmatic/datalogic-ui';

function CustomEditor({ expression }) {
  const { nodes: rawNodes, edges } = jsonLogicToNodes(expression);
  const nodes = applyTreeLayout(rawNodes, edges);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      fitView
      nodesDraggable={false}
      nodesConnectable={false}
    >
      <Background />
      <Controls />
    </ReactFlow>
  );
}
```

### Custom Node Types

Create custom node components:

```tsx
import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import { CATEGORY_COLORS, type OperatorNodeData } from '@goplasmatic/datalogic-ui';

function CustomOperatorNode({ data }: NodeProps<Node<OperatorNodeData>>) {
  const color = CATEGORY_COLORS[data.category];

  return (
    <div
      style={{
        background: color,
        padding: '12px 20px',
        borderRadius: '8px',
        color: 'white',
      }}
    >
      <Handle type="target" position={Position.Top} />
      <div>{data.label}</div>
      <div style={{ fontSize: '0.75em', opacity: 0.8 }}>{data.operator}</div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

const customNodeTypes = {
  operator: CustomOperatorNode,
  // ... other custom types
};
```

Node data describes the expression, not its value: there is no `result` field
on any node shape. Evaluated values live in the trace steps
(`evaluateWithTrace`), so a custom renderer that wants to show results should
keep its own map keyed by node id and look values up from there.

### Category Colors

`CATEGORY_COLORS` is a palette for your own legends, pickers and custom node
renderers. The shipped nodes do not use it: they are coloured by the value
type they produce, through the `--sig-*` tokens above. Its keys are the
operator categories plus `literal`:

```tsx
import { CATEGORY_COLORS } from '@goplasmatic/datalogic-ui';

// Default colors
console.log(CATEGORY_COLORS);
// {
//   variable: '#6366f1',
//   comparison: '#14b8a6',
//   logical: '#8b5cf6',
//   arithmetic: '#22c55e',
//   string: '#06b6d4',
//   array: '#7c3aed',
//   object: '#a855f7',
//   control: '#f59e0b',
//   datetime: '#0ea5e9',
//   validation: '#94a3b8',
//   utility: '#64748b',
//   error: '#ef4444',
//   flagd: '#f97316',
//   literal: '#64748b'
// }

// Use in custom components
function Legend() {
  return (
    <div>
      {Object.entries(CATEGORY_COLORS).map(([category, color]) => (
        <div key={category} style={{ display: 'flex', alignItems: 'center' }}>
          <span style={{ background: color, width: 16, height: 16 }} />
          <span>{category}</span>
        </div>
      ))}
    </div>
  );
}
```

## Responsive Design

Make the editor responsive:

```tsx
function ResponsiveEditor({ expression }) {
  return (
    <div className="editor-wrapper">
      <DataLogicEditor value={expression} />
    </div>
  );
}
```

```css
.editor-wrapper {
  width: 100%;
  height: 300px;
}

@media (min-width: 768px) {
  .editor-wrapper {
    height: 500px;
  }
}

@media (min-width: 1024px) {
  .editor-wrapper {
    height: 700px;
  }
}
```

## Performance Tips

### Memoization

Memoize expression objects to prevent unnecessary re-renders:

```tsx
import { useMemo } from 'react';

function OptimizedEditor({ config }) {
  const expression = useMemo(() => ({
    "and": [
      { ">=": [{ "var": "age" }, config.minAge] },
      { "var": "active" }
    ]
  }), [config.minAge]);

  return <DataLogicEditor value={expression} />;
}
```

### Debounced Data Updates

For frequently changing data in debug mode:

```tsx
import { useDeferredValue } from 'react';

function DebugWithDeferred({ expression, data }) {
  const deferredData = useDeferredValue(data);

  return (
    <DataLogicEditor
      value={expression}
      data={deferredData}
    />
  );
}
```
