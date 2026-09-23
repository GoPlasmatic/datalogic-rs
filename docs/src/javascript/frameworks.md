# Framework Integration

Integration recipes for common JavaScript frameworks and build tools.

## React

### Basic Setup

```tsx
import { useEffect, useState } from 'react';
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic-wasm';

function App() {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    init().then(() => setReady(true));
  }, []);

  if (!ready) return <div>Loading...</div>;

  return <RuleEvaluator />;
}

function RuleEvaluator() {
  const result = evaluate('{"==": [1, 1]}', '{}', false);
  return <div>Result: {result}</div>;
}
```

### Custom Hook

Create a reusable hook for JSONLogic evaluation:

```tsx
import { useEffect, useRef, useState } from 'react';
import init, { CompiledRule } from '@goplasmatic/datalogic-wasm';

// Initialize once at module level
let initPromise: Promise<unknown> | null = null;
function ensureInit() {
  if (!initPromise) {
    initPromise = init();
  }
  return initPromise;
}

// Inputs are keyed by their JSON text, so callers may pass fresh object
// literals on every render: the rule is recompiled only when its text
// changes, and an object result can never re-trigger the effect.
export function useJsonLogic(logic: object, data: unknown) {
  const [ready, setReady] = useState(false);
  const [result, setResult] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);
  const ruleRef = useRef<{ json: string; rule: CompiledRule } | null>(null);

  const logicJson = JSON.stringify(logic);
  const dataJson = JSON.stringify(data);

  useEffect(() => {
    ensureInit().then(() => setReady(true));
  }, []);

  useEffect(() => {
    if (!ready) return;
    try {
      if (!ruleRef.current || ruleRef.current.json !== logicJson) {
        // Release the previous rule's WASM memory before compiling the next one
        const previous = ruleRef.current;
        ruleRef.current = null;
        previous?.rule.free();
        ruleRef.current = { json: logicJson, rule: new CompiledRule(logicJson, false) };
      }
      setResult(JSON.parse(ruleRef.current.rule.evaluate(dataJson)));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [logicJson, dataJson, ready]);

  // Free the compiled rule on unmount
  useEffect(() => () => {
    ruleRef.current?.rule.free();
    ruleRef.current = null;
  }, []);

  return { result, error, ready };
}
```

Two details matter here. The effects key on the serialized inputs, not the objects: an inline rule literal is a new object identity on every render, and an object result from `JSON.parse` is a new identity on every evaluation, so keying on the objects themselves would recompile per render and, for object results, loop until React reports "Maximum update depth exceeded". And every `CompiledRule` holds WASM memory, so the hook calls `free()` when it replaces a rule and on unmount rather than waiting for the garbage collector.

Usage:

```tsx
function FeatureFlag({ feature, user }) {
  // An inline literal is fine: the hook keys on the rule's JSON text
  const rule = { "and": [
    { "in": [feature, { "var": "enabledFeatures" }] },
    { ">=": [{ "var": "accountAge" }, 30] }
  ]};

  const { result, error, ready } = useJsonLogic(rule, user);

  if (!ready) return null;
  if (error) return <div>Error: {error}</div>;
  return result ? <NewFeature /> : <LegacyFeature />;
}
```

### With React Query

```tsx
import { useQuery } from '@tanstack/react-query';
import init, { CompiledRule } from '@goplasmatic/datalogic-wasm';

export function useCompiledRule(logic: object) {
  return useQuery({
    queryKey: ['compiled-rule', JSON.stringify(logic)],
    queryFn: async () => {
      await init();
      return new CompiledRule(JSON.stringify(logic), false);
    },
    staleTime: Infinity,
  });
}
```

---

## Vue

### Composition API

```vue
<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import init, { CompiledRule } from '@goplasmatic/datalogic-wasm';

const ready = ref(false);
const data = ref({ age: 25 });

onMounted(async () => {
  await init();
  ready.value = true;
});

const rule = computed(() => {
  if (!ready.value) return null;
  return new CompiledRule('{">=": [{"var": "age"}, 18]}', false);
});

const isAdult = computed(() => {
  if (!rule.value) return null;
  return JSON.parse(rule.value.evaluate(JSON.stringify(data.value)));
});
</script>

<template>
  <div v-if="ready">
    Is Adult: {{ isAdult }}
  </div>
  <div v-else>Loading...</div>
</template>
```

### Composable

```typescript
// useJsonLogic.ts
import { ref, onMounted, onUnmounted, watchEffect, Ref } from 'vue';
import init, { CompiledRule } from '@goplasmatic/datalogic-wasm';

let initPromise: Promise<unknown> | null = null;

export function useJsonLogic(logic: Ref<object>, data: Ref<unknown>) {
  const result = ref<unknown>(null);
  const error = ref<string | null>(null);
  const ready = ref(false);
  let compiled: { json: string; rule: CompiledRule } | null = null;

  onMounted(async () => {
    if (!initPromise) initPromise = init();
    await initPromise;
    ready.value = true;
  });

  watchEffect(() => {
    if (!ready.value) return;
    const logicJson = JSON.stringify(logic.value);
    const dataJson = JSON.stringify(data.value);
    try {
      // Recompile only when the rule text changes, freeing the previous rule
      if (!compiled || compiled.json !== logicJson) {
        const previous = compiled;
        compiled = null;
        previous?.rule.free();
        compiled = { json: logicJson, rule: new CompiledRule(logicJson, false) };
      }
      result.value = JSON.parse(compiled.rule.evaluate(dataJson));
      error.value = null;
    } catch (e) {
      error.value = String(e);
    }
  });

  onUnmounted(() => {
    compiled?.rule.free();
    compiled = null;
  });

  return { result, error, ready };
}
```

