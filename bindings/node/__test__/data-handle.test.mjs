// DataHandle + the evaluation tiers built on it (ABI v2 mirror):
// handle evaluation paths, typed scalar results, batch entry points,
// and the async string tier.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine, DataHandle } from '../index.js';

// ---------------- DataHandle basics ----------------

test('DataHandle parses and reports allocated bytes', () => {
  const h = new DataHandle('{"user": {"age": 34}}');
  assert.ok(h instanceof DataHandle);
  assert.ok(h.allocatedBytes > 0);
});

test('DataHandle throws ParseError on malformed JSON', () => {
  assert.throws(() => new DataHandle('{ not json'), (e) => {
    assert.equal(e.name, 'ParseError');
    assert.equal(e.errorType, 'ParseError');
    return true;
  });
});

test('a handle is reusable across rules, engines, and sessions', () => {
  const h = new DataHandle('{"x": 41}');
  const e1 = new Engine();
  const e2 = new Engine();
  const r1 = e1.compile({ '+': [{ var: 'x' }, 1] });
  const r2 = e2.compile({ '*': [{ var: 'x' }, 2] });
  assert.equal(r1.evaluateData(h), 42);
  assert.equal(r2.evaluateData(h), 82);
  const s1 = e1.session();
  const s2 = e2.session();
  assert.equal(s1.evaluateData(r1, h), 42);
  assert.equal(s2.evaluateData(r2, h), 82);
  // Not consumed by evaluation.
  assert.equal(r1.evaluateData(h), 42);
});

// ---------------- handle evaluation paths ----------------

test('Rule.evaluateData / evaluateDataStr match the string paths', () => {
  const engine = new Engine();
  const rule = engine.compile({ cat: ['id-', { var: 'id' }] });
  const json = '{"id": "A7"}';
  const h = new DataHandle(json);
  assert.equal(rule.evaluateData(h), rule.evaluate(json));
  assert.equal(rule.evaluateDataStr(h), rule.evaluateStr(json));
});

test('Session.evaluateData / evaluateDataStr match the string paths', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  const sess = engine.session();
  const h = new DataHandle('{"x": 1}');
  for (let i = 0; i < 5; i++) {
    assert.equal(sess.evaluateData(rule, h), 2);
    assert.equal(sess.evaluateDataStr(rule, h), '2');
  }
});

test('handle evaluation surfaces structured EvaluateErrors', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  const h = new DataHandle('{"x": "not a number"}');
  assert.throws(() => rule.evaluateData(h), (e) => {
    assert.equal(e.name, 'EvaluateError');
    assert.ok(e.errorType);
    return true;
  });
  const sess = engine.session();
  assert.throws(() => sess.evaluateDataStr(rule, h), (e) => {
    assert.equal(e.name, 'EvaluateError');
    return true;
  });
});

// ---------------- typed scalar results ----------------

test('evaluateBool returns strict booleans', () => {
  const engine = new Engine();
  const sess = engine.session();
  const h = new DataHandle('{"age": 25}');
  assert.equal(sess.evaluateBool(engine.compile({ '>': [{ var: 'age' }, 18] }), h), true);
  assert.equal(sess.evaluateBool(engine.compile({ '<': [{ var: 'age' }, 18] }), h), false);
});

test('evaluateBool mismatches on non-boolean results', () => {
  const engine = new Engine();
  const sess = engine.session();
  const h = new DataHandle('{"age": 25}');
  assert.throws(() => sess.evaluateBool(engine.compile({ var: 'age' }), h), (e) => {
    assert.equal(e.name, 'EvaluateError');
    assert.equal(e.errorType, 'TypeMismatch');
    assert.equal(e.message, 'result is not a boolean (got number)');
    return true;
  });
});

test('evaluateNumber accepts any JSON number', () => {
  const engine = new Engine();
  const sess = engine.session();
  const h = new DataHandle('{"i": 41, "f": 2.5}');
  assert.equal(sess.evaluateNumber(engine.compile({ '+': [{ var: 'i' }, 1] }), h), 42);
  assert.equal(sess.evaluateNumber(engine.compile({ var: 'f' }), h), 2.5);
});

