// One-shot `apply()` — parity surface with classic JSONLogic bindings.

import test from 'node:test';
import assert from 'node:assert/strict';
import { apply } from '../index.js';

test('apply with object inputs', () => {
  const rule = { if: [{ '>': [{ var: 'score' }, 50] }, 'pass', 'fail'] };
  assert.equal(apply(rule, { score: 75 }), 'pass');
  assert.equal(apply(rule, { score: 25 }), 'fail');
});

test('apply with string inputs', () => {
  const rule = '{"+": [{"var": "x"}, 1]}';
  const data = '{"x": 41}';
  assert.equal(apply(rule, data), 42);
});

test('apply with mixed string and object', () => {
  assert.equal(apply({ '+': [1, 2] }, 'null'), 3);
});

test('apply against nested data', () => {
  assert.equal(apply({ var: 'a.b.c' }, { a: { b: { c: 'deep' } } }), 'deep');
});

test('apply returns object', () => {
  assert.deepEqual(
    apply({ var: 'user' }, { user: { name: 'Ada', active: true } }),
    { name: 'Ada', active: true }
  );
});

test('apply returns array', () => {
  assert.deepEqual(apply({ var: 'items' }, { items: [1, 2, 3] }), [1, 2, 3]);
});
