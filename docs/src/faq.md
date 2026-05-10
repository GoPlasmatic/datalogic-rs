# FAQ

Frequently asked questions about datalogic-rs.

## General

### What is JSONLogic?

JSONLogic is a way to write portable, safe logic rules as JSON. The
specification is available at [jsonlogic.com](https://jsonlogic.com).

### Why use datalogic-rs instead of the reference implementation?

- **Performance** — significantly faster than JS implementations
- **Thread Safety** — `Logic` is `Send + Sync`; wrap in `Arc` to share
- **Extended Operators** — datetime, regex, error handling, more
- **Type Safety** — full Rust type system benefits
- **WASM Support** — same engine in browsers and Node.js
- **Zero `unsafe`** — the crate is built with `#![forbid(unsafe_code)]`

### Is datalogic-rs fully compatible with JSONLogic?

Yes. datalogic-rs passes the complete official JSONLogic test suite. It
also includes additional operators that extend the specification.

---

## Rust Usage

### Should I use v4 or v5?

**Use v5** for new projects. The API is cleaner, the default build does
not pull in `serde_json`, and the arena evaluation path is exposed
directly. See the [Migration Guide](migration.md) for the move from v4.

If you're already on v4 and need a slow rollout, enable `features = ["compat"]`
to keep the v4 entry points available (every method is `#[deprecated]` so
the compiler will guide you through the rename).

### How do I share compiled rules across threads?

`Logic` is `Send + Sync`. Wrap it in `Arc` to share:

```rust
use datalogic_rs::Engine;
use std::sync::Arc;

let engine = Arc::new(Engine::new());
let compiled = Arc::new(engine.compile(rule).unwrap());

let compiled_clone = Arc::clone(&compiled);
std::thread::spawn(move || {
    let mut session = engine.session();
    session.evaluate_str(&compiled_clone, data)
});
```

### Why are custom operator arguments pre-evaluated in v5?

The pre-evaluated, arena-based design makes custom operators behave like
built-ins: the engine recurses, hands you `&DataValue<'a>` borrows, and you
return another arena allocation. This avoids the boundary conversion that
the v4 `Operator` trait paid on every call and removes the need for a
separate `Evaluator` trait.

If you need lazy / short-circuit semantics like `and` / `or`, that lives in
built-in operators today (none of the v5 short-circuit operators are
exposed through the public custom-operator surface).

### What's the difference between `evaluate`, `evaluate_str`, `session.evaluate*`, and `evaluate_json_value`?

| Method | Input | Output | Notes |
|--------|-------|--------|-------|
| `Engine::evaluate_str` | `&str`, `&str` | `String` | One-shot. Allocates a fresh arena internally. |
| `Engine::evaluate` | any `EvalInput` + `&Bump` | `&DataValue<'a>` | Hot path. Caller owns the arena, result borrows from it. |
| `Session::evaluate_str` | `&str` | `String` | Reuses the session's arena across calls. |
| `Session::evaluate` | any `EvalInput` | `OwnedDataValue` | Owned tree that survives the next reset. |
| `Engine::evaluate_json_value` (`compat`) | `&serde_json::Value` × 2 | `serde_json::Value` | Mirror of `evaluate_str` for callers on `serde_json`. |

---

## JavaScript / WASM Usage

### Do I need to call `init()` in Node.js?

No. The Node.js target does not require initialization:

```javascript
const { evaluate } = require('@goplasmatic/datalogic');
evaluate('{"==": [1, 1]}', '{}', false);
```

### Why do I need to JSON.stringify my data?

The WASM interface uses string-based communication for maximum compatibility:

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

React Flow (the underlying library) requires a container with defined
dimensions to calculate node positions and viewport.

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

---

## Operators

### How do I access array elements by index?

Use the `var` operator with numeric path segments:

```json
{"var": "items.0.name"}
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
{"if": [
    {"missing": ["user.email"]},
    "Email required",
    "Valid"
]}
```

Or use default values with `var`:

```json
{"var": ["user.email", "no-email@example.com"]}
```

### What happened to the `preserve` operator?

It was removed in v5. Literal scalars and arrays already pass through
inline, and templated objects belong in `preserve_structure` mode
(`Engine::builder().preserve_structure(true).build()`, requires
`feature = "preserve"`).

---

## Configuration

### How do I handle NaN in arithmetic?

Use the `NanHandling` configuration:

```rust
use datalogic_rs::{Engine, EvaluationConfig, NanHandling};

let config = EvaluationConfig {
    arithmetic_nan_handling: NanHandling::IgnoreValue,
    ..Default::default()
};
let engine = Engine::builder().config(config).build();
```

Options: `ThrowError` (default), `CoerceToZero`, `IgnoreValue`, `ReturnNull`.

### How do I change division by zero behavior?

```rust
use datalogic_rs::{EvaluationConfig, DivisionByZeroHandling};

let config = EvaluationConfig {
    division_by_zero: DivisionByZeroHandling::ReturnNull,
    ..Default::default()
};
```

Options: `ReturnSaturated` (default — `f64::MAX/MIN`), `ThrowError`,
`ReturnNull`, `ReturnInfinity`.

---

## Troubleshooting

### "Invalid operator" error

In standard mode, unrecognized keys are treated as errors. Either:

1. Fix the operator name (operators are case-sensitive)
2. Register a custom operator on the builder
3. Enable `preserve_structure` mode for templating (`feature = "preserve"`)

### Performance issues with large expressions

1. Use `Session` for repeated calls (arena reuse)
2. Drop to `Engine::evaluate` with a caller-managed `bumpalo::Bump` for the
   absolute hot path
3. Profile with `feature = "trace"` to identify slow sub-expressions

### WASM initialization fails

Ensure you `await init()` before calling other functions:

```javascript
await init();
const result = evaluate(...);
```

For more troubleshooting, see the [Troubleshooting Guide](troubleshooting.md).
