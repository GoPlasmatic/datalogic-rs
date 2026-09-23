# Usage Modes

The DataLogicEditor has no `mode` enum. The props you pass determine its behavior. The same component is a read-only viewer, a live debugger, a visual editor, or any combination of those, depending on `data`, `editable`, and `templating`.

## Behavior Overview

| Behavior | Enabled by | Description | Requires `data` |
|----------|------------|-------------|-----------------|
| Read-only | (none) | Static diagram visualization | No |
| Debugger | `data` | Step-through execution trace with a step timeline and failure highlighting | Yes |
| Editing | `editable` | Visual builder: node selection, properties panel, context menus, undo/redo | No |
| Templating | `templating` | Multi-key objects and arrays become output-shaping templates | No |
| Engine settings | `config` | Evaluation semantics: presets, NaN and division-by-zero handling, truthiness, coercion, recursion cap | No |
| Custom operators | `customOperators` | Extra operators registered on the engine | No |

You can combine these. Setting `editable` and providing `data` at the same time gives you live debugging while you edit.

## Read-only (Default)

With only a `value`, the editor renders a static flow diagram of the JSONLogic expression.

```tsx
<DataLogicEditor value={expression} />
```

**Use cases:**
- Documentation and explanation
- Code review and understanding
- Static representation in reports

**Features:**
- Interactive pan and zoom
- Node highlighting on hover
- Tree-based automatic layout
- Nodes coloured by the type of value they produce (boolean, number, string, collection, data, temporal, null), with a category icon in the header
- A Flow/Hierarchy toolbar toggle: **Flow** (default) puts sources on the left and the result on the right, **Hierarchy** puts the root on the left in JSON nesting order. The editor reflects the choice as `data-direction` on the `.logic-editor` root

## Debugging

Provide a `data` prop and the editor evaluates the expression with the engine's tracing API and exposes debugger controls for stepping through the execution.

```tsx
<DataLogicEditor
  value={expression}
  data={contextData}
/>
```

**Use cases:**
- Understanding evaluation flow
- Debugging unexpected results
- Testing expressions with different inputs
- Learning JSONLogic

**Features:**
- All read-only features, plus:
- Play/pause, step forward and back, and jump to first/last (Space, arrow keys, Home/End)
- A step timeline listing every recorded step with its node, iteration index, context and result, with click-to-jump
- A bubble on the current node showing the context it evaluated against and the value it produced
- A highlighted execution path, so you can see which branch ran
- Failure reporting: the editor marks the node on the engine's failure breadcrumb (`node_ids` in the structured error) with the error, and a rule that fails to compile reports the error in a banner above the diagram

Values appear as you step. No node shows a result at rest.

Internally, when you provide `data` the component uses the WASM `evaluateWithTrace` API to capture the result of each sub-expression, the order of evaluation, context values at each step, and the final computed result.

## Editing

Set `editable` to turn on the full visual builder.

```tsx
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  editable
/>
```

**Features:**
- Node selection
- Properties panel for the selected node, with per-operator help and a link to that operator's documentation page
- Context menus (right-click a node or the canvas)
- An **Insert** toolbar button (Cmd/Ctrl+K) that adds an argument to the selection, wraps it, or targets the root
- Undo/redo, from the toolbar or the keyboard
- Keyboard shortcuts: copy/paste (Cmd/Ctrl+C / V), duplicate (Cmd/Ctrl+D), select all (Cmd/Ctrl+A), undo/redo (Cmd/Ctrl+Z, Shift+Cmd/Ctrl+Z or Cmd/Ctrl+Y), delete (Backspace/Delete), deselect (Escape)

When `editable` is set, `onChange` is active: the editor debounces edits (about 300ms) and passes back the rebuilt JSONLogic expression so you can keep your own state in sync.

## Editing with Live Debugging

Combine `editable` with `data` to edit and debug in the same view: the trace re-runs as you build, so you can step through the expression you are editing.

```tsx
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  data={contextData}
  editable
/>
```

## Templating

Set `templating` so that multi-key objects and arrays in the compiled rule become output-shaping templates with embedded JSONLogic, rather than being rejected as invalid JSONLogic. This matches the v5 core API (`Engine::builder().with_templating(true)`). Passing `onTemplatingChange` adds a Templating checkbox to the toolbar; with `templating` alone the mode is fixed and no checkbox renders.

```tsx
<DataLogicEditor
  value={expression}
  templating={templating}
  onTemplatingChange={setTemplating}
/>
```

## Engine Settings and Custom Operators

`config` changes evaluation semantics for both the result and the trace, and
`customOperators` registers extra operators on the engine:

```tsx
<DataLogicEditor
  value={expression}
  data={contextData}
  config={{ preset: 'safe_arithmetic', truthy_evaluator: 'python' }}
  customOperators={{ double: (args) => Number(args[0]) * 2 }}
/>
```

The toolbar shows a summary whenever settings differ from the engine defaults.
Both props rebuild the engine when they change, which resets selection and
undo history, so keep them referentially stable (for example with `useMemo`)
if the surrounding component re-renders often. See
[Props & API](props-api.md#config) for every key.

## Behavior Comparison

| Aspect | Read-only | Debugger (`data`) | Editing (`editable`) |
|--------|-----------|-------------------|----------------------|
| Node display | Structure only | Structure, plus values on the current step | Editable nodes |
| Interactivity | Pan/zoom | Pan/zoom + stepping | Full editing |
| `data` required | No | Yes | No |
| Output | Static | Static + trace | Two-way bound via `onChange` |

### Performance Considerations

- **Read-only** is fastest: no evaluation overhead.
- **Debugger** runs evaluation on every `data` change.
- **Editing** rebuilds the expression on each change (debounced before `onChange` fires).

For large expressions or frequent data updates, consider debouncing the `data` you pass in:

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

## Toggling Behavior at Runtime

Because behavior is prop-driven, you toggle it by toggling props. For example, to switch between plain visualization and debugging, conditionally pass `data`:

```tsx
function DebugToggle() {
  const [debug, setDebug] = useState(false);

  return (
    <div>
      <button onClick={() => setDebug((d) => !d)}>
        {debug ? 'Stop debugging' : 'Debug'}
      </button>

      <DataLogicEditor
        value={expression}
        data={debug ? data : undefined}
      />
    </div>
  );
}
```

## Next Steps

- [Props & API](props-api.md) - Complete props reference
- [Customization](customization.md) - Theming and styling