test('evaluateNumber mismatches on non-number results', () => {
  const engine = new Engine();
  const sess = engine.session();
  const h = new DataHandle('{"s": "text"}');
  assert.throws(() => sess.evaluateNumber(engine.compile({ var: 's' }), h), (e) => {
    assert.equal(e.errorType, 'TypeMismatch');
    assert.equal(e.message, 'result is not a number (got string)');
    return true;
  });
  assert.throws(() => sess.evaluateNumber(engine.compile({ '>': [1, 0] }), h), (e) => {
    assert.equal(e.message, 'result is not a number (got boolean)');
    return true;
  });
});

test('evaluateTruthy never mismatches', () => {
  const engine = new Engine();
  const sess = engine.session();
  const h = new DataHandle('{"one": 1, "zero": 0, "s": "x", "arr": [1]}');
  assert.equal(sess.evaluateTruthy(engine.compile({ var: 'one' }), h), true);
  assert.equal(sess.evaluateTruthy(engine.compile({ var: 'zero' }), h), false);
  assert.equal(sess.evaluateTruthy(engine.compile({ var: 's' }), h), true);
  assert.equal(sess.evaluateTruthy(engine.compile({ var: 'arr' }), h), true);
});

// ---------------- batch: one rule × many handles ----------------

test('evaluateBatch returns allSettled-shaped items in order', () => {
  const engine = new Engine();
  const rule = engine.compile({ '>': [{ var: 'age' }, 18] });
  const sess = engine.session();
  const handles = [21, 3, 65].map((age) => new DataHandle(JSON.stringify({ age })));
  const out = sess.evaluateBatch(rule, handles);
  assert.deepEqual(out, [
    { status: 'fulfilled', value: 'true' },
    { status: 'fulfilled', value: 'false' },
    { status: 'fulfilled', value: 'true' },
  ]);
});

test('evaluateBatch item failures never fail the call', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  const sess = engine.session();
  const good = new DataHandle('{"x": 1}');
  const bad = new DataHandle('{"x": "not a number"}');
  const out = sess.evaluateBatch(rule, [good, bad, good]);
  assert.equal(out.length, 3);
  assert.deepEqual(out[0], { status: 'fulfilled', value: '2' });
  assert.equal(out[1].status, 'rejected');
  assert.ok(out[1].reason.tag.length > 0, 'reason.tag is populated');
  assert.ok(out[1].reason.message.length > 0, 'reason.message is populated');
  // The neighbour after the failure still evaluates.
  assert.deepEqual(out[2], { status: 'fulfilled', value: '2' });
});

test('evaluateBatch rejected reasons carry the failing operator when known', () => {
  const engine = new Engine();
  const rule = engine.compile({ '/': [1, { var: 'd' }] });
  const sess = engine.session();
  const out = sess.evaluateBatch(rule, [new DataHandle('{"d": 0}')]);
  assert.equal(out[0].status, 'rejected');
  assert.equal(out[0].reason.operator, '/');
});

test('evaluateBatch argument errors do throw', () => {
  const engine = new Engine();
  const rule = engine.compile({ var: 'x' });
  const sess = engine.session();
  const h = new DataHandle('{"x": 1}');
  // A non-DataHandle element is an argument error, not an item error.
  assert.throws(() => sess.evaluateBatch(rule, [h, {}]));
  assert.throws(() => sess.evaluateBatch(rule, [h, null]));
  assert.throws(() => sess.evaluateBatch(rule, 'not an array'));
});

test('evaluateBatch on an empty array returns an empty array', () => {
  const engine = new Engine();
  const sess = engine.session();
  assert.deepEqual(sess.evaluateBatch(engine.compile({ var: 'x' }), []), []);
});

// ---------------- batch: many rules × one handle ----------------

test('evaluateMany returns one item per rule in order', () => {
  const engine = new Engine();
  const sess = engine.session();
  const rules = [
    engine.compile({ '>': [{ var: 'age' }, 18] }),
    engine.compile({ var: 'name' }),
    engine.compile({ '+': [{ var: 'age' }, 1] }),
  ];
  const h = new DataHandle('{"age": 34, "name": "Ada"}');
  assert.deepEqual(sess.evaluateMany(rules, h), [
    { status: 'fulfilled', value: 'true' },
    { status: 'fulfilled', value: '"Ada"' },
    { status: 'fulfilled', value: '35' },
  ]);
});

test('evaluateMany isolates item failures', () => {
  const engine = new Engine();
  const sess = engine.session();
  const rules = [
    engine.compile({ var: 'x' }),
    engine.compile({ '+': ['text', 1] }),
    engine.compile({ var: 'x' }),
  ];
  const out = sess.evaluateMany(rules, new DataHandle('{"x": 7}'));
  assert.deepEqual(out[0], { status: 'fulfilled', value: '7' });
  assert.equal(out[1].status, 'rejected');
  assert.ok(out[1].reason.tag);
  assert.deepEqual(out[2], { status: 'fulfilled', value: '7' });
});

