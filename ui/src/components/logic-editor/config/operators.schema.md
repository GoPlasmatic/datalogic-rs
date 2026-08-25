# Operator Configuration Schema

The operator registry under `ui/src/components/logic-editor/config/` is the
single source of truth for operator documentation, arity validation and UI
rendering in the visual editor. This document describes the TypeScript
types in `operators.types.ts` and the conventions the registry follows. The
types file is authoritative; if the two disagree, fix this document.

## File layout

```
ui/src/components/logic-editor/config/
├── operators.types.ts      # All interfaces described below
├── operators/
│   ├── index.ts            # `operators` map + getOperator / isOperator / search helpers
│   ├── variable.ts         # var, val, exists
│   ├── comparison.ts       # ==, ===, !=, !==, >, >=, <, <=
│   ├── logical.ts          # !, !!, and, or
│   ├── arithmetic.ts       # aggregates arithmetic-basic.ts (+ - * / %) and
│   │                       #   arithmetic-functions.ts (max min abs ceil floor)
│   ├── control.ts          # if, ?:, switch, match, ??
│   ├── string.ts           # aggregates string-core.ts (cat substr in) and
│   │                       #   string-transform.ts (length starts_with ends_with upper lower trim split)
│   ├── array.ts            # aggregates array-iteration.ts (map filter reduce all some none) and
│   │                       #   array-manipulation.ts (merge sort slice group_by distinct)
│   ├── object.ts           # keys, values, entries
│   ├── datetime.ts         # datetime, timestamp, parse_date, format_date, date_diff, now
│   ├── validation.ts       # missing, missing_some
│   ├── error.ts            # try, throw
│   ├── utility.ts          # type
│   └── flagd.ts            # fractional, sem_ver
├── categories.ts           # CategoryMeta per category (colour, icon, docs page)
├── docs.ts                 # Per-operator documentation URL (page + mdBook anchor)
├── arity.ts                # formatArity(): the "Args: ..." badge text
├── literalPanel.ts         # Panel config for literal nodes (and the structure-node variant)
└── __tests__/
    ├── examples.test.ts    # Every help example is evaluated by the WASM engine
    └── registry.test.ts    # Registry == engine builtin names, docs links, arity text
```

The registry covers exactly the engine's `builtinOperatorNames()`: 64
canonical operators plus the aliases `var` (val), `?:` (if) and `match`
(switch). Each alias has its own entry so the picker and help panel work
for either spelling; alias entries say so in their notes and list the
canonical operator in `seeAlso`.

## Root structure

```typescript
interface OperatorConfig {
  version: string;
  operators: Record<string, Operator>;
}
```

`operators/index.ts` exports the flat `operators` map directly (there is no
versioned wrapper object in use today) together with these helpers:

```typescript
getOperator(name): Operator | undefined
getOperatorsByCategory(category): Operator[]
isOperator(name): boolean
getOperatorsGroupedByCategory(): Map<OperatorCategory, Operator[]>
searchOperators(query): Operator[]
```

## Operator

```typescript
interface Operator {
  name: string;               // Engine operator key ("+", "var", "map", ...)
  label: string;              // Display label ("Add", "Variable", "Map")
  category: OperatorCategory; // Grouping, colour and docs page
  description: string;        // One-liner shown in pickers
  arity: AritySpec;           // Argument specification (drives the panel)
  help: OperatorHelp;         // Help panel content
  ui?: OperatorUIHints;       // Rendering hints
  panel?: PanelConfig;        // Properties-panel layout (see Panel configuration)
}
```

The map key must equal `name` (checked by `registry.test.ts`).

### Categories

