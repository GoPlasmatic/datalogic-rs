# Quick Start

The examples below cover common patterns for the DataLogicEditor component.

## Basic Visualization

Render a JSONLogic expression as a flow diagram:

```tsx
import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor, type JsonLogicValue } from '@goplasmatic/datalogic-ui';

function App() {
  // Annotate the literal: TypeScript widens an array holding two different
  // operator keys into a union that JsonLogicValue does not accept.
  const expression: JsonLogicValue = {
    and: [
      { '>': [{ var: 'age' }, 18] },
      { '==': [{ var: 'status' }, 'active'] },
    ],
  };

  return (
    <div style={{ width: '100%', height: '500px' }}>
      <DataLogicEditor value={expression} />
    </div>
  );
}
```

## Debugging

Provide a `data` context to turn on the debugger. The toolbar gains play/pause, step, and a step timeline over the engine's execution trace:

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

As you step, the current node shows its context and result in a bubble, and executed nodes stay highlighted so the taken path is visible. If evaluation fails, the editor marks the node on the engine's failure breadcrumb with the error (and a rule that does not compile at all reports the error in a banner above the diagram). Nodes do not display results at rest, so step through the trace to read values.

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
      { ">": [{ "var": "price" }, 20] }
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
import { DataLogicEditor, type JsonLogicValue } from '@goplasmatic/datalogic-ui';

function EditableExample() {
  // `onChange` hands back `JsonLogicValue | null`, so type the state to match.
  const [expression, setExpression] = useState<JsonLogicValue | null>({
    '>': [{ var: 'cart.total' }, 100],
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

Add `data` to combine editing with live debugging in the same view. In edit
mode the toolbar also gains an **Insert** button (Cmd/Ctrl+K) that adds an
argument to the selection or wraps the root, and the canvas supports
copy/paste (Cmd/Ctrl+C / V), duplicate (Cmd/Ctrl+D), select-all (Cmd/Ctrl+A),
undo/redo (Cmd/Ctrl+Z, Shift+Cmd/Ctrl+Z) and delete (Backspace/Delete).

## Engine Settings

Pass `config` to change how the engine evaluates, for both the result and the
trace. Every key is optional and omitted keys keep the engine default:

```tsx
<DataLogicEditor
  value={expression}
  data={data}
  config={{
    preset: 'strict',
    division_by_zero: 'return_null',
    truthy_evaluator: 'python',
  }}
/>
```

When the settings differ from the defaults, the toolbar shows a compact
summary so you can trace a surprising result to the configuration. Changing
`config` rebuilds the engine, which resets selection and undo history.

## Custom Operators

Register your own operators; rules that use them evaluate and trace like any
other. Arguments arrive already evaluated, and the return value can be any
JSON-serializable value (`undefined` becomes `null`; a thrown exception
becomes a runtime evaluation error):

```tsx
<DataLogicEditor
  value={{ discounted: [{ var: 'price' }] }}
  data={{ price: 100 }}
  customOperators={{
    discounted: (args) => Number(args[0]) * 0.9,
  }}
/>
```

The palette and help panel only know the built-in operators, so custom nodes
render with the generic "utility" styling.

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

The component sets `data-theme` on its own `.logic-editor` root and does not read a `data-theme` on a parent or ancestor. Use the `theme` prop to force a theme.

## Handling Null/Empty Expressions

The editor handles null or undefined expressions:

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