test('evaluateMany argument errors do throw', () => {
  const engine = new Engine();
  const sess = engine.session();
  const h = new DataHandle('{}');
  assert.throws(() => sess.evaluateMany([engine.compile({ var: 'x' }), 42], h));
  assert.deepEqual(sess.evaluateMany([], h), []);
});

// ---------------- cross-engine rules ----------------
//
// A Node `Rule` carries its own engine Arc, and `Session` methods
// evaluate the rule's compiled logic with the *session's* engine —
// there is no engine-identity check (unlike the C ABI, which rejects
// the pair). These tests pin that documented behavior.

test('a rule from another engine evaluates under the session engine config', () => {
  // NaN-producing arithmetic returns null on the lax engine, throws on
  // the default one — an observable probe for "whose config applies".
  const lax = new Engine({ config: { arithmetic_nan_handling: 'return_null' } });
  const strict = new Engine();
  const rule = lax.compile({ '+': [{ var: 's' }, 1] });
  const h = new DataHandle('{"s": "text"}');

  // On its own engine: null, no throw.
  assert.equal(rule.evaluateData(h), null);
  // On the strict engine's session: the session's config applies.
  const sess = strict.session();
  assert.throws(() => sess.evaluateData(rule, h), (e) => {
    assert.equal(e.name, 'EvaluateError');
    return true;
  });
  // And the same rule still works on its own engine afterwards.
  assert.equal(rule.evaluateData(h), null);
});

test('evaluateMany accepts rules compiled by another engine (session engine wins)', () => {
  const lax = new Engine({ config: { arithmetic_nan_handling: 'return_null' } });
  const strict = new Engine();
  const foreign = lax.compile({ '+': [{ var: 's' }, 1] });
  const native = strict.compile({ var: 's' });
  const sess = strict.session();
  const out = sess.evaluateMany([native, foreign], new DataHandle('{"s": "text"}'));
  assert.deepEqual(out[0], { status: 'fulfilled', value: '"text"' });
  // The foreign rule runs under the strict session engine → item failure.
  assert.equal(out[1].status, 'rejected');
});

// ---------------- async string tier ----------------

test('evaluateStrAsync resolves with the result JSON string', async () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  assert.equal(await rule.evaluateStrAsync('{"x": 41}'), '42');
});

test('evaluateStrAsync runs concurrently and preserves per-call inputs', async () => {
  const engine = new Engine();
  const rule = engine.compile({ '*': [{ var: 'x' }, 2] });
  const results = await Promise.all(
    Array.from({ length: 16 }, (_, i) => rule.evaluateStrAsync(JSON.stringify({ x: i }))),
  );
  results.forEach((r, i) => assert.equal(r, String(i * 2)));
});

test('evaluateStrAsync rejects with the structured error surface', async () => {
  const engine = new Engine();
  const rule = engine.compile({ var: 'x' });
  await assert.rejects(rule.evaluateStrAsync('{ bad json'), (e) => {
    assert.equal(e.name, 'ParseError');
    assert.equal(e.errorType, 'ParseError');
    assert.ok(Array.isArray(e.nodeIds));
    return true;
  });

  const failing = engine.compile({ '+': ['text', 1] });
  await assert.rejects(failing.evaluateStrAsync('{}'), (e) => {
    assert.equal(e.name, 'EvaluateError');
    assert.ok(e.errorType);
    assert.ok(Array.isArray(e.path), 'path resolves for evaluate failures');
    return true;
  });
});

test('evaluateStrAsync rejects when it reaches a JS custom operator', async () => {
  // Custom-operator callbacks are pinned to the registering JS thread;
  // the async path computes on the libuv pool, so reaching one rejects
  // with a normal EvaluateError instead of touching the foreign isolate.
  const engine = new Engine({}, { double: (a) => JSON.stringify(JSON.parse(a)[0] * 2) });
  const rule = engine.compile({ double: [21] });
  assert.equal(rule.evaluateStr('{}'), '42'); // sync path works
  await assert.rejects(rule.evaluateStrAsync('{}'), (e) => {
    assert.equal(e.name, 'EvaluateError');
    assert.match(e.message, /different thread/);
    return true;
  });
});