As with the React hook, the composable compares the rule's JSON text so a `data` change does not recompile, and it frees each `CompiledRule` it replaces (and the last one on unmount) instead of leaving WASM memory to the garbage collector.

---

## Node.js

### Express Middleware

```javascript
const express = require('express');
const { evaluate, CompiledRule } = require('@goplasmatic/datalogic-wasm');

const app = express();
app.use(express.json());

// Compile rules at startup
const rules = {
  canAccess: new CompiledRule(JSON.stringify({
    "and": [
      { "==": [{ "var": "role" }, "admin"] },
      { "var": "active" }
    ]
  }), false)
};

// Middleware
function authorize(ruleName) {
  return (req, res, next) => {
    const rule = rules[ruleName];
    if (!rule) return res.status(500).json({ error: 'Unknown rule' });

    const result = JSON.parse(rule.evaluate(JSON.stringify(req.user)));
    if (result) {
      next();
    } else {
      res.status(403).json({ error: 'Forbidden' });
    }
  };
}

app.get('/admin', authorize('canAccess'), (req, res) => {
  res.json({ message: 'Welcome, admin!' });
});
```

### Rule Evaluation API

```javascript
const { evaluate } = require('@goplasmatic/datalogic-wasm');

app.post('/api/evaluate', (req, res) => {
  const { logic, data, templating = false } = req.body;

  try {
    const result = evaluate(
      JSON.stringify(logic),
      JSON.stringify(data),
      templating
    );
    res.json({ result: JSON.parse(result) });
  } catch (error) {
    res.status(400).json({ error: String(error) });
  }
});
```

---

## Bundler Configuration

### Vite

Vite needs no WASM-specific configuration:

```typescript
// vite.config.ts
import { defineConfig } from 'vite';

export default defineConfig({
  // No special configuration needed
});
```

### Webpack 5

Enable async WASM:

```javascript
// webpack.config.js
module.exports = {
  experiments: {
    asyncWebAssembly: true,
  },
};
```

### Next.js

```javascript
// next.config.js
module.exports = {
  webpack: (config) => {
    config.experiments = {
      ...config.experiments,
      asyncWebAssembly: true,
    };
    return config;
  },
};
```

For App Router, create a client component:

```tsx
'use client';

import { useEffect, useState } from 'react';
import init, { evaluate } from '@goplasmatic/datalogic-wasm';

export function JsonLogicEvaluator({ logic, data }) {
  const [result, setResult] = useState(null);

  useEffect(() => {
    init().then(() => {
      const res = evaluate(JSON.stringify(logic), JSON.stringify(data), false);
      setResult(JSON.parse(res));
    });
  }, [logic, data]);

  return <div>{JSON.stringify(result)}</div>;
}
```

---

## Browser (No Build Tools)

For simple pages without bundlers:

```html
<!DOCTYPE html>
<html>
<head>
  <title>JSONLogic Demo</title>
</head>
<body>
  <div id="result"></div>

  <script type="module">
    import init, { evaluate } from 'https://unpkg.com/@goplasmatic/datalogic-wasm@latest/web/datalogic_wasm.js';

    async function run() {
      await init();

      const logic = JSON.stringify({ ">=": [{ "var": "age" }, 18] });
      const data = JSON.stringify({ age: 21 });
      const result = JSON.parse(evaluate(logic, data, false));

      document.getElementById('result').textContent =
        result ? 'Adult' : 'Minor';
    }

    run();
  </script>
</body>
</html>
```

---

## Worker Threads

### Web Worker

```javascript
// worker.js
import init, { CompiledRule } from '@goplasmatic/datalogic-wasm';

let rule = null;

self.onmessage = async (e) => {
  if (e.data.type === 'init') {
    await init();
    rule = new CompiledRule(e.data.logic, false);
    self.postMessage({ type: 'ready' });
  } else if (e.data.type === 'evaluate') {
    const result = rule.evaluate(JSON.stringify(e.data.data));
    self.postMessage({ type: 'result', result: JSON.parse(result) });
  }
};
```

### Node.js Worker Thread

```javascript
const { Worker, isMainThread, parentPort } = require('worker_threads');
const { CompiledRule } = require('@goplasmatic/datalogic-wasm');

if (isMainThread) {
  const worker = new Worker(__filename);
  worker.postMessage({ logic: '{"==": [1, 1]}', data: {} }); // logic is JSON text, data is an object
  worker.on('message', (result) => console.log(result));      // true
} else {
  parentPort.on('message', ({ logic, data }) => {
    // logic is already JSON text: pass it through as is. Stringifying it
    // again would compile the literal string '{"==": [1, 1]}' and return it.
    const rule = new CompiledRule(logic, false);
    const result = JSON.parse(rule.evaluate(JSON.stringify(data)));
    parentPort.postMessage(result);
  });
}
```
