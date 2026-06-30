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

#### `data`

Data context for evaluation. When provided, the debugger controls become available and each node shows its evaluated result via the WASM trace API.

```tsx
data?: unknown
```

```tsx
<DataLogicEditor
  value={{ "var": "user.name" }}
  data={{ user: { name: "Alice" } }}
/>
```

#### `onChange`

Callback fired when the expression changes. It is active whenever `editable` is set: edits in the canvas are debounced (about 300ms) and the rebuilt JSONLogic expression is passed back.

```tsx
onChange?: (expr: JsonLogicValue | null) => void
```

```tsx
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  editable
/>
```

#### `editable`

Enable editing: node selection, properties panel, context menus, and undo/redo.

```tsx
editable?: boolean
```

Default: `false`

```tsx
<DataLogicEditor value={expr} onChange={setExpr} editable />
```

#### `templating`

Enable templating mode: multi-key objects and arrays in compiled rules become output-shaping templates with embedded JSONLogic expressions, rather than being rejected as invalid JSONLogic. Matches the v5 core API (`Engine::builder().with_templating(true)`).

```tsx
templating?: boolean
```

Default: `false`

```tsx
<DataLogicEditor value={expr} templating />
```

#### `onTemplatingChange`

Callback fired when templating mode changes from the toolbar checkbox.

```tsx
onTemplatingChange?: (value: boolean) => void
```

```tsx
<DataLogicEditor
  value={expr}
  templating={templating}
  onTemplatingChange={setTemplating}
/>
```

#### `exampleSuggestions`

Optional list of example names to surface as quick-action chips in the empty state. Each chip, when clicked, calls `onSelectExample` with the corresponding name. Ignored when the editor is non-empty.

```tsx
exampleSuggestions?: string[]
```

```tsx
<DataLogicEditor
  value={null}
  exampleSuggestions={['Age check', 'Discount rule']}
  onSelectExample={loadExample}
/>
```

#### `onSelectExample`

Callback invoked when a user clicks an empty-state example chip. Receives the example name from `exampleSuggestions`.

```tsx
onSelectExample?: (name: string) => void
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

### DataLogicEditorProps

```tsx
interface DataLogicEditorProps {
  value: JsonLogicValue | null;
  onChange?: (expr: JsonLogicValue | null) => void;
  data?: unknown;
  theme?: 'light' | 'dark';
  className?: string;
  templating?: boolean;
  onTemplatingChange?: (value: boolean) => void;
  editable?: boolean;
  exampleSuggestions?: string[];
  onSelectExample?: (name: string) => void;
}
```

### LogicNode

A React Flow node carrying our custom node data (for advanced customization):

```tsx
import type { Node } from '@xyflow/react';

type LogicNode = Node<LogicNodeData>;

type LogicNodeData = OperatorNodeData | LiteralNodeData | StructureNodeData;
```

The `data` payload is one of three shapes, discriminated by its `type` field:

```tsx
interface OperatorNodeData {
  type: 'operator';
  operator: string;
  category: OperatorCategory;
  label: string;
  icon: IconName;
  cells: CellData[];        // all arguments as rows
  collapsed?: boolean;
  expressionText?: string;  // single-line text when collapsed
}

interface LiteralNodeData {
  type: 'literal';
  value: JsonLogicValue;
  valueType: 'string' | 'number' | 'boolean' | 'null' | 'array';
}

interface StructureNodeData {
  type: 'structure';
  isArray: boolean;
  formattedJson: string;
  elements: StructureElement[];
  collapsed?: boolean;
  expressionText?: string;
}
```

### LogicEdge

An alias for the React Flow `Edge` type:

```tsx
import type { Edge } from '@xyflow/react';

type LogicEdge = Edge;
```

### OperatorCategory

```tsx
type OperatorCategory =
  | 'variable'
  | 'comparison'
  | 'logical'
  | 'arithmetic'
  | 'control'
  | 'string'
  | 'array'
  | 'datetime'
  | 'validation'
  | 'error'
  | 'utility';
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
  JsonLogicValue,
  LogicNode,
  LogicEdge,
  LogicNodeData,
  OperatorNodeData,
  VariableNodeData,
  LiteralNodeData,
  NodeEvaluationResult,
  EvaluationResultsMap,
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
const { nodes, edges, rootId } = jsonLogicToNodes(expression, { templating });
```

**applyTreeLayout:** Apply dagre tree layout to nodes

```tsx
const layoutedNodes = applyTreeLayout(nodes, edges);
```

---

## Utility Functions

### jsonLogicToNodes

Convert a JSONLogic expression to React Flow nodes and edges.

```tsx
function jsonLogicToNodes(
  expr: JsonLogicValue | null,
  options?: { templating?: boolean }
): ConversionResult

interface ConversionResult {
  nodes: LogicNode[];
  edges: LogicEdge[];
  rootId: string | null;
}
```

**Parameters:**
- `expr` - JSONLogic expression to convert (`null` yields an empty result)
- `options.templating` - When `true`, multi-key objects compile to output-shaping templates with embedded JSONLogic

**Returns:** A `ConversionResult` with `nodes`, `edges`, and `rootId` (the id of the root node, or `null` for an empty expression)

**Example:**
```tsx
import { jsonLogicToNodes } from '@goplasmatic/datalogic-ui';

const expr = { "==": [{ "var": "x" }, 1] };
const { nodes, edges, rootId } = jsonLogicToNodes(expr);

// nodes: LogicNode[], React Flow nodes whose `data` is
//   OperatorNodeData | LiteralNodeData | StructureNodeData (node `type` is
//   'operator' | 'literal' | 'structure'). The `==` and `var` expressions
//   become operator nodes (categories 'comparison' and 'variable'); the
//   `1` becomes a literal node.
// edges: LogicEdge[], React Flow edges linking each operator to its arguments
// rootId: string, id of the root `==` node
```

### applyTreeLayout

Apply dagre-based tree layout to nodes.

```tsx
function applyTreeLayout(
  nodes: LogicNode[],
  edges?: LogicEdge[]
): LogicNode[]
```

**Parameters:**
- `nodes` - Array of nodes
- `edges` - Optional array of edges. When omitted, edges are derived from the node relationships

**Returns:** Nodes with updated positions and dimensions. The layout flows left-to-right.

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
const logicalColor = CATEGORY_COLORS.logical;  // '#8b5cf6'
```

## Next Steps

- [Customization](customization.md) - Theming and styling options
