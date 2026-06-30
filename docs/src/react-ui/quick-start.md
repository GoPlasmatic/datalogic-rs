# Quick Start

This guide covers essential patterns for using the DataLogicEditor component.

## Basic Visualization

Render a JSONLogic expression as a flow diagram:

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

  return (
    <div style={{ width: '100%', height: '500px' }}>
      <DataLogicEditor value={expression} />
    </div>
  );
}
```

## Debugging

Add evaluation results by providing a `data` context. When `data` is present, the editor exposes debugger controls with a step-through execution trace:

```tsx
function DebugExample() {
  const expression = {
    "if": [
      { ">=": [{ "var": "score" }, 90] }, "A",
      { ">=": [{ "var": "score" }, 80] }, "B",
      "C"
    ]
  };

  const userData = {
    score: 85
  };

  return (
    <div style={{ width: '100%', height: '500px' }}>
      <DataLogicEditor
        value={expression}
        data={userData}
      />
    </div>
  );
}
```

With `data` provided, each node displays its evaluated result, making it easy to trace how the final value was computed.

## Dynamic Data

Update evaluation results by changing the data:

```tsx
import { useState } from 'react';

function DynamicDebugger() {
  const [score, setScore] = useState(75);

  const expression = {
    "if": [
      { ">=": [{ "var": "score" }, 90] }, "A",
      { ">=": [{ "var": "score" }, 80] }, "B",
      { ">=": [{ "var": "score" }, 70] }, "C",
      "F"
    ]
  };

  return (
    <div>
      <div>
        <label>
          Score:
          <input
            type="range"
            min="0"
            max="100"
            value={score}
            onChange={(e) => setScore(Number(e.target.value))}
          />
          {score}
        </label>
      </div>

      <div style={{ width: '100%', height: '400px' }}>
        <DataLogicEditor
          value={expression}
          data={{ score }}
        />
      </div>
    </div>
  );
}
```

## Complex Expressions

The editor handles complex nested expressions:

```tsx
function ComplexExample() {
  const expression = {
    "and": [
      { "or": [
        { "==": [{ "var": "user.role" }, "admin"] },
        { "==": [{ "var": "user.role" }, "moderator"] }
      ]},
      { ">=": [{ "var": "user.accountAge" }, 30] },
      { "!": [{ "var": "user.banned" }] }
    ]
  };

  const data = {
    user: {
      role: "moderator",
      accountAge: 45,
      banned: false
    }
  };

  return (
    <div style={{ width: '100%', height: '600px' }}>
      <DataLogicEditor
        value={expression}
        data={data}
      />
    </div>
  );
}
```

## Array Operations

Visualize array operations like map, filter, and reduce:

```tsx
function ArrayExample() {
  const expression = {
    "filter": [
      { "var": "items" },
      { ">": [{ "var": ".price" }, 20] }
    ]
  };

  const data = {
    items: [
      { name: "Book", price: 15 },
      { name: "Phone", price: 299 },
      { name: "Pen", price: 5 }
    ]
  };

  return (
    <div style={{ width: '100%', height: '400px' }}>
      <DataLogicEditor
        value={expression}
        data={data}
      />
    </div>
  );
}
```

## Editing

Set `editable` to turn on the visual builder: node selection, a properties panel, context menus, and undo/redo. Pair it with `value` and `onChange` to keep your own state in sync (`onChange` fires debounced, about 300ms, with the rebuilt JSONLogic):

```tsx
import { useState } from 'react';

function EditableExample() {
  const [expression, setExpression] = useState({
    ">": [{ "var": "cart.total" }, 100]
  });

  return (
    <div style={{ width: '100%', height: '600px' }}>
      <DataLogicEditor
        value={expression}
        onChange={setExpression}
        editable
      />
    </div>
  );
}
```

Add `data` to combine editing with live debugging in the same view.

## Theme Support

The editor supports light and dark themes:

```tsx
// Explicit theme
<DataLogicEditor
  value={expression}
  theme="dark"
/>

// System preference (default)
<DataLogicEditor value={expression} />
```

The component sets `data-theme` on its own `.logic-editor` root, so a `data-theme` on a parent or ancestor is not read. Use the `theme` prop to force a theme.

## Handling Null/Empty Expressions

The editor gracefully handles null or undefined expressions:

```tsx
function ConditionalEditor({ expression }) {
  return (
    <div style={{ width: '100%', height: '400px' }}>
      <DataLogicEditor
        value={expression}  // Can be null
      />
    </div>
  );
}
```

## Styling Container

Add custom styling to the container:

```tsx
<DataLogicEditor
  value={expression}
  className="my-custom-editor"
/>

// CSS
.my-custom-editor {
  border: 1px solid #ccc;
  border-radius: 8px;
}
```

## Next Steps

- [Modes](modes.md) - Detailed mode documentation
- [Props & API](props-api.md) - Complete props reference
- [Customization](customization.md) - Theming and styling
