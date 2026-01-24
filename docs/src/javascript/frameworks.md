# Framework Integration

This guide covers integration with popular JavaScript frameworks and build tools.

## React

### Basic Setup

```tsx
import { useEffect, useState } from 'react';
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic';

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
import { useEffect, useState, useMemo } from 'react';
import init, { CompiledRule } from '@goplasmatic/datalogic';

// Initialize once at module level
let initPromise: Promise<void> | null = null;
function ensureInit() {
  if (!initPromise) {
    initPromise = init();
  }
  return initPromise;
}

export function useJsonLogic(logic: object, data: unknown) {
  const [ready, setReady] = useState(false);
  const [result, setResult] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);

  const rule = useMemo(() => {
    if (!ready) return null;
    try {
      return new CompiledRule(JSON.stringify(logic), false);
    } catch (e) {
      setError(String(e));
      return null;
    }
  }, [logic, ready]);

  useEffect(() => {
    ensureInit().then(() => setReady(true));
  }, []);

  useEffect(() => {
    if (!rule) return;
    try {
      const res = rule.evaluate(JSON.stringify(data));
      setResult(JSON.parse(res));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [rule, data]);

  return { result, error, ready };
}
```

Usage:

```tsx
function FeatureFlag({ feature, user }) {
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
import init, { CompiledRule } from '@goplasmatic/datalogic';

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
import init, { CompiledRule } from '@goplasmatic/datalogic';

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
import { ref, onMounted, watchEffect, Ref } from 'vue';
import init, { CompiledRule } from '@goplasmatic/datalogic';

let initialized = false;
let initPromise: Promise<void> | null = null;

export function useJsonLogic(logic: Ref<object>, data: Ref<unknown>) {
  const result = ref<unknown>(null);
  const error = ref<string | null>(null);
  const ready = ref(false);

  onMounted(async () => {
    if (!initialized) {
      if (!initPromise) initPromise = init();
      await initPromise;
      initialized = true;
    }
    ready.value = true;
  });

  watchEffect(() => {
    if (!ready.value) return;
    try {
      const rule = new CompiledRule(JSON.stringify(logic.value), false);
      result.value = JSON.parse(rule.evaluate(JSON.stringify(data.value)));
      error.value = null;
    } catch (e) {
      error.value = String(e);
    }
  });

  return { result, error, ready };
}
```

---

## Node.js

### Express Middleware

```javascript
const express = require('express');
const { evaluate, CompiledRule } = require('@goplasmatic/datalogic');

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
const { evaluate } = require('@goplasmatic/datalogic');

app.post('/api/evaluate', (req, res) => {
  const { logic, data, preserveStructure = false } = req.body;

  try {
    const result = evaluate(
      JSON.stringify(logic),
      JSON.stringify(data),
      preserveStructure
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

WASM works out of the box with Vite:

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
import init, { evaluate } from '@goplasmatic/datalogic';

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
    import init, { evaluate } from 'https://unpkg.com/@goplasmatic/datalogic@latest/web/datalogic_wasm.js';

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
import init, { CompiledRule } from '@goplasmatic/datalogic';

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
const { CompiledRule } = require('@goplasmatic/datalogic');

if (isMainThread) {
  const worker = new Worker(__filename);
  worker.postMessage({ logic: '{"==": [1, 1]}', data: {} });
  worker.on('message', (result) => console.log(result));
} else {
  parentPort.on('message', ({ logic, data }) => {
    const rule = new CompiledRule(JSON.stringify(logic), false);
    const result = JSON.parse(rule.evaluate(JSON.stringify(data)));
    parentPort.postMessage(result);
  });
}
```
