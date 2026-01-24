# FAQ

Frequently asked questions about datalogic-rs.

## General

### What is JSONLogic?

JSONLogic is a way to write portable, safe logic rules as JSON. It was created to allow non-developers to create complex rules that can be evaluated consistently across different platforms. The specification is available at [jsonlogic.com](https://jsonlogic.com).

### Why use datalogic-rs instead of the reference implementation?

- **Performance:** datalogic-rs is significantly faster than JavaScript implementations
- **Thread Safety:** Compiled rules can be safely shared across threads
- **Extended Operators:** Includes datetime, regex, and additional string/array operators
- **Type Safety:** Full Rust type system benefits
- **WASM Support:** Use the same engine in browsers and Node.js

### Is datalogic-rs fully compatible with JSONLogic?

Yes. datalogic-rs passes the complete official JSONLogic test suite. It also includes additional operators that extend the specification.

---

## Rust Usage

### Should I use v3 or v4?

**Use v4** for most projects. It has a simpler, more ergonomic API.

**Use v3** only if you need maximum performance with arena allocation and are comfortable with lifetime management.

Both versions are maintained and receive bug fixes.

### How do I share compiled rules across threads?

`CompiledLogic` is wrapped in `Arc` and is `Send + Sync`:

```rust
use std::sync::Arc;
use datalogic_rs::DataLogic;

let engine = Arc::new(DataLogic::new());
let compiled = engine.compile(&logic).unwrap();

// Clone the Arc for each thread
let compiled_clone = Arc::clone(&compiled);
std::thread::spawn(move || {
    // Use compiled_clone here
});
```

### Why do custom operators receive unevaluated arguments?

This design allows operators to implement lazy evaluation (like `and` and `or`) and control how arguments are processed. Always call `evaluator.evaluate()` on arguments that should be evaluated:

```rust
impl Operator for MyOperator {
    fn evaluate(&self, args: &[Value], context: &mut ContextStack, evaluator: &dyn Evaluator) -> Result<Value> {
        // Evaluate the first argument
        let value = evaluator.evaluate(&args[0], context)?;
        // ...
    }
}
```

### What's the difference between `evaluate` and `evaluate_owned`?

- `evaluate`: Takes a reference to data, returns `Cow<Value>` (avoids cloning when possible)
- `evaluate_owned`: Takes ownership of data, returns `Value` (simpler API)

Use `evaluate` for performance-critical code with large data. Use `evaluate_owned` for simpler code.

---

## JavaScript/WASM Usage

### Do I need to call `init()` in Node.js?

No. The Node.js target doesn't require initialization:

```javascript
const { evaluate } = require('@goplasmatic/datalogic');
evaluate('{"==": [1, 1]}', '{}', false); // Works immediately
```

### Why do I need to JSON.stringify my data?

The WASM interface uses string-based communication for maximum compatibility. Always stringify inputs and parse outputs:

```javascript
const result = evaluate(
  JSON.stringify(logic),
  JSON.stringify(data),
  false
);
const value = JSON.parse(result);
```

### How do I use this with TypeScript?

Types are included in the package:

```typescript
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic';

await init();
const result: string = evaluate('{"==": [1, 1]}', '{}', false);
```

---

## React UI

### Why does the editor need explicit dimensions?

React Flow (the underlying library) requires a container with defined dimensions to calculate node positions and viewport. Set dimensions via CSS or inline styles:

```tsx
<div style={{ height: '500px' }}>
  <DataLogicEditor value={expression} />
</div>
```

### Can I use this with Next.js?

Yes. For the App Router, wrap in a client component:

```tsx
'use client';

import '@xyflow/react/dist/style.css';
import '@goplasmatic/datalogic-ui/styles.css';
import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

export function Editor({ expression }) {
  return <DataLogicEditor value={expression} />;
}
```

### When will edit mode be available?

Edit mode is on the roadmap. Check the [GitHub issues](https://github.com/GoPlasmatic/datalogic-rs/issues) for updates.

---

## Operators

### How do I access array elements by index?

Use the `var` operator with numeric path segments:

```json
{ "var": "items.0.name" }
```

### What's the difference between `==` and `===`?

- `==`: Loose equality (with type coercion, like JavaScript)
- `===`: Strict equality (no type coercion)

```json
{"==": [1, "1"]}   // true
{"===": [1, "1"]}  // false
```

### How do I handle missing data?

Use the `missing` or `missing_some` operators:

```json
{
  "if": [
    { "missing": ["user.email"] },
    "Email required",
    "Valid"
  ]
}
```

Or use default values with `var`:

```json
{ "var": ["user.email", "no-email@example.com"] }
```

### Can I use regex?

Yes. Use the `match` operator:

```json
{ "match": [{ "var": "email" }, "^[a-z]+@example\\.com$"] }
```

---

## Configuration

### How do I handle NaN in arithmetic?

Use the `NanHandling` configuration:

```rust
use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling};

let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::IgnoreValue);
let engine = DataLogic::with_config(config);
```

Options:
- `ThrowError` (default): Return an error
- `CoerceToZero`: Treat non-numeric as 0
- `IgnoreValue`: Skip non-numeric values

### How do I change division by zero behavior?

```rust
use datalogic_rs::{EvaluationConfig, DivisionByZero};

let config = EvaluationConfig::default()
    .with_division_by_zero(DivisionByZero::ReturnBounds);
```

Options:
- `ReturnBounds` (default): Return Infinity/-Infinity
- `ThrowError`: Return an error
- `ReturnZero`: Return 0

---

## Troubleshooting

### "Unknown operator" error

In standard mode, unrecognized keys are treated as errors. Either:
1. Fix the operator name (check spelling)
2. Register a custom operator
3. Enable `preserve_structure` mode for templating

### Performance issues with large expressions

1. Use `CompiledRule` instead of repeated `evaluate` calls
2. Consider breaking complex rules into smaller, composable pieces
3. Profile with tracing to identify slow sub-expressions

### WASM initialization fails

Ensure you're awaiting `init()` before calling other functions:

```javascript
// Wrong
const result = evaluate(...);

// Correct
await init();
const result = evaluate(...);
```

For more troubleshooting, see the [Troubleshooting Guide](troubleshooting.md).