```typescript
type OperatorCategory =
  | 'variable'     // var, val, exists
  | 'comparison'   // ==, ===, !=, !==, >, >=, <, <=
  | 'logical'      // !, !!, and, or
  | 'arithmetic'   // +, -, *, /, %, max, min, abs, ceil, floor
  | 'control'      // if, ?:, switch, match, ??
  | 'string'       // cat, substr, in, length, starts_with, ends_with, upper, lower, trim, split
  | 'array'        // map, filter, reduce, all, some, none, merge, sort, slice, group_by, distinct
  | 'object'       // keys, values, entries
  | 'datetime'     // datetime, timestamp, parse_date, format_date, date_diff, now
  | 'validation'   // missing, missing_some
  | 'error'        // try, throw
  | 'utility'      // type
  | 'flagd';       // fractional, sem_ver
```

`types/jsonlogic.ts` derives `NodeCategory = OperatorCategory | 'literal'`
from this union, so adding a category also requires entries in
`categories.ts` and `constants/colors.ts` (`CATEGORY_COLORS`); the compiler
enforces both.

```typescript
interface CategoryMeta {
  name: OperatorCategory;
  label: string;
  description: string;
  color: string;      // Hex colour used for nodes and badges
  icon: IconName;     // Must be registered in utils/Icon.tsx (no runtime fallback)
  docsPage: string;   // Slug under https://goplasmatic.github.io/datalogic-rs/operators/
}
```

## Arity

```typescript
type ArityType =
  | 'nullary'    // 0 args (now)
  | 'unary'      // 1 arg (!, upper)
  | 'binary'     // 2 args (in, starts_with)
  | 'ternary'    // 3 args (date_diff, sem_ver)
  | 'nary'       // min+ args, min defaults to 1 (+, cat, ??)
  | 'variadic'   // min+ args, min defaults to 2
  | 'chainable'  // 2+ args compared pairwise (<, ==)
  | 'range'      // min..max args (substr 1-3, sort 1-3, slice 1-4, throw 0-1)
  | 'special';   // Structured argument list (if, val, switch, fractional)

interface AritySpec {
  type: ArityType;
  min?: number;      // Explicit minimum; wins over the nominal count of `type`
  max?: number;      // Explicit maximum (undefined = unlimited)
  args?: ArgSpec[];  // Named argument slots, in order
}

interface ArgSpec {
  name: string;
  label: string;
  description?: string;
  type?: ArgType;
  required?: boolean;   // default true
  repeatable?: boolean; // the slot may repeat (nary / variadic tails)
}

type ArgType =
  | 'any' | 'number' | 'string' | 'boolean' | 'array' | 'object'
  | 'expression'   // A JSONLogic expression evaluated per element (map, sort key)
  | 'path'         // A data path (var / val / exists)
  | 'datetime' | 'duration';
```

The properties panel derives its behaviour from `arity`: `nary`, `variadic`,
`chainable`, `range` and `special` allow adding/removing arguments (bounded by
`min` / `max`); the fixed types do not. `formatArity()` in `arity.ts` renders
the badge text and always honours explicit `min` / `max`, so `throw`
(`unary`, 0-1) reads "Args: 0-1".

## Help content

```typescript
interface OperatorHelp {
  summary: string;            // One line, always visible
  details?: string;           // Longer explanation
  returnType: ReturnType;
  examples: OperatorExample[];
  notes?: string[];           // Gotchas and edge cases
  seeAlso?: string[];         // Related operator names (must exist in the registry)
}

type ReturnType =
  | 'any' | 'number' | 'string' | 'boolean' | 'array' | 'object' | 'null'
  | 'datetime' | 'duration'
  | 'number | string'
  | 'same'      // Same shape as the input (slice: array or string)
  | 'never';    // Always throws (throw)

interface OperatorExample {
  title: string;
  rule: unknown;            // JSONLogic expression
  data?: unknown;           // Input data (null when omitted)
  result?: unknown;         // Expected engine result
  error?: { type: string }; // Expected structured error type instead of a result
  note?: string;
  templating?: boolean;     // Evaluate in templating mode
}
```

`returnType` also drives node colouring (`utils/signal.ts`): boolean is the
resting boolean signal, `array` / `object` / `same` the collection signal,
`datetime` / `duration` the temporal signal.

### Example rules

