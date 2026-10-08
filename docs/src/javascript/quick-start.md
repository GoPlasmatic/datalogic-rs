# Quick Start

The core patterns for using JSONLogic from JavaScript/TypeScript.

## Basic Evaluation

Build an `Engine` once, then evaluate:

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';

// Initialize WASM (browser/bundler only; on Node the default import is not
// a function, so guard the call or import from '@goplasmatic/datalogic-wasm/nodejs')
if (typeof init === 'function') await init();

const engine = new Engine();

// Evaluate a simple expression
const result = engine.evalStr('{"==": [1, 1]}', '{}');
console.log(result); // "true"
```

## Working with Data

Pass data as a JSON string for variable resolution:

```javascript
// Access nested data
const logic = '{"var": "user.age"}';
const data = '{"user": {"age": 25}}';
const result = engine.evalStr(logic, data);
console.log(result); // "25"

// Multiple variables
const priceLogic = '{"*": [{"var": "price"}, {"var": "quantity"}]}';
const orderData = '{"price": 10.99, "quantity": 3}';
console.log(engine.evalStr(priceLogic, orderData)); // "32.97"
```

## Compiled Rules

For repeated evaluation of the same logic, compile it once with `engine.compile` instead of passing it to `evalStr` on every call:

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';

if (typeof init === 'function') await init();

const engine = new Engine();

// Compile once
const rule = engine.compile('{">=": [{"var": "age"}, 18]}');

// Evaluate many times with different data
console.log(rule.evaluate('{"age": 21}')); // "true"
console.log(rule.evaluate('{"age": 16}')); // "false"
console.log(rule.evaluate('{"age": 18}')); // "true"
```

## Parsing Results

Every call returns its result as a JSON string. Parse it for use in your application:

```javascript
const result = engine.evalStr('{"+": [1, 2, 3]}', '{}');
const value = JSON.parse(result); // 6 (number)

// For complex results
const arrayResult = engine.evalStr('{"map": [[1,2,3], {"+": [{"var": ""}, 10]}]}', '{}');
const array = JSON.parse(arrayResult); // [11, 12, 13]
```

## Conditional Logic

Use `if` for branching:

```javascript
const gradeLogic = JSON.stringify({
  "if": [
    { ">=": [{ "var": "score" }, 90] }, "A",
    { ">=": [{ "var": "score" }, 80] }, "B",
    { ">=": [{ "var": "score" }, 70] }, "C",
    { ">=": [{ "var": "score" }, 60] }, "D",
    "F"
  ]
});

const rule = engine.compile(gradeLogic);
console.log(JSON.parse(rule.evaluate('{"score": 85}'))); // "B"
console.log(JSON.parse(rule.evaluate('{"score": 42}'))); // "F"
```

## Array Operations

Process arrays with map, filter, and reduce:

```javascript
// Filter items
const filterLogic = JSON.stringify({
  "filter": [
    { "var": "items" },
    { ">": [{ "var": "price" }, 20] }
  ]
});

const data = JSON.stringify({
  items: [
    { name: "Book", price: 15 },
    { name: "Phone", price: 299 },
    { name: "Pen", price: 5 },
    { name: "Headphones", price: 50 }
  ]
});

const result = JSON.parse(engine.evalStr(filterLogic, data));
// [{ name: "Phone", price: 299 }, { name: "Headphones", price: 50 }]
```

## Templating Mode

Compile a multi-key object with `compileTemplate` to use it as a JSON template:

```javascript
const template = JSON.stringify({
  "user": {
    "fullName": { "cat": [{ "var": "firstName" }, " ", { "var": "lastName" }] },
    "isAdult": { ">=": [{ "var": "age" }, 18] }
  },
  "timestamp": { "now": [] }
});

const data = JSON.stringify({
  firstName: "Alice",
  lastName: "Smith",
  age: 25
});

// compileTemplate: multi-key objects become output templates for this rule
const result = JSON.parse(engine.compileTemplate(template).evaluate(data));
// {
//   "user": { "fullName": "Alice Smith", "isAdult": true },
//   "timestamp": "2024-01-15T10:30:00Z"   (the current time, ISO 8601 UTC)
// }
```

To make templating the default for every `compile`, build the engine with `new Engine({ templating: true })`.

## Error Handling

Wrap evaluations in try-catch. Failures throw a real `Error` whose `name` is a stable tag (`"ParseError"`, `"InvalidOperator"`, `"Thrown"`, ...); see [Error Handling](api-reference.md#error-handling) for the full shape:

```javascript
try {
  const result = engine.evalStr('{"invalid": "json', '{}');
} catch (error) {
  console.error('Evaluation failed:', error.name, error.message);
  // Evaluation failed: ParseError Parse error: json parse error at byte 17: unexpected end of input
}
```

To catch mistakes such as a misspelled operator before a rule runs, compile it with `engine.compileChecked(logic)`, which throws a `CompileError` listing every problem, or call `engine.check(logic)` for the list itself; see [`check`](api-reference.md#checklogic-string-mode-string-string).

## Debugging

Use `engine.evaluateWithTrace` for step-by-step debugging:

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';

if (typeof init === 'function') await init();

const engine = new Engine();
const trace = engine.evaluateWithTrace(
  '{"and": [{"var": "a"}, {"var": "b"}]}',
  '{"a": true, "b": false}'
);

const traceData = JSON.parse(trace);
console.log('Result:', traceData.result);      // Result: false
console.log('Steps:', traceData.steps.length); // Steps: 3 (var a, var b, and)
```

Each step records the node id, the data context it saw, and its result (or error). Literal operands do not record steps.

## Next Steps

- [API Reference](api-reference.md) - Complete function documentation
- [Framework Integration](frameworks.md) - React, Vue, and bundler setup
