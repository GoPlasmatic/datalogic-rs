# Troubleshooting

Common issues and solutions for datalogic-rs.

## Rust Issues

### "Invalid operator: xyz"

**Cause:** Using an unrecognized operator name.

**Solutions:**

1. Check the operator name spelling (operators are case-sensitive).
2. Register a custom operator on the builder.
3. Enable `preserve_structure` mode for templating
   (requires `feature = "preserve"`).

```rust
// Option 1: Fix spelling
let logic = r#"{"and": [...]}"#;  // not "AND"

// Option 2: Custom operator
let engine = datalogic_rs::Engine::builder()
    .add_operator("xyz", XyzOperator)
    .build();

// Option 3: Templating mode (feature = "preserve")
# #[cfg(feature = "preserve")]
let engine = datalogic_rs::Engine::builder().preserve_structure(true).build();
```

### "Variable not found"

**Cause:** Accessing a path that doesn't exist in the data.

**Solutions:**

1. Check the variable path spelling
2. Use a default value
3. Use `missing` to check first

```json
{"var": ["user.name", "Anonymous"]}

{"if": [
    {"missing": ["user.name"]},
    "No name",
    {"var": "user.name"}
]}
```

### Unexpected `NaN` / `Thrown` errors from arithmetic

**Cause:** Non-numeric values in arithmetic operations.

**Solution:** Configure NaN handling:

```rust
use datalogic_rs::{Engine, EvaluationConfig, NanHandling};

let config = EvaluationConfig {
    arithmetic_nan_handling: NanHandling::IgnoreValue, // or CoerceToZero
    ..Default::default()
};
let engine = Engine::builder().config(config).build();
```

### "the trait bound `T: CustomOperator` is not satisfied" / `Send`-`Sync` errors

**Cause:** Custom operator type that isn't `Send + Sync`.

**Solution:** Use thread-safe primitives. Avoid `Rc`, `RefCell`, etc., in
operator state — wrap shared state in `Arc<Mutex<_>>` or atomics.

### v4 method calls fail to compile in v5

**Cause:** v5 renamed the public surface (`DataLogic` → `Engine`,
`CompiledLogic` → `Logic`, `Operator` → `CustomOperator`, etc.).

**Solutions:**

- For a quick rollout, enable `features = ["compat"]` and import the
  legacy methods via the `LegacyApi` trait:

  ```rust
  use datalogic_rs::compat::LegacyApi;
  ```

  Each shimmed method is `#[deprecated]` so the compiler points you at the
  v5 replacement.

- For a clean migration, follow the [Migration Guide](migration.md).

### Slow compilation

**Cause:** Very large or deeply nested expressions.

**Solutions:**

- Compile once, evaluate many times
- Break expressions into smaller composable pieces
- Profile with `feature = "trace"` to see which sub-expressions dominate

```rust
let compiled = engine.compile(rule).unwrap();
let mut session = engine.session();
for data in dataset {
    session.evaluate_str(&compiled, data)?;
}
```

---

## JavaScript / WASM Issues

### "RuntimeError: memory access out of bounds"

**Cause:** WASM module not initialized.

**Solution:** Call `init()` before using any functions:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic';

await init();
evaluate(logic, data, false);
```

### "TypeError: Cannot read properties of undefined"

**Cause:** Wrong import style for your environment.

**Solutions:**

```javascript
// Browser/Bundler — need default import for init
import init, { evaluate } from '@goplasmatic/datalogic';

// Node.js — no init needed
const { evaluate } = require('@goplasmatic/datalogic');
```

### "Failed to fetch" in browser

**Cause:** WASM file not accessible from the browser.

**Solutions:**

1. Check your bundler configuration
2. Ensure WASM files are served correctly
3. Check CORS headers if loading from CDN

For Webpack:

```javascript
// webpack.config.js
module.exports = {
  experiments: {
    asyncWebAssembly: true,
  },
};
```

### Results are strings, not values

**Cause:** WASM returns JSON strings, not native values.

**Solution:** Parse the result:

```javascript
const resultString = evaluate(logic, data, false);
const result = JSON.parse(resultString);
```

### Performance issues

**Cause:** Recompiling rules repeatedly.

**Solution:** Use `CompiledRule`:

```javascript
const rule = new CompiledRule(logic, false);
for (const item of items) {
  rule.evaluate(JSON.stringify(item));
}
```

---

## React UI Issues

### "ResizeObserver loop completed with undelivered notifications"

**Cause:** Container size changes rapidly. Usually harmless.

### Editor shows blank / empty

**Causes:**

1. Container has no dimensions
2. CSS not imported
3. Expression is null

**Solutions:**

```tsx
<div style={{ width: '100%', height: '500px' }}>
  <DataLogicEditor value={expression} />
</div>
```

```tsx
import '@xyflow/react/dist/style.css';
import '@goplasmatic/datalogic-ui/styles.css';
```

### Debug mode not showing results

**Cause:** `data` prop not provided.

**Solution:**

```tsx
<DataLogicEditor
  value={expression}
  data={{ x: 1, y: 2 }}
  mode="debug"
/>
```

### SSR / Hydration errors in Next.js

**Cause:** WASM doesn't run on server.

**Solution:** Use a client component with dynamic import:

```tsx
'use client';

import dynamic from 'next/dynamic';

const DataLogicEditor = dynamic(
  () => import('@goplasmatic/datalogic-ui').then(mod => mod.DataLogicEditor),
  { ssr: false }
);
```

---

## Build Issues

### WASM build fails

**Cause:** Missing wasm-pack or target.

**Solution:**

```bash
cargo install wasm-pack
rustup target add wasm32-unknown-unknown
cd packages/wasm && ./build.sh
```

### TypeScript errors with imports

```json
{
  "compilerOptions": {
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true
  }
}
```

### Bundler can't find WASM file

```javascript
// Webpack — enable async WASM
experiments: { asyncWebAssembly: true }
```

---

## Getting Help

If you can't resolve an issue:

1. Check [existing issues](https://github.com/GoPlasmatic/datalogic-rs/issues)
2. Create a minimal reproduction
3. Open a new issue with:
   - datalogic-rs version
   - Environment (Rust / Node / Browser)
   - Minimal code to reproduce
   - Expected vs actual behavior
