# Usage Modes

The DataLogicEditor has no `mode` enum. Its behavior is driven entirely by which props you pass. The same component is a read-only viewer, a live debugger, a visual editor, or any combination of those, depending on `data`, `editable`, and `templating`.

## Behavior Overview

| Behavior | Enabled by | Description | Requires `data` |
|----------|------------|-------------|-----------------|
| Read-only | (none) | Static diagram visualization | No |
| Debugger | `data` | Diagram with per-node evaluation results and a step-through trace | Yes |
| Editing | `editable` | Visual builder: node selection, properties panel, context menus, undo/redo | No |
| Templating | `templating` | Multi-key objects and arrays become output-shaping templates | No |

These are not mutually exclusive. Setting `editable` and providing `data` at the same time gives you live debugging while you edit.

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
- Color-coded operator categories

## Debugging

Provide a `data` prop and the editor overlays evaluation results on each node, showing how the expression evaluates against the data, and exposes debugger controls for stepping through the execution trace.

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
- Evaluation results displayed on each node
- Step-by-step execution visibility via debugger controls
- Context values shown for variable nodes
- Highlighted execution path

Internally, when `data` is provided the component uses the WASM `evaluateWithTrace` API to capture the result of each sub-expression, the order of evaluation, context values at each step, and the final computed result.

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
- Properties panel for the selected node
- Context menus (right-click a node or the canvas)
- Undo/redo

When `editable` is set, `onChange` is active: edits are debounced (about 300ms) and the rebuilt JSONLogic expression is passed back so you can keep your own state in sync.

## Editing with Live Debugging

Combine `editable` with `data` to edit and debug in the same view: each node shows its evaluated result while you build the expression.

```tsx
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  data={contextData}
  editable
/>
```

## Templating

Set `templating` so that multi-key objects and arrays in the compiled rule become output-shaping templates with embedded JSONLogic, rather than being rejected as invalid JSONLogic. This matches the v5 core API (`Engine::builder().with_templating(true)`). The toolbar also surfaces a templating checkbox; wire `onTemplatingChange` to keep your state in sync.

```tsx
<DataLogicEditor
  value={expression}
  templating={templating}
  onTemplatingChange={setTemplating}
/>
```

## Behavior Comparison

| Aspect | Read-only | Debugger (`data`) | Editing (`editable`) |
|--------|-----------|-------------------|----------------------|
| Node display | Structure only | Structure + values | Editable nodes |
| Interactivity | Pan/zoom | Pan/zoom + inspection | Full editing |
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
        {debug ? 'Hide results' : 'Show results'}
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
