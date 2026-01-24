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

## Debug Mode

Add evaluation results by providing data context:

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
        mode="debug"
      />
    </div>
  );
}
```

In debug mode, each node displays its evaluated result, making it easy to trace how the final value was computed.

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
          mode="debug"
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
        mode="debug"
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
        mode="debug"
      />
    </div>
  );
}
```

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

// Or set data-theme on a parent element
<div data-theme="dark">
  <DataLogicEditor value={expression} />
</div>
```

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
