# Troubleshooting

Common issues and solutions for datalogic-rs.

## Rust Issues

### "Unknown operator: xyz"

**Cause:** Using an unrecognized operator name.

**Solutions:**
1. Check the operator name spelling (operators are case-sensitive)
2. Register a custom operator if it's your own
3. Enable `preserve_structure` mode if you're using JSONLogic for templating

```rust
// Option 1: Fix spelling
let logic = json!({ "and": [...] }); // not "AND"

// Option 2: Custom operator
engine.add_operator("xyz".to_string(), Box::new(XyzOperator));

// Option 3: Templating mode
let engine = DataLogic::with_preserve_structure();
```

### "Variable not found"

**Cause:** Accessing a path that doesn't exist in the data.

**Solutions:**
1. Check the variable path spelling
2. Use a default value
3. Use `missing` to check first

```rust
// Default value
let logic = json!({ "var": ["user.name", "Anonymous"] });

// Check first
let logic = json!({
    "if": [
        { "missing": ["user.name"] },
        "No name",
        { "var": "user.name" }
    ]
});
```

### "NaN" or unexpected arithmetic results

**Cause:** Non-numeric values in arithmetic operations.

**Solution:** Configure NaN handling:

```rust
use datalogic_rs::{EvaluationConfig, NanHandling};

let config = EvaluationConfig::default()
    .with_nan_handling(NanHandling::IgnoreValue); // or CoerceToZero
let engine = DataLogic::with_config(config);
```

### Thread safety errors

**Cause:** Custom operators that aren't `Send + Sync`.

**Solution:** Ensure custom operators are thread-safe:

```rust
// This won't compile if operator uses RefCell, Rc, etc.
engine.add_operator("my_op".to_string(), Box::new(MyOperator));

// Use Arc, Mutex, or make fields immutable
struct MyOperator {
    config: Arc<Config>, // Thread-safe
}
```

### Slow compilation

**Cause:** Very large or deeply nested expressions.

**Solutions:**
1. Compile once, evaluate many times
2. Break expressions into smaller pieces
3. Consider using `preserve_structure` for simpler parsing

```rust
// Compile once
let compiled = engine.compile(&logic)?;

// Evaluate many times
for data in dataset {
    engine.evaluate_owned(&compiled, data)?;
}
```

---

## JavaScript/WASM Issues

### "RuntimeError: memory access out of bounds"

**Cause:** WASM module not initialized.

**Solution:** Call `init()` before using any functions:

```javascript
import init, { evaluate } from '@goplasmatic/datalogic';

await init(); // Must await before using evaluate
evaluate(logic, data, false);
```

### "TypeError: Cannot read properties of undefined"

**Cause:** Using the wrong import style for your environment.

**Solutions:**

```javascript
// Browser/Bundler - need default import for init
import init, { evaluate } from '@goplasmatic/datalogic';

// Node.js - no init needed
const { evaluate } = require('@goplasmatic/datalogic');
```

### "Failed to fetch" in browser

**Cause:** WASM file not accessible from the browser.

**Solutions:**
1. Check your bundler configuration
2. Ensure WASM files are being served correctly
3. Check CORS headers if loading from CDN

For Vite, it should work automatically. For Webpack:

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
const result = JSON.parse(resultString); // Parse to native value
```

### Performance issues

**Cause:** Recompiling rules repeatedly.

**Solution:** Use `CompiledRule`:

```javascript
// Slow - compiles each time
for (const item of items) {
  evaluate(logic, JSON.stringify(item), false);
}

// Fast - compile once
const rule = new CompiledRule(logic, false);
for (const item of items) {
  rule.evaluate(JSON.stringify(item));
}
```

---

## React UI Issues

### "ResizeObserver loop completed with undelivered notifications"

**Cause:** Container size changes rapidly.

**Solution:** This warning is usually harmless, but you can debounce size changes:

```tsx
function StableEditor({ expression }) {
  const containerRef = useRef<HTMLDivElement>(null);

  return (
    <div ref={containerRef} style={{ height: '500px' }}>
      <DataLogicEditor value={expression} />
    </div>
  );
}
```

### Editor shows blank/empty

**Causes:**
1. Container has no dimensions
2. CSS not imported
3. Expression is null

**Solutions:**

```tsx
// 1. Ensure container has dimensions
<div style={{ width: '100%', height: '500px' }}>
  <DataLogicEditor value={expression} />
</div>

// 2. Import CSS in correct order
import '@xyflow/react/dist/style.css';
import '@goplasmatic/datalogic-ui/styles.css';

// 3. Check expression
<DataLogicEditor value={expression ?? { "==": [1, 1] }} />
```

### Debug mode not showing results

**Cause:** `data` prop not provided.

**Solution:** Debug mode requires data:

```tsx
<DataLogicEditor
  value={expression}
  data={{ x: 1, y: 2 }} // Required for debug mode
  mode="debug"
/>
```

### SSR/Hydration errors in Next.js

**Cause:** WASM doesn't run on server.

**Solution:** Use client component:

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
# Install wasm-pack
cargo install wasm-pack

# Add target
rustup target add wasm32-unknown-unknown

# Build
cd wasm && ./build.sh
```

### TypeScript errors with imports

**Cause:** Missing type declarations or wrong import.

**Solution:** Check your tsconfig.json:

```json
{
  "compilerOptions": {
    "moduleResolution": "bundler", // or "node16"
    "allowSyntheticDefaultImports": true
  }
}
```

### Bundler can't find WASM file

**Cause:** WASM file not copied to output.

**Solution:** Depends on bundler:

```javascript
// Vite - usually automatic

// Webpack - enable async WASM
experiments: { asyncWebAssembly: true }

// Rollup - use @rollup/plugin-wasm
```

---

## Getting Help

If you can't resolve an issue:

1. Check [existing issues](https://github.com/GoPlasmatic/datalogic-rs/issues)
2. Create a minimal reproduction
3. Open a new issue with:
   - datalogic-rs version
   - Environment (Rust/Node/Browser)
   - Minimal code to reproduce
   - Expected vs actual behavior
