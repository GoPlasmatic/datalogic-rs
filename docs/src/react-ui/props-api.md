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

Data context for evaluation. When provided, the editor evaluates the expression through the WASM trace API and the debugger controls become available. Any JSON value is a valid root context: object, array or scalar. Values appear on nodes as you step, not at rest (see [Modes](modes.md#debugging)).

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

Callback fired when the expression changes. It is active whenever `editable` is set: the editor debounces canvas edits (about 300ms) and passes back the rebuilt JSONLogic expression.

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

Enable editing: node selection, properties panel, context menus, the Insert menu (Cmd/Ctrl+K), keyboard shortcuts, and undo/redo.

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

Callback fired when templating mode changes from the toolbar checkbox. The checkbox renders only when this prop is provided; with `templating` alone the mode is fixed.

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

Optional list of example names to surface as quick-action chips in the empty state. Each chip, when clicked, calls `onSelectExample` with the corresponding name. Chips render only when you provide both `exampleSuggestions` and `onSelectExample` and the editor is empty.

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

#### `config`

Engine evaluation settings, applied to both the plain result and the traced
run. Mirrors the core `EvaluationConfig`; every key is optional and omitted
keys keep the engine default (or the selected preset's value).

```tsx
config?: DataLogicEvaluationConfig

interface DataLogicEvaluationConfig {
  preset?: 'default' | 'safe_arithmetic' | 'strict';
  arithmetic_nan_handling?: 'throw_error' | 'ignore_value' | 'coerce_to_zero' | 'return_null';
  division_by_zero?: 'return_saturated' | 'throw_error' | 'return_null' | 'return_infinity';
  loose_equality_errors?: boolean;
  truthy_evaluator?: 'javascript' | 'python' | 'strict_boolean';
  numeric_coercion?: {
    empty_string_to_zero?: boolean;
    null_to_zero?: boolean;
    bool_to_number?: boolean;
    reject_non_numeric?: boolean;
  };
  max_recursion_depth?: number;
}
```

```tsx
<DataLogicEditor
  value={expr}
  data={data}
  config={{ preset: 'strict', division_by_zero: 'return_null' }}
/>
```

The toolbar shows a compact summary whenever the settings differ from the
engine defaults. Changing `config` rebuilds the engine, which resets selection
and undo history, so keep the object referentially stable (`useMemo`) if the
parent re-renders often. The engine rejects an unknown key or value with a
`ConfigurationError`. See
[Configuration](../advanced/configuration.md) for what each setting does.

#### `customOperators`

Custom operators registered on the evaluation engine, keyed by operator name.

```tsx
customOperators?: Record<string, (args: unknown[]) => unknown>
```

```tsx
<DataLogicEditor
  value={{ discounted: [{ var: 'price' }] }}
  data={{ price: 100 }}
  customOperators={{ discounted: (args) => Number(args[0]) * 0.9 }}
/>
```

Arguments arrive already evaluated; the return value may be any
JSON-serializable value (`undefined` becomes `null`), and a thrown exception
becomes a runtime evaluation error. Rules using them evaluate and trace
normally, but the palette and help panel only know built-in operators, so
custom nodes render with the generic "utility" styling. Built-ins win a name
collision: registering `"+"` has no effect. Like `config`, changing this prop
rebuilds the engine.

#### `theme`

Theme override.

```tsx
theme?: 'light' | 'dark'
```

Default: System preference

```tsx
<DataLogicEditor value={expr} theme="dark" />
```

The component writes `data-theme` onto its own `.logic-editor` root; it does not read a `data-theme` set on an ancestor.

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

Annotate expression literals whose arrays hold more than one operator key.
Without the annotation TypeScript widens the array into a union of
per-key object types, which the index signature does not accept:

```tsx
const expression: JsonLogicValue = {
  and: [
    { '>': [{ var: 'age' }, 18] },
    { '==': [{ var: 'status' }, 'active'] },
  ],
};
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
  config?: DataLogicEvaluationConfig;
  customOperators?: Record<string, DataLogicCustomOperator>;
  editable?: boolean;
  exampleSuggestions?: string[];
  onSelectExample?: (name: string) => void;
}
```

### LogicNode

A React Flow node carrying the package's custom node data (for advanced customization):

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
  | 'object'
  | 'datetime'
  | 'validation'
  | 'error'
  | 'utility'
  | 'flagd';
```

`CATEGORY_COLORS` is keyed by `NodeCategory`, which is `OperatorCategory` plus
`'literal'` for literal nodes.

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
  DataLogicEvaluationConfig,
  DataLogicCustomOperator,
  JsonLogicValue,
  JsonLogicToNodesOptions,
  LogicNode,
  LogicEdge,
  LogicNodeData,
  OperatorNodeData,
  VariableNodeData,
  LiteralNodeData,
  StructureNodeData,
  StructureElement,
  CellData,
  ConversionResult,
  NodeEvaluationResult,
  EvaluationResultsMap,
  StructuredError,
  TracedResult,
  OperatorCategory,
  FlowDirection,
  IconName,
} from '@goplasmatic/datalogic-ui';
```

### Constants

```tsx
import { OPERATORS, CATEGORY_COLORS } from '@goplasmatic/datalogic-ui';
```

**OPERATORS:** the operator registry, keyed by name. Each entry carries its
label, category, arity, properties-panel configuration, and help (summary,
return type, notes, and examples). The registry covers every operator the
bundled engine accepts, and the package's test suite evaluates every help
example against the engine, so the metadata cannot drift from the runtime.

**CATEGORY_COLORS:** a per-category palette for consumer-side legends,
pickers and custom node renderers. The shipped nodes are not coloured by
category: they are coloured by the type of value they produce, through the
`--sig-*` tokens described in [Customization](customization.md#css-variables).

### Utilities and hooks

```tsx
import {
  jsonLogicToNodes,
  applyTreeLayout,
  useWasmEvaluator,
  DataLogicEvaluationError,
  summarizeEvaluationConfig,
  isDefaultEvaluationConfig,
} from '@goplasmatic/datalogic-ui';
```

**jsonLogicToNodes:** Convert JSONLogic expression to React Flow nodes/edges

```tsx
const { nodes, edges, rootId } = jsonLogicToNodes(expression, { templating });
```

**applyTreeLayout:** Apply dagre tree layout to nodes

```tsx
const layoutedNodes = applyTreeLayout(nodes, edges, 'flow');
```

**useWasmEvaluator:** the engine hook the component uses internally. It loads
the bundled WASM engine and builds one `Engine` per
(`templating`, `config`, `customOperators`) combination:

```tsx
const { ready, loading, error, evaluate, evaluateWithTrace } = useWasmEvaluator({
  templating: false,
  config: { preset: 'strict' },
  customOperators: { double: (args) => Number(args[0]) * 2 },
});

if (ready) {
  const result = evaluate({ '+': [1, 2] }, {});           // 3
  const trace = evaluateWithTrace({ '+': [1, 2] }, {});   // { result, steps, expression_tree, ... }
}
```

**DataLogicEvaluationError:** thrown by `evaluate` when the engine fails. Its
`.structured` field is a `StructuredError` carrying `type` and `message`, plus
`operator`, `node_ids`, `thrown`, `variable`, `index`, `length` or `stage`
where the engine provides them.

**summarizeEvaluationConfig / isDefaultEvaluationConfig:** format a
`DataLogicEvaluationConfig` as the one-line summary the toolbar shows, and
test whether it matches the engine defaults.

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
  edges?: LogicEdge[],
  direction?: FlowDirection   // 'flow' | 'hierarchy', default 'flow'
): LogicNode[]
```

**Parameters:**
- `nodes` - Array of nodes
- `edges` - Optional array of edges. When omitted, the function derives edges from the node relationships
- `direction` - `'flow'` (default) lays the graph out left-to-right in data-flow order: leaf operands on the left, the root's result on the right. `'hierarchy'` also runs left-to-right but ranks the root first, matching JSON nesting order. The component's toolbar toggles between the two and reflects the choice as `data-direction` on the `.logic-editor` root.

**Returns:** Nodes with updated positions and dimensions.

---

## Advanced Usage

### Custom Node Rendering

For advanced customization, use the utilities to render with your own React Flow setup:

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

// Use in your own legends, pickers or custom node renderers
const logicalColor = CATEGORY_COLORS.logical;  // '#8b5cf6'
```

To re-theme the shipped nodes, override the `--sig-*` tokens instead: see
[Customization](customization.md#css-variables).

## Next Steps

- [Customization](customization.md) - Theming and styling options