`__tests__/examples.test.ts` evaluates every example with the vendored WASM
engine and asserts:

- with `error` set: evaluation throws and the error `type` matches
  (`Thrown`, `InvalidArguments`, `InvalidOperator`, ...);
- with `result` set: the parsed result deep-equals `result`;
- with neither: evaluation succeeds and `note` explains why there is no
  fixed result (only `now` uses this).

Because the examples are executed, they must be written the way the engine
reads them:

- A bare array is the argument list. Wrap array literals:
  `{"length": [[1, 2, 3]]}`, `{"type": [[1, 2, 3]]}`.
- Multi-key object literals only parse in templating mode. Either read the
  object from `data` (`{"throw": {"var": "err"}}`) or set `templating: true`.
- Iteration metadata is only reachable through the scope form
  `{"val": [[1], "index"]}` / `{"val": [[1], "key"]}`. The string form
  `{"val": "index"}` is a plain key lookup and returns null.
- Missing variables return null; they are not errors, so `try` does not
  catch them.
- `sort` is `[array, ascending?, keyExpression?]`; `slice` is
  `[value, start?, end?, step?]`; `??` is variadic.

## UI hints

```typescript
type NodeType =
  | 'operator'   // Standard operator node
  | 'variable'   // var / val / exists
  | 'literal'    // Literal value
  | 'decision'   // if / ?: / switch / try
  | 'vertical'   // Vertical cell layout (comparison, and / or)
  | 'iterator'   // map / filter / reduce / all / some / none
  | 'structure'; // Array / object structure

interface OperatorUIHints {
  icon?: IconName;           // Help-header icon; falls back to the category icon
  shortLabel?: string;       // Compact label on the node ("+", "map")
  nodeType?: NodeType;
  inlineEditable?: boolean;
  showArgLabels?: boolean;
  collapsible?: boolean;
  scopeJump?: boolean;       // val: supports [[N], ...] scope jumps
  metadata?: boolean;        // val: supports index / key metadata
  iteratorContext?: boolean; // Creates an iteration frame (map, filter, ...)
  addArgumentLabel?: string; // Label for the add-argument button ("Add Else If")
}
```

`icon` is an `IconName` (see `utils/icons.ts`), so a name that has no
registered component is a compile error rather than a canvas crash.

## Panel configuration

```typescript
type PanelInputType =
  | 'text' | 'textarea' | 'number' | 'boolean' | 'select'
  | 'path' | 'pathArray' | 'expression' | 'json';

interface VisibilityCondition {
  field: string;
  operator: 'equals' | 'notEquals' | 'exists' | 'notExists';
  value?: unknown;
}

interface SelectOption {
  value: string | number | boolean;
  label: string;
  description?: string;
}

interface PanelField {
  id: string;
  label: string;
  inputType: PanelInputType;
  helpText?: string;
  placeholder?: string;
  required?: boolean;
  defaultValue?: unknown;
  options?: SelectOption[];        // for 'select'
  showWhen?: VisibilityCondition[];
  min?: number;                    // for 'number'
  max?: number;
  repeatable?: boolean;
}

interface PanelSection {
  id: string;
  title?: string;
  fields: PanelField[];
  defaultCollapsed?: boolean;
  showWhen?: VisibilityCondition[];
}

interface ContextVariable {          // Variables an iterator body can read
  name: string;
  label: string;
  description: string;
  accessor: 'var' | 'val';
  example: string;                   // e.g. '{"val": [[1], "index"]}'
}

interface PanelConfig {
  sections: PanelSection[];
  contextVariables?: ContextVariable[];
  chainable?: boolean;
}
```

`literalPanel.ts` exports `literalPanelConfig` (string / number / boolean /
null / array) for literal nodes and `structurePanelConfig`, which adds the
object type and its template mode, for structure nodes.

## Documentation links

