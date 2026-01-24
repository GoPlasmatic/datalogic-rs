# Props & API Reference

Complete reference for the DataLogicEditor component and related exports.

## DataLogicEditor Props

### Required Props

#### `value`

The JSONLogic expression to render.

```tsx
value: JsonLogicValue | null
```

Accepts any valid JSONLogic expression or `null` for an empty state.

```tsx
// Simple expression
<DataLogicEditor value={{ "==": [1, 1] }} />

// Complex expression
<DataLogicEditor value={{
  "and": [
    { ">=": [{ "var": "age" }, 18] },
    { "var": "active" }
  ]
}} />

// Null for empty state
<DataLogicEditor value={null} />
```

### Optional Props

#### `mode`

The editor mode.

```tsx
mode?: 'visualize' | 'debug' | 'edit'
```

Default: `'visualize'`

```tsx
<DataLogicEditor value={expr} mode="debug" />
```

#### `data`

Data context for evaluation (required for debug mode).

```tsx
data?: unknown
```

```tsx
<DataLogicEditor
  value={{ "var": "user.name" }}
  data={{ user: { name: "Alice" } }}
  mode="debug"
/>
```

#### `onChange`

Callback when expression changes (for future edit mode).

```tsx
onChange?: (expression: JsonLogicValue | null) => void
```

```tsx
// Future usage
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  mode="edit"
/>
```

#### `theme`

Theme override.

```tsx
theme?: 'light' | 'dark'
```

Default: System preference

```tsx
<DataLogicEditor value={expr} theme="dark" />
```

#### `className`

Additional CSS class for the container.

```tsx
className?: string
```

```tsx
<DataLogicEditor value={expr} className="my-editor" />
```

---

## Type Definitions

### JsonLogicValue

The type for JSONLogic expressions:

```tsx
type JsonLogicValue =
  | string
  | number
  | boolean
  | null
  | JsonLogicValue[]
  | { [operator: string]: JsonLogicValue };
```

### DataLogicEditorMode

```tsx
type DataLogicEditorMode = 'visualize' | 'debug' | 'edit';
```

### DataLogicEditorProps

```tsx
interface DataLogicEditorProps {
  value: JsonLogicValue | null;
  onChange?: (expression: JsonLogicValue | null) => void;
  data?: unknown;
  mode?: DataLogicEditorMode;
  theme?: 'light' | 'dark';
  className?: string;
}
```

### LogicNode

Internal node type (for advanced customization):

```tsx
interface LogicNode {
  id: string;
  type: string;
  position: { x: number; y: number };
  data: {
    label: string;
    category: OperatorCategory;
    value?: unknown;
    result?: unknown;
  };
}
```

### LogicEdge

Internal edge type:

```tsx
interface LogicEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string;
  targetHandle?: string;
}
```

### OperatorCategory

```tsx
type OperatorCategory =
  | 'logical'
  | 'comparison'
  | 'arithmetic'
  | 'string'
  | 'array'
  | 'control'
  | 'variable'
  | 'literal'
  | 'datetime'
  | 'misc';
```

---

## Exports

### Component

```tsx
import { DataLogicEditor } from '@goplasmatic/datalogic-ui';
```

### Types

```tsx
import type {
  DataLogicEditorProps,
  DataLogicEditorMode,
  JsonLogicValue,
  LogicNode,
  LogicEdge,
  OperatorCategory,
} from '@goplasmatic/datalogic-ui';
```

### Constants

```tsx
import { OPERATORS, CATEGORY_COLORS } from '@goplasmatic/datalogic-ui';
```

**OPERATORS:** Map of operator names to their metadata (category, label, etc.)

**CATEGORY_COLORS:** Color definitions for each operator category

### Utilities

```tsx
import { jsonLogicToNodes, applyTreeLayout } from '@goplasmatic/datalogic-ui';
```

**jsonLogicToNodes:** Convert JSONLogic expression to React Flow nodes/edges

```tsx
const { nodes, edges } = jsonLogicToNodes(expression, traceData?);
```

**applyTreeLayout:** Apply dagre tree layout to nodes

```tsx
const layoutedNodes = applyTreeLayout(nodes, edges, direction?);
```

---

## Utility Functions

### jsonLogicToNodes

Convert a JSONLogic expression to React Flow nodes and edges.

```tsx
function jsonLogicToNodes(
  expression: JsonLogicValue,
  trace?: TraceData
): { nodes: LogicNode[]; edges: LogicEdge[] }
```

**Parameters:**
- `expression` - JSONLogic expression to convert
- `trace` - Optional trace data for debug mode

**Returns:** Object with `nodes` and `edges` arrays

**Example:**
```tsx
import { jsonLogicToNodes } from '@goplasmatic/datalogic-ui';

const expr = { "==": [{ "var": "x" }, 1] };
const { nodes, edges } = jsonLogicToNodes(expr);

console.log(nodes);
// [
//   { id: '0', type: 'operator', data: { label: '==', category: 'comparison' }, ... },
//   { id: '1', type: 'variable', data: { label: 'x', category: 'variable' }, ... },
//   { id: '2', type: 'literal', data: { label: '1', category: 'literal' }, ... }
// ]
```

### applyTreeLayout

Apply dagre-based tree layout to nodes.

```tsx
function applyTreeLayout(
  nodes: LogicNode[],
  edges: LogicEdge[],
  direction?: 'TB' | 'LR'
): LogicNode[]
```

**Parameters:**
- `nodes` - Array of nodes
- `edges` - Array of edges
- `direction` - Layout direction (default: `'TB'` for top-to-bottom)

**Returns:** Nodes with updated positions

---

## Advanced Usage

### Custom Node Rendering

For advanced customization, you can use the utilities to render with your own React Flow setup:

```tsx
import { ReactFlow } from '@xyflow/react';
import { jsonLogicToNodes, applyTreeLayout } from '@goplasmatic/datalogic-ui';

function CustomEditor({ expression }) {
  const { nodes: rawNodes, edges } = jsonLogicToNodes(expression);
  const nodes = applyTreeLayout(rawNodes, edges);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={customNodeTypes}
      // Custom configuration...
    />
  );
}
```

### Accessing Category Colors

```tsx
import { CATEGORY_COLORS } from '@goplasmatic/datalogic-ui';

// Use in custom styling
const logicalColor = CATEGORY_COLORS.logical;  // e.g., '#4CAF50'
```

## Next Steps

- [Customization](customization.md) - Theming and styling options
