# Operator Configuration Schema

This document defines the structure for the operator configuration file that serves as the single source of truth for all operator documentation and UI rendering.

## Proposed File Structure

```
ui/src/components/logic-editor/config/
├── operators.json          # Main operator definitions (or operators.ts for type safety)
├── operators.schema.md     # This documentation
└── categories.ts           # Category metadata (colors, icons, labels)
```

## Schema Definition

### Root Structure

```typescript
interface OperatorConfig {
  version: string;                    // Schema version for future migrations
  operators: Record<string, Operator>;
}
```

### Operator Definition

```typescript
interface Operator {
  // === Identity ===
  name: string;                       // Operator key (e.g., "+", "var", "map")
  label: string;                      // Display label (e.g., "Add", "Variable", "Transform Each")
  category: OperatorCategory;         // Category for grouping and styling

  // === Arity ===
  arity: AritySpec;                   // Argument specification

  // === Documentation ===
  description: string;                // One-line description (shown in picker)
  help: OperatorHelp;                 // Detailed help content

  // === UI Hints ===
  ui?: OperatorUIHints;               // Optional UI-specific configuration
}
```

### Category Enum

```typescript
type OperatorCategory =
  | 'variable'      // var, val, exists
  | 'comparison'    // ==, ===, !=, !==, >, >=, <, <=
  | 'logical'       // !, !!, and, or
  | 'arithmetic'    // +, -, *, /, %, max, min, abs, ceil, floor
  | 'control'       // if, ?:, ??
  | 'string'        // cat, substr, in, length, starts_with, ends_with, upper, lower, trim, split
  | 'array'         // merge, filter, map, reduce, all, some, none, sort, slice
  | 'datetime'      // datetime, timestamp, parse_date, format_date, date_diff, now
  | 'validation'    // missing, missing_some
  | 'error'         // try, throw
  | 'utility';      // type, preserve
```

### Arity Specification

```typescript
interface AritySpec {
  type: ArityType;
  min?: number;                       // Minimum arguments (default: 0)
  max?: number;                       // Maximum arguments (undefined = unlimited)
  args?: ArgSpec[];                   // Named argument specifications
}

type ArityType =
  | 'nullary'       // 0 args (e.g., now)
  | 'unary'         // 1 arg (e.g., !, abs)
  | 'binary'        // 2 args (e.g., /, %)
  | 'ternary'       // 3 args (e.g., ?:, reduce)
  | 'nary'          // 1+ args (e.g., +, cat)
  | 'variadic'      // 2+ args (e.g., *, and)
  | 'chainable'     // 2+ args with chaining (e.g., <, >)
  | 'special';      // Custom structure (e.g., if, val)

interface ArgSpec {
  name: string;                       // Argument name (e.g., "left", "right", "array")
  label: string;                      // Display label
  description?: string;               // Tooltip description
  type?: ArgType;                     // Expected type hint
  required?: boolean;                 // Is this argument required? (default: true)
  repeatable?: boolean;               // Can this arg repeat? (for nary operators)
}

type ArgType =
  | 'any'
  | 'number'
  | 'string'
  | 'boolean'
  | 'array'
  | 'object'
  | 'expression'    // A JSONLogic expression
  | 'path'          // A variable path (dot notation or array)
  | 'datetime'
  | 'duration';
```

### Help Content

```typescript
interface OperatorHelp {
  summary: string;                    // One-line summary (always visible)
  details?: string;                   // Extended explanation (markdown supported)
  returnType: ReturnType;             // What type does this operator return
  examples: OperatorExample[];        // Code examples with results
  notes?: string[];                   // Tips, gotchas, edge cases
  seeAlso?: string[];                 // Related operator names
}

type ReturnType =
  | 'any'
  | 'number'
  | 'string'
  | 'boolean'
  | 'array'
  | 'object'
  | 'null'
  | 'datetime'
  | 'duration'
  | 'number | string'                 // For operators like + that can return either
  | 'same';                           // Returns same type as input

interface OperatorExample {
  title: string;                      // Example title
  rule: unknown;                      // JSONLogic expression (will be JSON)
  data?: unknown;                     // Sample input data
  result: unknown;                    // Expected output
  note?: string;                      // Optional note for this example
}
```