`docs.ts` builds `https://goplasmatic.github.io/datalogic-rs/operators/<docsPage>.html#<anchor>`
for every operator. The anchor is the operator's `## heading` normalised the
way mdBook does it (lowercase, spaces to `-`, other symbols dropped), so
word-named operators anchor as themselves (`#starts_with`, `#keys`) while
symbolic ones go through their heading text (`+ (Add)` becomes `#-add`,
`?? (Null Coalesce)` becomes `#-null-coalesce`). The `type` operator lives on
the control-flow page, which is why the `utility` category maps there.

## Example entries

### Unary operator

```typescript
'!': {
  name: '!',
  label: 'Not',
  category: 'logical',
  description: 'Logical NOT - negates a boolean value',
  arity: {
    type: 'unary',
    min: 1,
    max: 1,
    args: [{ name: 'value', label: 'Value', type: 'any', required: true }],
  },
  help: {
    summary: 'Negates a boolean value',
    returnType: 'boolean',
    examples: [
      { title: 'Negate true', rule: { '!': [true] }, result: false },
      { title: 'Negate empty array', rule: { '!': [[]] }, result: true },
    ],
    notes: ['Falsy values: false, null, 0, "" (empty string), [] and {}'],
    seeAlso: ['!!', 'and', 'or'],
  },
  ui: { icon: 'ban', shortLabel: '!', nodeType: 'operator' },
}
```

### Variable operator with scope jumps

```typescript
val: {
  name: 'val',
  label: 'Value',
  category: 'variable',
  description: 'Access data using array path with scope jump support',
  arity: {
    type: 'special',
    min: 1,
    args: [{ name: 'path', label: 'Path', type: 'path', required: true }],
  },
  help: {
    summary: 'Access data using array path components with scope jump and metadata support',
    returnType: 'any',
    examples: [
      {
        title: 'Array path',
        rule: { val: ['user', 'profile', 'name'] },
        data: { user: { profile: { name: 'Alice' } } },
        result: 'Alice',
      },
      {
        title: 'Get iteration index',
        rule: { map: [['a', 'b', 'c'], { val: [[1], 'index'] }] },
        result: [0, 1, 2],
      },
    ],
    notes: ['No default argument: use var\'s second argument or ?? for fallbacks'],
    seeAlso: ['var', 'exists'],
  },
  ui: { icon: 'database', nodeType: 'variable', scopeJump: true, metadata: true },
}
```

### Iterator with context variables

```typescript
map: {
  name: 'map',
  label: 'Map',
  category: 'array',
  description: 'Transform each element of an array',
  arity: {
    type: 'binary',
    min: 2,
    max: 2,
    args: [
      { name: 'array', label: 'Array', type: 'array', required: true },
      { name: 'expression', label: 'Expression', type: 'expression', required: true },
    ],
  },
  help: {
    summary: 'Apply an expression to each element of an array',
    returnType: 'array',
    examples: [
      {
        title: 'With index',
        rule: { map: [{ var: 'items' }, { cat: ['Item ', { val: [[1], 'index'] }] }] },
        data: { items: ['a', 'b'] },
        result: ['Item 0', 'Item 1'],
      },
    ],
    seeAlso: ['filter', 'reduce'],
  },
  ui: { icon: 'repeat', nodeType: 'iterator', iteratorContext: true },
  panel: {
    sections: [/* array + expression fields */],
    contextVariables: [
      { name: '', label: 'Current Element', accessor: 'var', example: '{"var": ""}', description: '...' },
      { name: 'index', label: 'Index', accessor: 'val', example: '{"val": [[1], "index"]}', description: '...' },
    ],
  },
}
```

## Adding an operator

1. Add the entry to the matching `operators/<category>.ts` module (or a new
   module spread into `operators/index.ts`).
2. Verify every example against the engine (`cargo run` the CLI or rely on
   `examples.test.ts`) and write the results exactly as the engine returns
   them.
3. Run `npx vitest run src/components/logic-editor/config` and `npx tsc -b`.
   The registry test fails until the UI set equals the engine's
   `builtinOperatorNames()`.
