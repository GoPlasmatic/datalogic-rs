# Framework Integration

Integration recipes for common JavaScript frameworks and build tools.

## React

### Basic Setup

```tsx
import { useEffect, useState } from 'react';
import init, { Engine } from '@goplasmatic/datalogic-wasm';

function App() {
  const [engine, setEngine] = useState<Engine | null>(null);

  useEffect(() => {
    init().then(() => setEngine(new Engine()));
  }, []);

  if (!engine) return <div>Loading...</div>;

  return <RuleEvaluator engine={engine} />;
}

function RuleEvaluator({ engine }: { engine: Engine }) {
  const result = engine.evalStr('{"==": [1, 1]}', '{}');
  return <div>Result: {result}</div>;
}
```

### Custom Hook

Create a reusable hook for JSONLogic evaluation:

```tsx
// useJsonLogic.ts
import { useEffect, useRef, useState } from 'react';
import init, { Engine, Rule } from '@goplasmatic/datalogic-wasm';

// Load the module and build one engine, once, for every component
let enginePromise: Promise<Engine> | null = null;
export function getEngine() {
  if (!enginePromise) {
    enginePromise = init().then(() => new Engine());
  }
  return enginePromise;
}

// Inputs are keyed by their JSON text, so callers may pass fresh object
// literals on every render: the rule is recompiled only when its text
// changes, and an object result can never re-trigger the effect.
export function useJsonLogic(logic: object, data: unknown) {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [result, setResult] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);
  const ruleRef = useRef<{ json: string; rule: Rule } | null>(null);

  const logicJson = JSON.stringify(logic);
  const dataJson = JSON.stringify(data);

  useEffect(() => {
    getEngine().then(setEngine);
  }, []);

  useEffect(() => {
    if (!engine) return;
    try {
      if (!ruleRef.current || ruleRef.current.json !== logicJson) {
        // Release the previous rule's WASM memory before compiling the next one
        const previous = ruleRef.current;
        ruleRef.current = null;
        previous?.rule.free();
        ruleRef.current = { json: logicJson, rule: engine.compile(logicJson) };
      }
      setResult(JSON.parse(ruleRef.current.rule.evaluate(dataJson)));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [logicJson, dataJson, engine]);

  // Free the compiled rule on unmount
  useEffect(() => () => {
    ruleRef.current?.rule.free();
    ruleRef.current = null;
  }, []);

  return { result, error, ready: engine !== null };
}
```

Two details matter here. The effects key on the serialized inputs, not the objects: an inline rule literal is a new object identity on every render, and an object result from `JSON.parse` is a new identity on every evaluation, so keying on the objects themselves would recompile per render and, for object results, loop until React reports "Maximum update depth exceeded". And every `Rule` holds WASM memory, so the hook calls `free()` when it replaces a rule and on unmount instead of leaving it to the garbage collector. The engine lives for the page, so the hook never frees it.

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
import { getEngine } from './useJsonLogic'; // the shared engine from the hook above

export function useCompiledRule(logic: object) {
  return useQuery({
    queryKey: ['compiled-rule', JSON.stringify(logic)],
    queryFn: async () => (await getEngine()).compile(JSON.stringify(logic)),
    staleTime: Infinity,
  });
}
```

---

## Vue

### Composition API

```vue
<script setup lang="ts">
import { ref, shallowRef, onMounted, computed } from 'vue';
import init, { Engine } from '@goplasmatic/datalogic-wasm';

const engine = shallowRef<Engine | null>(null);
const ready = computed(() => engine.value !== null);
const data = ref({ age: 25 });

onMounted(async () => {
  await init();
  engine.value = new Engine();
});