### UI Hints (Optional)

```typescript
interface OperatorUIHints {
  // === Display ===
  icon?: string;                      // Lucide icon name (e.g., "plus", "variable")
  shortLabel?: string;                // Very short label for compact display (e.g., "+")

  // === Node Rendering ===
  nodeType?: NodeType;                // How to render this operator

  // === Editor Behavior ===
  inlineEditable?: boolean;           // Can value be edited inline on canvas?
  showArgLabels?: boolean;            // Show argument labels in node?
  collapsible?: boolean;              // Can this node be collapsed?

  // === Special Features ===
  scopeJump?: boolean;                // Does this support scope jumps? (val)
  metadata?: boolean;                 // Does this access metadata? (val)
  datetimeProps?: boolean;            // Does this access datetime properties? (val)
  iteratorContext?: boolean;          // Does this create iterator context? (map, filter, etc.)
}

type NodeType =
  | 'operator'      // Standard operator node
  | 'variable'      // Variable access node (var, val, exists)
  | 'literal'       // Literal value node
  | 'decision'      // If/then/else diamond
  | 'vertical'      // Vertical cell layout (comparisons, logical)
  | 'iterator'      // Array iterator (map, filter, reduce)
  | 'structure';    // Object/array structure
```

## Example Operator Entries

### Simple Unary Operator

```json
{
  "!": {
    "name": "!",
    "label": "Not",
    "category": "logical",
    "description": "Logical NOT - negates a boolean value",
    "arity": {
      "type": "unary",
      "min": 1,
      "max": 1,
      "args": [
        {
          "name": "value",
          "label": "Value",
          "type": "any",
          "description": "Value to negate"
        }
      ]
    },
    "help": {
      "summary": "Negates a boolean value",
      "details": "Returns true if the value is falsy (false, null, 0, \"\"), false otherwise.",
      "returnType": "boolean",
      "examples": [
        {
          "title": "Negate true",
          "rule": {"!": [true]},
          "result": false
        },
        {
          "title": "Negate falsy value",
          "rule": {"!": [0]},
          "result": true
        },
        {
          "title": "With variable",
          "rule": {"!": [{"var": "isActive"}]},
          "data": {"isActive": false},
          "result": true
        }
      ],
      "notes": [
        "Falsy values: false, null, 0, \"\" (empty string)",
        "All other values are considered truthy"
      ],
      "seeAlso": ["!!", "and", "or"]
    },
    "ui": {
      "icon": "circle-slash",
      "shortLabel": "!",
      "nodeType": "operator"
    }
  }
}
```

### Variable Operator (val with special features)

```json
{
  "val": {
    "name": "val",
    "label": "Value",
    "category": "variable",
    "description": "Access data using array path with scope jump support",
    "arity": {
      "type": "special",
      "min": 1,
      "args": [
        {
          "name": "path",
          "label": "Path",
          "type": "path",
          "description": "Array of path components, optionally starting with scope level"
        }
      ]
    },
    "help": {
      "summary": "Access data using array path components with scope jump and metadata support",
      "details": "Retrieves a value using an array of path components. Supports scope jumps for accessing parent contexts in nested iterators. Use [[N], \"field\", ...] to jump up N context levels (sign is ignored).",
      "returnType": "any",
      "examples": [
        {
          "title": "Array path",
          "rule": {"val": ["user", "profile", "name"]},
          "data": {"user": {"profile": {"name": "Alice"}}},
          "result": "Alice"
        },
        {
          "title": "Current element",
          "rule": {"val": []},
          "result": "(current element in iterator)"
        },
        {
          "title": "Parent scope",
          "rule": {"val": [[1], "multiplier"]},
          "result": "(parent context's multiplier)"
        },
        {
          "title": "Get iteration index",
          "rule": {"val": "index"},
          "result": "(current index: 0, 1, 2, ...)"
        },
        {
          "title": "DateTime property",
          "rule": {"val": [{"var": "date"}, "year"]},
          "data": {"date": "2024-01-15"},
          "result": 2024
        }
      ],
      "notes": [
        "Path is array of components: [\"a\", \"b\", \"c\"]",
        "Scope jump: [[N], ...] goes up N context levels",
        "Sign is ignored: [1] and [-1] are equivalent",
        "If level exceeds depth, returns root data",
        "Special metadata: \"index\" and \"key\" for iteration",
        "DateTime props: year, month, day, hour, minute, second, timestamp, iso",
        "Duration props: days, hours, minutes, seconds, total_seconds"
      ],
      "seeAlso": ["var", "exists"]
    },
    "ui": {
      "icon": "brackets",
      "nodeType": "variable",
      "scopeJump": true,
      "metadata": true,
      "datetimeProps": true
    }
  }
}
```

