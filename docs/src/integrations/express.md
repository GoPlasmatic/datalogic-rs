# Integration: Express (Node.js)

This guide wires [`@goplasmatic/datalogic-node`](../nodejs/overview.md)
into an Express service the way it wants to be used in production:
**one engine for the process, rules compiled once and cached,
evaluation per request**, with the async tier keeping heavy
evaluations off the event loop.

The running example is a discount service: rules live in a database
column as JSONLogic, product/ops people change them without a deploy,
and the API applies whichever rule is active.

## Install

```bash
npm install express @goplasmatic/datalogic-node
```

The package ships native prebuilds for eight platforms: no Rust
toolchain or node-gyp involved.

## A rule service: compile once, cache by version

Compiling is the expensive step (still microseconds, but don't pay it
per request). Cache compiled rules keyed by their identity, and let a
rule update replace the cache entry:

```js
// rules.js
import { Engine } from '@goplasmatic/datalogic-node';

const engine = new Engine();          // one per process
const cache = new Map();              // ruleId@version -> compiled Rule

export function getRule(row) {
  // row: { id, version, logic } from your DB
  const key = `${row.id}@${row.version}`;
  let rule = cache.get(key);
  if (!rule) {
    rule = engine.compile(row.logic); // throws on malformed JSON or invalid structure
    cache.set(key, rule);
  }
  return rule;
}
```

Compiled `Rule` objects are immutable and safe to share across requests,
so a plain `Map` is all the machinery you need. If rules churn, swap the
`Map` for an LRU: compiled rules are cheap to rebuild. (They cannot be
posted to `worker_threads`; a worker loads the module and keeps its own
cache.)

## The endpoint

```js
// app.js
import express from 'express';
import { getRule } from './rules.js';
import { loadActiveDiscountRule } from './db.js';

const app = express();
app.use(express.json());

app.post('/quote', async (req, res, next) => {
  try {
    const row = await loadActiveDiscountRule();
    const rule = getRule(row);
    const total = rule.evaluate({ cart: req.body.cart, user: req.user });
    res.json({ total });
  } catch (err) {
    next(err);
  }
});
```

`rule.evaluate(data)` takes and returns plain JS objects: no manual
JSON stringify/parse on your side.

## Validating rules at ingestion, not at request time

The moment users or admins can author rules, treat rule ingestion like
any other untrusted input path:

```js
app.put('/rules/:id', express.json({ limit: '64kb' }), (req, res) => {
  let compiled;
  try {
    compiled = engine.compile(req.body.logic); // JSON + structure check only
  } catch (err) {
    return res.status(422).json({ error: `invalid rule: ${err.message}` });
  }
  // run the rule against golden cases before activating it; this is also
  // where an unknown operator name surfaces (errorType 'InvalidOperator')
  try {
    for (const [input, expected] of req.body.tests ?? []) {
      if (JSON.stringify(compiled.evaluate(input)) !== JSON.stringify(expected)) {
        return res.status(422).json({ error: 'rule fails its test cases' });
      }
    }
  } catch (err) {
    return res.status(422).json({ error: `rule fails its test cases: ${err.message}` });
  }
  // ...persist row with a bumped version...
  res.sendStatus(204);
});
```

`compile` rejects malformed JSON and structurally invalid rules (for
example a multi-key object when templating is off), but it does **not**
verify operator names: `{ "discuont": [...] }` compiles fine and throws
`errorType: 'InvalidOperator'` only when you evaluate the rule. The
golden-case loop catches this; if a rule ships without test cases,
evaluate it once against a representative payload before persisting it.

Two things are doing security work here: the **size limit** on the body
(a hostile 10 MB rule is safe to compile but not free), and the
compile-then-test gate. Evaluation itself is sandboxed: rules have no
I/O, no `eval`, and can only read the data document you pass.

## Keeping big evaluations off the event loop

Evaluations are typically sub-microsecond, so the sync call is right
for most endpoints. For large payloads or batch endpoints, use the
async tier. It runs on the libuv thread pool:

```js
app.post('/evaluate-batch', async (req, res, next) => {
  try {
    const rule = getRule(await loadActiveDiscountRule());
    const results = await Promise.all(
      req.body.items.map((item) => rule.evaluateStrAsync(JSON.stringify(item)))
    );
    res.json(results.map((r) => JSON.parse(r)));
  } catch (err) {
    next(err);
  }
});
```

## Letting the frontend preview the same rule

Because every binding runs the same core, you can preview the exact
rule your Express service enforces in the browser with
[`@goplasmatic/datalogic-wasm`](../javascript/installation.md), or
render and step-debug it with the
[React visual editor](../react-ui/installation.md). Nothing gets
re-implemented, so what the UI shows matches what the API decides.

## Error handling

`compile` and `evaluate` throw real `Error` objects with a stable
`errorType` tag (`"ParseError"`, `"InvalidOperator"`, `"Thrown"`,
`"InvalidArguments"`, ...). The typed session methods
(`evaluateBool` / `evaluateNumber`) add `"TypeMismatch"` for a result of
the wrong type. Map them in your Express error middleware:

```js
app.use((err, req, res, next) => {
  if (err.errorType === 'ParseError') return res.status(422).json({ error: err.message });
  if (err.errorType) return res.status(400).json({ error: err.message });
  next(err);
});
```

See the [Node.js chapter](../nodejs/overview.md) for the full API
surface (sessions, data handles, typed results, batch and async
evaluation, tracing, operator names).
