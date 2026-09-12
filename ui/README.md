# @goplasmatic/datalogic-ui

[![npm](https://img.shields.io/npm/v/@goplasmatic/datalogic-ui)](https://www.npmjs.com/package/@goplasmatic/datalogic-ui)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A React component library for visualizing, debugging, and editing
JSONLogic expressions as interactive node-based flow diagrams.

This is the **React surface** of the
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs) monorepo.
It consumes the WASM binding
([`@goplasmatic/datalogic-wasm`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/wasm/README.md)) for evaluation
and tracing — for the engine itself and the cross-runtime overview,
see the [repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

## Features

- Visual representation of JSONLogic expressions as flow diagrams
- Every built-in operator the bundled engine accepts (84 canonical operators plus the `var`, `?:` and `match` aliases), across variables, comparison, logical, arithmetic, string, array, object, control flow, datetime, validation, error handling and the flagd feature-flag operators (`fractional`, `sem_ver`)
- Per-operator help with engine-verified examples and a link to that operator's documentation page
- Tree-based automatic layout using @dagrejs/dagre, in data-flow or JSON-hierarchy direction
- Prop-based modes: read-only visualization, debugging with step-through trace, and full visual editing
- Editing mode with node selection, properties panel, context menus, an Insert menu (Cmd/Ctrl+K) and undo/redo
- Templating mode for JSON templates with embedded JSONLogic
- Built-in WASM-based JSONLogic evaluation with execution tracing, a step timeline, and failed-node highlighting
- Engine evaluation settings (`config`) and custom operators (`customOperators`) passed straight through to the engine
- Light/dark theme support with system preference detection

## Installation

```bash
npm install @goplasmatic/datalogic-ui @xyflow/react
```

**Peer dependencies:** React 18+ or 19+, @xyflow/react 12+

## Quick Start

```tsx
import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor, type JsonLogicValue } from '@goplasmatic/datalogic-ui';

function App() {
  // Annotate the expression: TypeScript widens a bare object literal whose
  // array holds two different operator keys into a union that is not
  // assignable to JsonLogicValue.
  const expression: JsonLogicValue = {
    and: [
      { '>': [{ var: 'age' }, 18] },
      { '==': [{ var: 'status' }, 'active'] },
    ],
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

Provide `data` to enable the debugger toolbar (play/pause, step, jump, and a
step timeline). As you step, the current node shows its context and result in
a bubble, executed and on-path nodes are highlighted, and a node on the
engine's failure breadcrumb is marked with its error. Nodes do not show
results at rest; step through the trace to see values:

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
| `templating` | `boolean` | `false` | Enable templating mode: multi-key objects and arrays compile to output-shaping templates with embedded JSONLogic |
| `onTemplatingChange` | `(value: boolean) => void` | - | Callback when templating mode changes. The toolbar's Templating checkbox renders only when this is provided |
| `config` | `DataLogicEvaluationConfig` | - | Engine evaluation settings (preset, NaN and division-by-zero handling, truthiness, numeric coercion, recursion cap). Applied to both the result and the trace |
| `customOperators` | `Record<string, (args: unknown[]) => unknown>` | - | Custom operators registered on the engine. Rules using them evaluate and trace normally; their nodes render with the generic "utility" styling |
| `editable` | `boolean` | `false` | Enable editing: node selection, properties panel, context menus, Insert menu, undo/redo |
| `exampleSuggestions` | `string[]` | - | Example names shown as quick-action chips in the empty state. Chips render only when `onSelectExample` is also provided |
| `onSelectExample` | `(name: string) => void` | - | Called with the example name when a user clicks an empty-state chip |

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

### Constants (for customization)

```tsx
import { OPERATORS, CATEGORY_COLORS } from '@goplasmatic/datalogic-ui';
```

`OPERATORS` is the operator registry keyed by name: label, category, arity,
panel configuration, and help (summary, return type, notes, and examples that
are checked against the engine in this package's test suite). `CATEGORY_COLORS`
is a per-category palette for consumer-side legends and custom renderers; the
shipped nodes are coloured by the value type they produce, through the
`--sig-*` tokens (see Styling).

### Utilities and hooks (for advanced use)

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

- `jsonLogicToNodes(value, options?)` converts an expression into
  `{ nodes, edges, rootId }`.
- `applyTreeLayout(nodes, edges?, direction?)` positions them with dagre.
  `direction` is `'flow'` (default: sources on the left, result on the right)
  or `'hierarchy'` (root on the left, JSON nesting order).
- `useWasmEvaluator({ templating, config, customOperators })` is the same
  engine hook the component uses: it returns `{ ready, loading, error,
  evaluate, evaluateWithTrace }`.
- `DataLogicEvaluationError` carries the engine's structured error on
  `.structured` (`type`, `message`, and, where the engine provides them,
  `operator`, `node_ids`, `thrown`, `variable`, `index`, `length`, `stage`).

### Engine settings and custom operators

```tsx
<DataLogicEditor
  value={expression}
  data={data}
  config={{ preset: 'strict', division_by_zero: 'return_null' }}
  customOperators={{ double: (args) => Number(args[0]) * 2 }}
/>
```

`config` mirrors the engine's `EvaluationConfig`: `preset`
(`'default' | 'safe_arithmetic' | 'strict'`), `arithmetic_nan_handling`,
`division_by_zero`, `loose_equality_errors`, `truthy_evaluator`,
`numeric_coercion` (`empty_string_to_zero`, `null_to_zero`, `bool_to_number`,
`reject_non_numeric`) and `max_recursion_depth`. Every key is optional and
omitted keys keep the engine default. Changing `config` or `customOperators`
rebuilds the engine, so selection and undo history reset.

A custom operator receives the already-evaluated arguments and returns any
JSON-serializable value (`undefined` becomes `null`); a thrown exception
becomes a runtime evaluation error.

## Styling

One CSS import is all you need. React Flow's base styles are bundled into
`styles.css`, so there is no separate `@xyflow/react/dist/style.css` import
and no import-order requirement:

```tsx
import '@goplasmatic/datalogic-ui/styles.css';
```

The component sets `data-theme` on its own `.logic-editor` root from the
`theme` prop, falling back to the system preference. It does not read
`data-theme` from ancestor elements: pass the `theme` prop to force a theme.

Every design decision is a CSS custom property scoped to `.logic-editor`, so
overrides never leak into the host app. The primary axis is the signal
palette (`--sig-bool-true`, `--sig-bool-false`, `--sig-bool-rest`,
`--sig-number`, `--sig-string`, `--sig-collection`, `--sig-data`,
`--sig-temporal`, `--sig-null`, each with a `-bg` variant): a node is
coloured by the type of value it produces. The neutral substrate is
`--board`, `--surface`, `--chip`, `--hairline`, `--ink`, `--muted`, and
`--accent` is reserved for selection, root and focus.

```css
.logic-editor {
  --sig-number: #0ea5e9;
  --font-ui: 'Inter', sans-serif;
}
.logic-editor[data-theme="dark"] {
  --board: #0b0b0f;
}
```

The default `--font-ui` / `--font-mono` stacks name Space Grotesk and
JetBrains Mono but the package does not ship them; either install
`@fontsource/space-grotesk` and `@fontsource/jetbrains-mono` yourself or
override the two tokens. See
[Customization](https://goplasmatic.github.io/datalogic-rs/react-ui/customization.html)
for the full token list.

## Development

This package lives at `ui/` in the
[datalogic-rs monorepo](https://github.com/GoPlasmatic/datalogic-rs) and
bundles the WASM engine into its own output. It builds against the WASM
package vendored from the repo, not against a registry download: build the
WASM once, and the `predev` / `prebuild*` hooks copy `bindings/wasm/pkg` into
`ui/vendor/datalogic` on every run (`npm run sync-wasm`).

```bash
cd bindings/wasm && ./build.sh   # once, and after any engine change
cd ../../ui
npm install       # install dependencies
npm run dev       # start the dev playground (Studio)
npm test          # vitest: round trips, operator help, trace, samples
npm run lint      # run ESLint
npm run build:lib # build the publishable library bundle
npm run build:embed # build the docs-site embed bundle
```

`@goplasmatic/datalogic-wasm` is a devDependency pinned to the last published
release. Nothing in the build resolves it (Vite and the tsconfigs alias the
package to `vendor/datalogic`); the release workflow rewrites the pin to the
version being published. See
[DEVELOPMENT.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/DEVELOPMENT.md)
for the repo-wide pipeline.

### Tests

`npm test` runs vitest against the vendored engine, so the checks are real
evaluations rather than fixtures:

- **Operator registry and help** (`config/__tests__`): every registry entry
  matches `builtinOperatorNames()` from the engine, and every help example is
  evaluated and compared to its documented result.
- **Round trips** (`utils/__tests__`): a corpus covering every operator plus
  the shipped samples must survive `jsonLogicToNodes` to `nodesToJsonLogic`
  unchanged and evaluate identically.
- **Trace** (`utils/trace/__tests__`): real `evaluateWithTrace` envelopes must
  map onto the diagram with no synthetic nodes.
- **Samples, sharing, menus, evaluator** (`tests/`): each sample evaluates to
  its stored expected result, share URLs round trip, and every operator is
  reachable from the menus.

When you add an operator config example or a sample, give it the result the
engine actually produces; the suites will tell you if it drifts.

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
- **StructureNode**: Renders JSON objects/arrays in templating mode

## Tech Stack

- React 18/19
- TypeScript
- Vite
- React Flow (@xyflow/react)
- @dagrejs/dagre (graph layout)
- lucide-react (icons)
- @goplasmatic/datalogic-wasm (bundled into the library output)

The dev playground additionally uses @msgpack/msgpack and fflate for share
links and @fontsource for its fonts; those are devDependencies and are not
installed by consumers.

## Documentation

For complete documentation including all props, customization options, and advanced usage, see the [full documentation](https://goplasmatic.github.io/datalogic-rs/react-ui/installation.html).

## Learn more

- [Repo README](https://github.com/GoPlasmatic/datalogic-rs#readme) — cross-runtime overview, all binding READMEs
- [WASM binding README](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/wasm/README.md) — `@goplasmatic/datalogic-wasm`, the JS/TS engine this UI consumes
- [Rust crate README](https://github.com/GoPlasmatic/datalogic-rs/blob/main/crates/datalogic-rs/README.md) — engine design, the 5-tier API model
- [Full documentation](https://goplasmatic.github.io/datalogic-rs/)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)

## License

Apache-2.0