### Iterator Operator (map)

```json
{
  "map": {
    "name": "map",
    "label": "Transform Each",
    "category": "array",
    "description": "Apply an expression to each element of an array",
    "arity": {
      "type": "binary",
      "min": 2,
      "max": 2,
      "args": [
        {
          "name": "array",
          "label": "Array",
          "type": "array",
          "description": "Array to iterate over"
        },
        {
          "name": "expression",
          "label": "Expression",
          "type": "expression",
          "description": "Expression applied to each element"
        }
      ]
    },
    "help": {
      "summary": "Apply an expression to each element of an array",
      "details": "Iterates over an array and applies the given expression to each element. Use {\"var\": \"\"} to access the current element. Use {\"val\": [[1], \"field\"]} to access parent scope.",
      "returnType": "array",
      "examples": [
        {
          "title": "Double each number",
          "rule": {"map": [[1, 2, 3], {"*": [{"var": ""}, 2]}]},
          "result": [2, 4, 6]
        },
        {
          "title": "Extract field",
          "rule": {"map": [{"var": "users"}, {"var": "name"}]},
          "data": {"users": [{"name": "Alice"}, {"name": "Bob"}]},
          "result": ["Alice", "Bob"]
        },
        {
          "title": "With index",
          "rule": {"map": [{"var": "items"}, {"cat": ["Item ", {"val": "index"}]}]},
          "data": {"items": ["a", "b"]},
          "result": ["Item 0", "Item 1"]
        }
      ],
      "notes": [
        "Use {\"var\": \"\"} to access current element",
        "Use {\"val\": \"index\"} to get current index",
        "Use {\"val\": [[1], \"field\"]} to access parent scope",
        "Returns a new array; does not modify the original"
      ],
      "seeAlso": ["filter", "reduce", "all", "some", "none"]
    },
    "ui": {
      "icon": "repeat",
      "nodeType": "iterator",
      "iteratorContext": true
    }
  }
}
```

## File Format Options

### Option A: JSON (operators.json)
- **Pros**: Language-agnostic, can be loaded dynamically, easy to parse
- **Cons**: No type checking, no comments, verbose

### Option B: TypeScript (operators.ts)
- **Pros**: Type safety, IDE support, can include comments, functions for derived values
- **Cons**: Needs compilation, slightly larger bundle

### Option C: Hybrid (operators.config.ts + operators.data.json)
- **Pros**: Types in TS, data in JSON, best of both
- **Cons**: Two files to maintain

## Recommendation

**Option B: TypeScript (operators.ts)** with exported const:

```typescript
// operators.ts
import { Operator, OperatorConfig } from './operators.types';

export const operatorConfig: OperatorConfig = {
  version: '1.0.0',
  operators: {
    '!': { ... },
    '!!': { ... },
    // ... all 59 operators
  }
};

// Helper functions
export function getOperator(name: string): Operator | undefined;
export function getOperatorsByCategory(category: OperatorCategory): Operator[];
export function searchOperators(query: string): Operator[];
```

This provides:
1. Full type safety during development
2. IDE autocompletion for operator names
3. Compile-time validation of the configuration
4. Helper functions for common queries
5. Tree-shaking if needed