const rule = computed(() => {
  if (!engine.value) return null;
  return engine.value.compile('{">=": [{"var": "age"}, 18]}');
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
import init, { Engine, Rule } from '@goplasmatic/datalogic-wasm';

// One engine for the app, built after the module loads
let enginePromise: Promise<Engine> | null = null;

export function useJsonLogic(logic: Ref<object>, data: Ref<unknown>) {
  const result = ref<unknown>(null);
  const error = ref<string | null>(null);
  const ready = ref(false);
  let engine: Engine | null = null;
  let compiled: { json: string; rule: Rule } | null = null;

  onMounted(async () => {
    if (!enginePromise) enginePromise = init().then(() => new Engine());
    engine = await enginePromise;
    ready.value = true;
  });

  watchEffect(() => {
    if (!ready.value || !engine) return;
    const logicJson = JSON.stringify(logic.value);
    const dataJson = JSON.stringify(data.value);
    try {
      // Recompile only when the rule text changes, freeing the previous rule
      if (!compiled || compiled.json !== logicJson) {
        const previous = compiled;
        compiled = null;
        previous?.rule.free();
        compiled = { json: logicJson, rule: engine.compile(logicJson) };
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

As with the React hook, the composable compares the rule's JSON text so a `data` change does not recompile, and it frees each `Rule` it replaces (and the last one on unmount) instead of leaving WASM memory to the garbage collector.

---

## Node.js

On Node servers, the native [`@goplasmatic/datalogic-node`](../nodejs/overview.md) package is faster and takes rules and data as JS values; [Integration: Express](../integrations/express.md) walks through a service built on it. The recipes below use the WASM package, for code that shares one artifact with a browser build.

### Express Middleware

```javascript
const express = require('express');
const { Engine } = require('@goplasmatic/datalogic-wasm');

const app = express();
app.use(express.json());

// One engine for the process; compile rules at startup
const engine = new Engine();
const rules = {
  canAccess: engine.compile(JSON.stringify({
    "and": [
      { "==": [{ "var": "role" }, "admin"] },
      { "var": "active" }
    ]
  }))
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
const { Engine } = require('@goplasmatic/datalogic-wasm');
const engine = new Engine();

app.post('/api/evaluate', (req, res) => {
  const { logic, data, templating = false } = req.body;

  let rule;
  try {
    // Choose the templating mode per request instead of per engine
    const text = JSON.stringify(logic);
    rule = templating ? engine.compileTemplate(text) : engine.compile(text);
    res.json({ result: JSON.parse(rule.evaluate(JSON.stringify(data))) });
  } catch (error) {
    res.status(400).json({ error: String(error) });
  } finally {
    rule?.free();
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
import init, { Engine } from '@goplasmatic/datalogic-wasm';

let enginePromise: Promise<Engine> | null = null;
const getEngine = () => (enginePromise ??= init().then(() => new Engine()));

export function JsonLogicEvaluator({ logic, data }) {
  const [result, setResult] = useState(null);

  useEffect(() => {
    getEngine().then((engine) => {
      const res = engine.evalStr(JSON.stringify(logic), JSON.stringify(data));
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
    import init, { Engine } from 'https://unpkg.com/@goplasmatic/datalogic-wasm@latest/web/datalogic_wasm.js';

    async function run() {
      await init();
      const engine = new Engine();

      const logic = JSON.stringify({ ">=": [{ "var": "age" }, 18] });
      const data = JSON.stringify({ age: 21 });
      const result = JSON.parse(engine.evalStr(logic, data));

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
import init, { Engine } from '@goplasmatic/datalogic-wasm';

let rule = null;

self.onmessage = async (e) => {
  if (e.data.type === 'init') {
    await init();
    // Each Worker loads its own module, so it builds its own engine and rule
    rule = new Engine().compile(e.data.logic);
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
const { Engine } = require('@goplasmatic/datalogic-wasm');

if (isMainThread) {
  const worker = new Worker(__filename);
  worker.postMessage({ logic: '{"==": [1, 1]}', data: {} }); // logic is JSON text, data is an object
  worker.on('message', (result) => console.log(result));      // true
} else {
  const engine = new Engine(); // one per worker
  parentPort.on('message', ({ logic, data }) => {
    // logic is already JSON text: pass it through as is. Stringifying it
    // again would compile the literal string '{"==": [1, 1]}' and return it.
    const rule = engine.compile(logic);
    const result = JSON.parse(rule.evaluate(JSON.stringify(data)));
    rule.free();
    parentPort.postMessage(result);
  });
}
```
