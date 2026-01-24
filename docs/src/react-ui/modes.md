# Editor Modes

The DataLogicEditor supports three modes, each providing different levels of functionality.

## Mode Overview

| Mode | API Value | Description | Requires Data |
|------|-----------|-------------|---------------|
| ReadOnly | `'visualize'` | Static diagram visualization | No |
| Debugger | `'debug'` | Diagram with evaluation results | Yes |
| Editor | `'edit'` | Visual builder (coming soon) | Optional |

## Visualize Mode (Default)

The default mode renders a static flow diagram of the JSONLogic expression.

```tsx
<DataLogicEditor
  value={expression}
  mode="visualize"  // Optional, this is the default
/>
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

## Debug Mode

Debug mode adds evaluation results to each node, showing how the expression evaluates against provided data.

```tsx
<DataLogicEditor
  value={expression}
  data={contextData}
  mode="debug"
/>
```

**Use cases:**
- Understanding evaluation flow
- Debugging unexpected results
- Testing expressions with different inputs
- Learning JSONLogic

**Features:**
- All visualization features, plus:
- Evaluation results displayed on each node
- Step-by-step execution visibility
- Context values shown for variable nodes
- Highlighted execution path

### Debug Mode Requirements

Debug mode requires the `data` prop. Without it, the component falls back to visualize mode:

```tsx
// This will work in debug mode
<DataLogicEditor
  value={expression}
  data={{ x: 1 }}
  mode="debug"
/>

// This falls back to visualize mode (no data)
<DataLogicEditor
  value={expression}
  mode="debug"
/>
```

### Tracing Execution

In debug mode, the component uses `evaluate_with_trace` internally to capture:

- The result of each sub-expression
- The order of evaluation
- Context values at each step
- Final computed result

## Edit Mode (Coming Soon)

Edit mode will provide a full visual builder for creating and modifying JSONLogic expressions.

```tsx
// Planned API
<DataLogicEditor
  value={expression}
  onChange={setExpression}
  data={contextData}  // Optional, for live preview
  mode="edit"
/>
```

**Planned features:**
- Drag-and-drop node creation
- Visual connection editing
- Operator palette
- Live evaluation preview
- Undo/redo support
- Expression validation

> **Note:** Using `mode="edit"` currently renders the component in read-only mode. If `data` is provided, it shows debug evaluation. A console warning indicates this limitation.

## Mode Comparison

### Visual Differences

| Aspect | Visualize | Debug | Edit (Planned) |
|--------|-----------|-------|----------------|
| Node display | Structure only | Structure + values | Editable nodes |
| Interactivity | Pan/zoom | Pan/zoom + inspection | Full editing |
| Data required | No | Yes | Optional |
| Output | Static | Static + trace | Two-way bound |

### Performance Considerations

- **Visualize mode** is fastest - no evaluation overhead
- **Debug mode** runs evaluation on every data change
- **Edit mode** will include validation and preview costs

For large expressions or frequent data updates, consider debouncing:

```tsx
import { useMemo } from 'react';
import { useDebouncedValue } from './hooks';

function DebugWithDebounce({ expression, data }) {
  const debouncedData = useDebouncedValue(data, 200);

  return (
    <DataLogicEditor
      value={expression}
      data={debouncedData}
      mode="debug"
    />
  );
}
```

## Switching Modes

You can dynamically switch between modes:

```tsx
function ModeToggle() {
  const [mode, setMode] = useState<'visualize' | 'debug'>('visualize');

  return (
    <div>
      <button onClick={() => setMode('visualize')}>Visualize</button>
      <button onClick={() => setMode('debug')}>Debug</button>

      <DataLogicEditor
        value={expression}
        data={mode === 'debug' ? data : undefined}
        mode={mode}
      />
    </div>
  );
}
```

## Next Steps

- [Props & API](props-api.md) - Complete props reference
- [Customization](customization.md) - Theming and styling
