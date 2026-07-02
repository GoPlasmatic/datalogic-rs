// Thread affinity of custom-operator engines.
//
// The Rust side guards every custom-operator invocation with a
// thread-affinity check: if the operator is ever invoked from a thread
// other than the one that registered it, evaluation fails with a normal
// engine error instead of dereferencing a foreign V8 isolate. That
// mismatch branch is not reachable from JS today: napi class instances
// structured-clone into hollow plain objects (the native wrap and the
// prototype are dropped by the clone algorithm), so a worker that
// receives an Engine cannot call methods on it at all, and the binding
// has no async API that moves evaluation off the JS thread. These tests
// pin down the two behaviors that ARE observable: the clone arrives
// hollow, and the supported per-worker pattern (each worker builds its
// own engine) works, which exercises the affinity check's happy path on
// a non-main thread.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Worker } from 'node:worker_threads';
import { fileURLToPath } from 'node:url';
import { Engine } from '../index.js';

const indexPath = fileURLToPath(new URL('../index.js', import.meta.url));

test('an Engine posted to a worker arrives hollow and unusable', async () => {
  const engine = new Engine({}, {
    double: (a) => JSON.stringify(JSON.parse(a)[0] * 2),
  });
  const src = `
    const { parentPort } = require('node:worker_threads');
    parentPort.on('message', (m) => {
      parentPort.postMessage({
        hasEvalStr: typeof m.evalStr === 'function',
        keys: Object.keys(m),
      });
    });
  `;
  const worker = new Worker(src, { eval: true });
  try {
    const report = await new Promise((resolve, reject) => {
      worker.once('message', resolve);
      worker.once('error', reject);
      worker.postMessage(engine);
    });
    // The structured clone drops the native wrap and the prototype, so
    // the receiving thread has no way to reach the custom operator.
    assert.equal(report.hasEvalStr, false);
    assert.deepEqual(report.keys, []);
  } finally {
    await worker.terminate();
  }
  // The original engine on this thread is untouched by the clone.
  assert.equal(engine.evalStr('{"double": [21]}', 'null'), '42');
});

test('each worker can build and use its own custom-operator engine', async () => {
  const src = `
    const { parentPort } = require('node:worker_threads');
    const { Engine } = require(${JSON.stringify(indexPath)});
    const engine = new Engine({}, {
      double: (a) => JSON.stringify(JSON.parse(a)[0] * 2),
    });
    parentPort.postMessage(engine.evalStr('{"double": [21]}', 'null'));
  `;
  const worker = new Worker(src, { eval: true });
  try {
    const result = await new Promise((resolve, reject) => {
      worker.once('message', resolve);
      worker.once('error', reject);
    });
    // The operator was registered AND invoked on the worker thread, so
    // the affinity check compares equal there. This would fail if the
    // check pinned operators to the main thread instead of the
    // registering thread.
    assert.equal(result, '42');
  } finally {
    await worker.terminate();
  }
});
