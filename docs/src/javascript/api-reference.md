# API Reference

Complete API documentation for the `@goplasmatic/datalogic` WebAssembly package.

## Functions

### `init()`

Initialize the WebAssembly module. Required before using any other functions in browser/bundler environments.

```typescript
function init(input?: InitInput): Promise<InitOutput>;
```

**Parameters:**
- `input` (optional) - Custom WASM source (URL, Response, or BufferSource)

**Returns:** Promise that resolves when initialization is complete

**Example:**
```javascript
import init from '@goplasmatic/datalogic';

// Standard initialization
await init();

// Custom WASM location
await init('/custom/path/datalogic_wasm_bg.wasm');
```

> **Note:** Node.js does not require initialization.

---

### `evaluate()`

Evaluate a JSONLogic expression against data.

```typescript
function evaluate(logic: string, data: string, preserve_structure: boolean): string;
```

**Parameters:**
- `logic` - JSON string containing the JSONLogic expression
- `data` - JSON string containing the data context
- `preserve_structure` - Enable templating mode (preserves object structure)

**Returns:** JSON string containing the result

**Throws:** String error message if evaluation fails

**Examples:**
```javascript
// Simple comparison
evaluate('{"==": [1, 1]}', '{}', false); // "true"

// Variable access
evaluate('{"var": "name"}', '{"name": "Alice"}', false); // "\"Alice\""

// Arithmetic
evaluate('{"+": [1, 2, 3]}', '{}', false); // "6"

// Array operations
evaluate('{"map": [[1,2,3], {"+": [{"var": ""}, 1]}]}', '{}', false); // "[2,3,4]"

// Templating mode
evaluate(
  '{"result": {"var": "x"}, "computed": {"+": [1, 2]}}',
  '{"x": 42}',
  true
); // '{"result":42,"computed":3}'
```

---

### `evaluate_with_trace()`

Evaluate with detailed execution trace for debugging.

```typescript
function evaluate_with_trace(logic: string, data: string, preserve_structure: boolean): string;
```

**Parameters:** Same as `evaluate()`

**Returns:** JSON string containing `TracedResult`:

```typescript
interface TracedResult {
  result: any;              // Evaluation result
  expression_tree: {        // Tree structure of the expression
    id: number;
    expression: string;
    children?: ExpressionNode[];
  };
  steps: Step[];            // Execution steps
}

interface Step {
  node_id: number;
  operator: string;
  input_values: any[];
  output_value: any;
  context: any;
}
```

**Example:**
```javascript
const trace = evaluate_with_trace(
  '{"and": [true, {"var": "x"}]}',
  '{"x": false}',
  false
);

const data = JSON.parse(trace);
console.log(data.result);      // false
console.log(data.steps.length); // 3 (and, true literal, var lookup)
```

---

## Classes

### `CompiledRule`

Pre-compiled rule for efficient repeated evaluation.

#### Constructor

```typescript
new CompiledRule(logic: string, preserve_structure: boolean)
```

**Parameters:**
- `logic` - JSON string containing the JSONLogic expression
- `preserve_structure` - Enable templating mode

**Throws:** If the logic is invalid JSON or contains compilation errors

**Example:**
```javascript
const rule = new CompiledRule('{">=": [{"var": "age"}, 18]}', false);
```

#### Methods

##### `evaluate(data: string): string`

Evaluate the compiled rule against data.

```typescript
evaluate(data: string): string;
```

**Parameters:**
- `data` - JSON string containing the data context

**Returns:** JSON string containing the result

**Example:**
```javascript
const rule = new CompiledRule('{"+": [{"var": "a"}, {"var": "b"}]}', false);

rule.evaluate('{"a": 1, "b": 2}');  // "3"
rule.evaluate('{"a": 10, "b": 20}'); // "30"
```

##### `evaluate_with_trace(data: string): string`

Evaluate with execution trace.

```typescript
evaluate_with_trace(data: string): string;
```

**Parameters:**
- `data` - JSON string containing the data context

**Returns:** JSON string containing `TracedResult`

**Example:**
```javascript
const rule = new CompiledRule('{"if": [{"var": "x"}, "yes", "no"]}', false);
const trace = JSON.parse(rule.evaluate_with_trace('{"x": true}'));
```

---

## Type Definitions

### Input/Output Types

All functions accept and return JSON strings. Parse results for use:

```typescript
// Input: Always JSON strings
const logic: string = JSON.stringify({ "==": [1, 1] });
const data: string = JSON.stringify({ x: 42 });

// Output: Always JSON strings
const result: string = evaluate(logic, data, false);
const parsed: boolean = JSON.parse(result); // true
```

### Preserve Structure Mode

When `preserve_structure` is `true`:
- Unknown object keys become output fields
- Only recognized operators are evaluated
- Useful for JSON templating

```javascript
// Without preserve_structure - "result" treated as unknown operator
evaluate('{"result": {"var": "x"}}', '{"x": 1}', false);
// Error or unexpected behavior

// With preserve_structure - "result" becomes output field
evaluate('{"result": {"var": "x"}}', '{"x": 1}', true);
// '{"result":1}'
```

---

## Error Handling

All functions throw string errors on failure:

```javascript
try {
  evaluate('{"invalid json', '{}', false);
} catch (error) {
  // error is a string describing the problem
  console.error('Failed:', error);
}
```

Common error types:
- JSON parse errors (invalid syntax)
- Unknown operator errors (in non-preserve mode)
- Type errors (wrong argument types)
- Variable access errors (missing required data)

---

## Performance Tips

1. **Use CompiledRule for repeated evaluation:**
   ```javascript
   // Slow: recompiles each time
   for (const user of users) {
     evaluate(logic, JSON.stringify(user), false);
   }

   // Fast: compile once
   const rule = new CompiledRule(logic, false);
   for (const user of users) {
     rule.evaluate(JSON.stringify(user));
   }
   ```

2. **Initialize once at startup:**
   ```javascript
   // Application entry point
   await init();
   // Now use evaluate/CompiledRule anywhere
   ```

3. **Reuse CompiledRule instances:**
   ```javascript
   // Store compiled rules
   const rules = {
     isAdult: new CompiledRule('{">=": [{"var": "age"}, 18]}', false),
     isPremium: new CompiledRule('{"==": [{"var": "tier"}, "premium"]}', false),
   };
   ```
