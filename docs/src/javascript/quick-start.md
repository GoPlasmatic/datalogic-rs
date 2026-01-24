# Quick Start

This guide covers the essential patterns for using JSONLogic in JavaScript/TypeScript.

## Basic Evaluation

The simplest way to evaluate JSONLogic:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic';

// Initialize WASM (required for browser/bundler)
await init();

// Evaluate a simple expression
const result = evaluate('{"==": [1, 1]}', '{}', false);
console.log(result); // "true"
```

## Working with Data

Pass data as a JSON string for variable resolution:

```javascript
// Access nested data
const logic = '{"var": "user.age"}';
const data = '{"user": {"age": 25}}';
const result = evaluate(logic, data, false);
console.log(result); // "25"

// Multiple variables
const priceLogic = '{"*": [{"var": "price"}, {"var": "quantity"}]}';
const orderData = '{"price": 10.99, "quantity": 3}';
console.log(evaluate(priceLogic, orderData, false)); // "32.97"
```

## Compiled Rules

For repeated evaluation of the same logic, use `CompiledRule` for better performance:

```javascript
import init, { CompiledRule } from '@goplasmatic/datalogic';

await init();

// Compile once
const rule = new CompiledRule('{">=": [{"var": "age"}, 18]}', false);

// Evaluate many times with different data
console.log(rule.evaluate('{"age": 21}')); // "true"
console.log(rule.evaluate('{"age": 16}')); // "false"
console.log(rule.evaluate('{"age": 18}')); // "true"
```

## Parsing Results

Results are returned as JSON strings. Parse them for use in your application:

```javascript
const result = evaluate('{"+": [1, 2, 3]}', '{}', false);
const value = JSON.parse(result); // 6 (number)

// For complex results
const arrayResult = evaluate('{"map": [[1,2,3], {"+": [{"var": ""}, 10]}]}', '{}', false);
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

const rule = new CompiledRule(gradeLogic, false);
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
    { ">": [{ "var": ".price" }, 20] }
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

const result = JSON.parse(evaluate(filterLogic, data, false));
// [{ name: "Phone", price: 299 }, { name: "Headphones", price: 50 }]
```

## Templating Mode

Enable `preserve_structure` for JSON templating:

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

// Third parameter = true enables structure preservation
const result = JSON.parse(evaluate(template, data, true));
// {
//   "user": { "fullName": "Alice Smith", "isAdult": true },
//   "timestamp": "2024-01-15T10:30:00Z"
// }
```

## Error Handling

Wrap evaluations in try-catch:

```javascript
try {
  const result = evaluate('{"invalid": "json', '{}', false);
} catch (error) {
  console.error('Evaluation failed:', error);
}
```

## Debugging

Use `evaluate_with_trace` for step-by-step debugging:

```javascript
import init, { evaluate_with_trace } from '@goplasmatic/datalogic';

await init();

const trace = evaluate_with_trace(
  '{"and": [{"var": "a"}, {"var": "b"}]}',
  '{"a": true, "b": false}',
  false
);

const traceData = JSON.parse(trace);
console.log('Result:', traceData.result);
console.log('Steps:', traceData.steps);
```

## Next Steps

- [API Reference](api-reference.md) - Complete function documentation
- [Framework Integration](frameworks.md) - React, Vue, and bundler setup
