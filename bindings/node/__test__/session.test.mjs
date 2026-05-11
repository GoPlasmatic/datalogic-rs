// Session — hot-loop arena reuse.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine, Session } from '../index.js';

test('session basic loop', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  const sess = engine.session();
  assert.ok(sess instanceof Session);
  for (let x = 0; x < 10; x++) {
    assert.equal(sess.evaluate(rule, { x }), x + 1);
  }
});

test('session evaluateStr', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  const sess = engine.session();
  assert.equal(sess.evaluateStr(rule, '{"x": 41}'), '42');
});

test('session reset returns undefined', () => {
  const engine = new Engine();
  const sess = engine.session();
  assert.equal(sess.reset(), undefined);
});

test('session allocated bytes grow then stay after reset', () => {
  // `evaluate*` resets at the start of each call, so allocatedBytes is
  // the high-water mark for the most recent eval.
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  const sess = engine.session();
  sess.evaluate(rule, { x: 1 });
  const after = sess.allocatedBytes();
  assert.ok(after > 0);
  sess.reset();
  // Bump::reset rewinds the bump pointer but keeps chunk memory — the
  // high-water mark stays.
  assert.equal(sess.allocatedBytes(), after);
});

test('session handles multiple rules in alternation', () => {
  const engine = new Engine();
  const add = engine.compile({ '+': [{ var: 'x' }, 1] });
  const mul = engine.compile({ '*': [{ var: 'x' }, 2] });
  const sess = engine.session();
  assert.equal(sess.evaluate(add, { x: 4 }), 5);
  assert.equal(sess.evaluate(mul, { x: 4 }), 8);
  assert.equal(sess.evaluate(add, { x: 10 }), 11);
});
