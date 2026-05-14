// Error surface — name, errorType, operator, nodeIds, path.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine, apply } from '../index.js';

test('ParseError on malformed rule string', () => {
  const engine = new Engine();
  assert.throws(() => engine.compile('{ this is not json'), (e) => {
    assert.equal(e.name, 'ParseError');
    assert.equal(e.errorType, 'ParseError');
    return true;
  });
});

test('ParseError on malformed data string', () => {
  const engine = new Engine();
  const rule = engine.compile({ var: 'x' });
  assert.throws(() => rule.evaluate('{ also not json'), (e) => {
    assert.equal(e.name, 'ParseError');
    return true;
  });
});

test('EvaluateError carries structured attributes', () => {
  const engine = new Engine();
  const rule = engine.compile({ '+': ['x', 1] });
  assert.throws(() => rule.evaluate({}), (e) => {
    assert.equal(e.name, 'EvaluateError');
    assert.ok(e.errorType, 'errorType should be populated');
    assert.equal(e.operator, '+');
    assert.ok(Array.isArray(e.nodeIds));
    assert.ok(e.nodeIds.every((n) => Number.isInteger(n)));
    assert.ok(Array.isArray(e.path));
    assert.equal(e.path[0].operator, '+');
    return true;
  });
});

test('apply throws EvaluateError', () => {
  assert.throws(() => apply({ '+': ['nope', 1] }, {}), (e) => {
    assert.equal(e.name, 'EvaluateError');
    return true;
  });
});

test('thrown operator surfaces as EvaluateError', () => {
  const engine = new Engine();
  const rule = engine.compile({ throw: 'boom' });
  assert.throws(() => rule.evaluate({}), (e) => {
    assert.equal(e.errorType, 'Thrown');
    return true;
  });
});
