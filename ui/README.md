# @goplasmatic/datalogic-ui

A React component library for visualizing, debugging, and editing JSONLogic expressions as interactive node-based flow diagrams.

## Features

- Visual representation of JSONLogic expressions as flow diagrams
- Support for all standard JSONLogic operators (logical, comparison, arithmetic, string, array, control flow, datetime, error handling)
- Tree-based automatic layout using @dagrejs/dagre
- Prop-based modes: read-only visualization, debugging with step-through trace, and full visual editing
- Editing mode with node selection, properties panel, context menus, and undo/redo
- Structure preserve mode for JSON templates with embedded JSONLogic
- Built-in WASM-based JSONLogic evaluation with execution tracing
- Light/dark theme support with system preference detection

## Installation

```bash
npm install @goplasmatic/datalogic-ui @xyflow/react
```

**Peer dependencies:** React 18+ or 19+, @xyflow/react 12+

## Quick Start

```tsx
import '@xyflow/react/dist/style.css';
import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

function App() {
  const expression = {
    "and": [
      { ">": [{ "var": "age" }, 18] },
      { "==": [{ "var": "status" }, "active"] }
    ]
  };

  return <DataLogicEditor value={expression} />;
}
```

## Usage Modes

The editor behavior is controlled by props rather than a mode enum. Different combinations of props enable different functionality:

### Read-only (default)

Simply render a JSONLogic expression as a flow diagram:

```tsx
<DataLogicEditor value={expression} />
```

### With Debugger

Provide `data` to enable debugger controls with step-through execution trace:

```tsx
<DataLogicEditor
  value={expression}
  data={{ age: 25, status: "active" }}
/>
```

### Editable

Enable full visual editing with node selection, properties panel, context menus, and undo/redo:

```tsx
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  editable
/>
```

### Editable + Debugger

Combine editing with live debugging:

```tsx
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  data={{ age: 25, status: "active" }}
  editable
/>
```

## Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `value` | `JsonLogicValue \| null` | required | JSONLogic expression to render |
| `onChange` | `(expr: JsonLogicValue \| null) => void` | - | Callback when expression changes (only when `editable` is true) |
| `data` | `unknown` | - | Data context for evaluation. When provided, debugger controls become available |
| `theme` | `'light' \| 'dark'` | system | Theme override. If not provided, uses system preference |
| `className` | `string` | - | Additional CSS class |
| `preserveStructure` | `boolean` | `false` | Enable structure preserve mode for JSON templates with embedded JSONLogic |
| `onPreserveStructureChange` | `(value: boolean) => void` | - | Callback when preserve structure changes (from toolbar checkbox) |
| `editable` | `boolean` | `false` | Enable editing: node selection, properties panel, context menus, undo/redo |

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

### Constants (for customization)

```tsx
import { OPERATORS, CATEGORY_COLORS } from '@goplasmatic/datalogic-ui';
```

### Utilities (for advanced use)

```tsx
import { jsonLogicToNodes, applyTreeLayout } from '@goplasmatic/datalogic-ui';
```

## Styling

The component requires two CSS imports:

```tsx
// React Flow base styles
import '@xyflow/react/dist/style.css';

// DataLogicEditor styles
import '@goplasmatic/datalogic-ui/styles.css';
```

The component respects the `data-theme` attribute on parent elements for theming, or you can override with the `theme` prop.

## Development

```bash
pnpm install      # Install dependencies
pnpm dev:ui       # Start development server
pnpm build:ui:lib # Build library for publishing
pnpm lint:ui      # Run ESLint
```

## Architecture

The main component is `DataLogicEditor` which:

1. Accepts a `value` prop (JSONLogic expression) and renders it as a flow diagram
2. Uses React Flow (`@xyflow/react`) for the node canvas
3. Internally loads WASM module for JSONLogic evaluation and execution tracing
4. Supports read-only, debugger, and editable modes via props

### Data Flow

1. **JSONLogic Input** → `useLogicEditor` hook parses the expression
2. **Conversion** → `jsonLogicToNodes()` transforms JSONLogic to visual nodes/edges
3. **Layout** → `applyTreeLayout()` positions nodes in a tree structure
4. **Rendering** → React Flow renders with custom node types

### Node Types

- **OperatorNode** (UnifiedOperatorNode): Renders all operators with cell-based argument display (and, or, if, var, val, ==, +, etc.)
- **LiteralNode**: Renders primitive values (strings, numbers, booleans, null)
- **StructureNode**: Renders JSON objects/arrays in structure preserve mode

## Tech Stack

- React 18/19
- TypeScript
- Vite
- React Flow (@xyflow/react)
- @dagrejs/dagre (graph layout)
- lucide-react (icons)
- @msgpack/msgpack (data serialization)
- fflate (compression)
- @goplasmatic/datalogic (bundled, for WASM evaluation)

## Documentation

For complete documentation including all props, customization options, and advanced usage, see the [full documentation](https://goplasmatic.github.io/datalogic-rs/react-ui/installation.html).

## Links

- [GitHub Repository](https://github.com/GoPlasmatic/datalogic-rs)
- [Full Documentation](https://goplasmatic.github.io/datalogic-rs/)
- [Online Playground](https://goplasmatic.github.io/datalogic-rs/playground/)

## License

MIT
