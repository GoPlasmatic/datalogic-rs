// Pins how this binding refuses a bad argument today. The bindings do not
// agree (budget 0 is `InvalidArguments` here, `InvalidArgument` in Python,
// a `ParseError` in WASM and the engine's own budget through the C ABI),
// so the cross-binding scenarios cannot hold these cases; each binding
// pins its own until one spelling is chosen.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine, DataHandle } from '../index.js';

const invalid = { name: 'EvaluateError', errorType: 'InvalidArguments' };

test('a budget that is not a whole number >= 1 is InvalidArguments', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [1, 2] });
  for (const budget of [0, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, 2 ** 53 + 2]) {
    assert.throws(() => rule.evaluateMetered(null, budget), invalid, String(budget));
    assert.throws(() => engine.evalMetered({ '+': [1, 2] }, null, budget), invalid, String(budget));
  }
  // An omitted budget falls back to the engine's.
  assert.equal(rule.evaluateMetered(null).result, '3');
});

test('a template-key escape that is not one character is InvalidArguments', () => {
  for (const templateKeyEscape of ['', 'ab']) {
    assert.throws(() => new Engine({ templating: true, templateKeyEscape }), invalid);
  }
});

test('an unknown mode is InvalidArguments', () => {
  const engine = new Engine();
  assert.throws(() => engine.check({ var: 'a' }, 'loose'), invalid);
  assert.throws(() => engine.evaluateWithTrace('{"var": "a"}', '{}', 'loose'), invalid);
});

test('an unknown config key or family is a ConfigurationError', () => {
  const configuration = { name: 'EvaluateError', errorType: 'ConfigurationError' };
  assert.throws(() => new Engine({ config: { bogus: 1 } }), configuration);
  assert.throws(() => new Engine({ families: ['Strings'] }), configuration);
});

test('a traced result keeps its key order', () => {
  const engine = new Engine();
  const text = engine.evaluateWithTrace('{"var": "o"}', '{"o": {"z": 1, "a": {"y": 2, "b": 3}}}');
  assert.ok(text.startsWith('{"result":{"z":1,"a":{"y":2,"b":3}},'), text);
  assert.deepEqual(Object.keys(JSON.parse(text).result), ['z', 'a']);
});

test('allocatedBytes are plain numbers', () => {
  const engine = new Engine();
  const handle = new DataHandle('{"a": [1, 2, 3]}');
  assert.equal(typeof handle.allocatedBytes, 'number');
  assert.ok(handle.allocatedBytes > 0);
  const session = engine.session();
  session.evaluateData(engine.compile({ map: [{ var: 'a' }, { '+': [{ var: '' }, 1] }] }), handle);
  assert.equal(typeof session.allocatedBytes(), 'number');
  assert.ok(session.allocatedBytes() > 0);
});
