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

### Parent-Based Theme

The component respects `data-theme` on parent elements:

```tsx
<div data-theme="dark">
  <DataLogicEditor value={expression} />
</div>
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

Override CSS variables for global styling:

```css
:root {
  /* Node colors by category */
  --datalogic-logical-bg: #4caf50;
  --datalogic-comparison-bg: #2196f3;
  --datalogic-arithmetic-bg: #ff9800;
  --datalogic-string-bg: #9c27b0;
  --datalogic-array-bg: #00bcd4;
  --datalogic-variable-bg: #607d8b;
  --datalogic-literal-bg: #795548;

  /* General theming */
  --datalogic-bg: #ffffff;
  --datalogic-text: #1a1a1a;
  --datalogic-border: #e5e7eb;
  --datalogic-node-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

[data-theme="dark"] {
  --datalogic-bg: #1a1a1a;
  --datalogic-text: #ffffff;
  --datalogic-border: #374151;
  --datalogic-node-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}
```

### Node Styling

Target specific node types:

```css
/* All nodes */
.react-flow__node {
  font-family: 'Inter', sans-serif;
}

/* Operator nodes */
.react-flow__node-operator {
  border-width: 2px;
}

/* Variable nodes */
.react-flow__node-variable {
  font-style: italic;
}

/* Literal nodes */
.react-flow__node-literal {
  font-weight: bold;
}
```

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
//   logical: '#4CAF50',
//   comparison: '#2196F3',
//   arithmetic: '#FF9800',
//   string: '#9C27B0',
//   array: '#00BCD4',
//   control: '#F44336',
//   variable: '#607D8B',
//   literal: '#795548',
//   datetime: '#3F51B5',
//   misc: '#9E9E9E'
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
      mode="debug"
    />
  );
}
```
