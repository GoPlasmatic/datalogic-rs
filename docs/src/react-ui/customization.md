# Customization

This guide covers theming, styling, and advanced customization of the DataLogicEditor.

## Theming

### System Theme (Default)

By default, the editor detects system theme preference:

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

The component's theme variables are scoped to its `.logic-editor` root element (not `:root`), so they do not leak into the rest of your app. To override them, target the same scope. The dark theme is applied via `.logic-editor[data-theme="dark"]`.

These are the real variable names defined by the component. The values below are the light-theme defaults:

```css
.logic-editor {
  /* Backgrounds */
  --bg-primary: #fafafa;
  --bg-secondary: #ffffff;
  --bg-tertiary: #f6f6f7;
  --bg-hover: #f0f0f1;
  --bg-active: #e8e8ea;

  /* Text */
  --text-primary: #18181b;
  --text-secondary: #3f3f46;
  --text-tertiary: #71717a;
  --text-muted: #a1a1aa;
  --text-placeholder: #c4c4c7;

  /* Borders */
  --border-primary: rgba(0, 0, 0, 0.10);
  --border-secondary: rgba(0, 0, 0, 0.06);
  --border-light: rgba(0, 0, 0, 0.04);

  /* Accents */
  --accent-blue: #6366f1;
  --accent-blue-light: #e0e7ff;
  --accent-blue-hover: #4f46e5;
  --accent-amber: #f59e0b;
  --accent-amber-light: #fef3c7;

  /* Nodes */
  --node-bg: #ffffff;
  --node-shadow: 0 1px 3px rgba(0, 0, 0, 0.06), 0 1px 2px rgba(0, 0, 0, 0.04);
  --node-shadow-hover: 0 4px 12px rgba(0, 0, 0, 0.08), 0 2px 4px rgba(0, 0, 0, 0.05);
}
```

Override any of them by re-declaring on the same scope (the dark variant lives on `.logic-editor[data-theme="dark"]`):

```css
.logic-editor {
  --accent-blue: #3b82f6;
  --node-bg: #ffffff;
}

.logic-editor[data-theme="dark"] {
  --node-bg: #18181b;
  --node-shadow: 0 1px 3px rgba(0, 0, 0, 0.4), 0 1px 2px rgba(0, 0, 0, 0.3);
}
```

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

> **Note:** There are three node types: `operator`, `literal`, and `structure`. There is no `variable` node type, variables (`var` / `val`) render as operator nodes, so a `.react-flow__node-variable` selector matches nothing.

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
import { Handle, Position } from '@xyflow/react';
import { CATEGORY_COLORS } from '@goplasmatic/datalogic-ui';

function CustomOperatorNode({ data }) {
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
      {data.result !== undefined && (
        <div style={{ fontSize: '0.75em', opacity: 0.8 }}>
          = {JSON.stringify(data.result)}
        </div>
      )}
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

const customNodeTypes = {
  operator: CustomOperatorNode,
  // ... other custom types
};
```

### Category Colors

Access and customize category colors:

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
//   control: '#f59e0b',
//   datetime: '#0ea5e9',
//   validation: '#94a3b8',
//   utility: '#64748b',
//   error: '#ef4444',
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
