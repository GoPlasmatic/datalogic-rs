// Compile-once / evaluate-many via Engine + Rule.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine, Rule } from '../index.js';

test('engine constructor with no options', () => {
  const engine = new Engine();
  assert.ok(engine);
});

test('engine templating option', () => {
  const engine = new Engine({ templating: true });
  const rule = engine.compile({
    name: { var: 'user.name' },
    ok: { '>': [{ var: 'score' }, 50] },
  });
  assert.deepEqual(
    rule.evaluate({ user: { name: 'Ada' }, score: 99 }),
    { name: 'Ada', ok: true }
  );
});

test('compile returns Rule instance', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [1, 2] });
  assert.ok(rule instanceof Rule);
});

test('rule.evaluate with object data', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  for (let x = 0; x < 5; x++) {
    assert.equal(rule.evaluate({ x }), x + 1);
  }
});

test('rule.evaluateStr returns JSON string', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  assert.equal(rule.evaluateStr('{"x": 41}'), '42');
});

test('rule.evaluate accepts JSON-string data', () => {
  const engine = new Engine();
  const rule = engine.compile({ var: 'msg' });
  assert.equal(rule.evaluate('{"msg": "hi"}'), 'hi');
});

test('compile from JSON-encoded string rule', () => {
  const engine = new Engine();
  const rule = engine.compile('{"+": [1, 2]}');
  assert.equal(rule.evaluate({}), 3);
});

test('engine one-shot eval / evalStr', () => {
  const engine = new Engine();
  assert.equal(engine.eval({ '+': [1, 2] }, {}), 3);
  assert.equal(engine.evalStr({ '+': [1, 2] }, {}), '3');
});

test('rule shared across many evaluations', () => {
  const engine = new Engine();
  const rule = engine.compile({
    if: [{ '>': [{ var: 'n' }, 0] }, 'positive', 'non-positive'],
  });
  const payloads = [{ n: 5 }, { n: -5 }, { n: 0 }];
  const expected = ['positive', 'non-positive', 'non-positive'];
  assert.deepEqual(payloads.map((p) => rule.evaluate(p)), expected);
});

test('rule traverses nested objects', () => {
  const engine = new Engine();
  const rule = engine.compile({ var: 'a.b.0.c' });
  assert.equal(
    rule.evaluate({ a: { b: [{ c: 'deep' }, { c: 'shallow' }] } }),
    'deep'
  );
});

test('rule returns array', () => {
  const engine = new Engine();
  const rule = engine.compile({ var: 'items' });
  assert.deepEqual(rule.evaluate({ items: [1, 2, 3] }), [1, 2, 3]);
});

test('rule string-data path matches object-data path', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, { var: 'y' }] });
  const data = { x: 10, y: 32 };
  assert.equal(rule.evaluate(data), rule.evaluate(JSON.stringify(data)));
});
