# @goplasmatic/datalogic-ui

A React component library for visualizing and debugging JSONLogic expressions as interactive node-based flow diagrams.

## Demo

![DataLogic Debugger Demo](public/demo.gif)

## Features

- Visual representation of JSONLogic expressions as flow diagrams
- Support for all standard JSONLogic operators (logical, comparison, arithmetic, string, array, control flow)
- Tree-based automatic layout using dagre
- Three modes: Visualization, Debugging, and Editing (future)
- Built-in WASM-based JSONLogic evaluation
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

The editor supports three modes, each providing different levels of functionality:

| Mode | API Value | Description |
|------|-----------|-------------|
| ReadOnly | `'visualize'` | Static diagram visualization, no evaluation |
| Debugger | `'debug'` | Diagram with evaluation results and step-through debugging |
| Editor | `'edit'` | **Coming Soon** - Full visual builder with live evaluation |

### Mode 1: ReadOnly (`visualize`) - Default

Simply render a JSONLogic expression as a flow diagram:

```tsx
<DataLogicEditor value={expression} />
```

### Mode 2: Debugger (`debug`)

Render with evaluation results and step-through debugging. Requires a `data` context:

```tsx
<DataLogicEditor
  value={expression}
  data={{ age: 25, status: "active" }}
  mode="debug"
/>
```

### Mode 3: Editor (`edit`) - Coming Soon

Full visual builder with two-way binding and live evaluation. This mode is planned for a future release.

```tsx
// Coming Soon
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  data={contextData}
  mode="edit"
/>
```

> **Note:** Using `mode="edit"` currently renders the component in read-only mode with debug evaluation (if `data` is provided). A console warning will indicate this.

## Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `value` | `JsonLogicValue \| null` | required | JSONLogic expression to render |
| `onChange` | `(expr: JsonLogicValue \| null) => void` | - | Callback when expression changes (edit mode only) |
| `data` | `unknown` | - | Data context for evaluation (debug mode) |
| `mode` | `'visualize' \| 'debug' \| 'edit'` | `'visualize'` | Editor mode |
| `theme` | `'light' \| 'dark'` | system | Theme override |
| `className` | `string` | - | Additional CSS class |

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
npm install       # Install dependencies
npm run dev       # Start development server
npm run build     # Build demo app
npm run build:lib # Build library for publishing
npm run lint      # Run ESLint
```

## Architecture

The main component is `DataLogicEditor` which:

1. Accepts a `value` prop (JSONLogic expression) and renders it as a flow diagram
2. Uses React Flow (`@xyflow/react`) for the node canvas
3. Internally loads WASM module for JSONLogic evaluation (in debug mode)
4. Supports three rendering modes: visualize, debug, and edit

### Data Flow

1. **JSONLogic Input** → `useLogicEditor` hook parses the expression
2. **Conversion** → `jsonLogicToNodes()` transforms JSONLogic to visual nodes/edges
3. **Layout** → `applyTreeLayout()` positions nodes in a tree structure
4. **Rendering** → React Flow renders with custom node types

### Node Types

- **OperatorNode**: Renders operators (and, or, if, ==, +, etc.)
- **VariableNode**: Renders variable references (var, val, exists)
- **LiteralNode**: Renders literal values (strings, numbers, booleans)
- **VerticalCellNode**: Renders vertical layouts for comparison chains and iterators

## Tech Stack

- React 18/19
- TypeScript
- Vite
- React Flow (@xyflow/react)
- dagre (for graph layout)
- @goplasmatic/datalogic (bundled, for WASM evaluation)

## Roadmap

- **Full Visual Builder (Edit Mode)**: Interactive visual editing of JSONLogic expressions with drag-and-drop node creation, connection editing, and real-time evaluation feedback.

## License

MIT
