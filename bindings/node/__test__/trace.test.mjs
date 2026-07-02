// `Engine.evaluateWithTrace`: WASM-compatible trace envelope.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine } from '../index.js';

test('evaluateWithTrace returns result, steps, and expression tree', () => {
  const engine = new Engine();
  const out = JSON.parse(engine.evaluateWithTrace('{"+": [1, 2, 3]}', 'null'));

  assert.equal(out.result, 6);
  assert.ok(Array.isArray(out.steps));
  assert.ok(out.steps.length > 0, 'expected at least one execution step');
  const step = out.steps[0];
  assert.ok(Number.isInteger(step.step_id));
  assert.ok(Number.isInteger(step.node_id));
  assert.ok('context' in step);
  assert.ok('result' in step);
  assert.ok(Number.isInteger(out.expression_tree.id));
  assert.equal(typeof out.expression_tree.expression, 'string');
  assert.ok(Array.isArray(out.expression_tree.children));
  assert.ok(!('error' in out), 'no error field on success');
  assert.ok(!('structured_error' in out), 'no structured_error field on success');
});

test('evaluateWithTrace records nested operator steps', () => {
  const engine = new Engine();
  const rule = '{"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]}';
  const out = JSON.parse(engine.evaluateWithTrace(rule, '{"score": 75}'));

  assert.equal(out.result, 'pass');
  assert.ok(out.steps.length >= 2, 'nested rule should surface multiple steps');
  assert.ok(out.expression_tree.children.length > 0);
});

test('evaluateWithTrace reports runtime errors inside the envelope', () => {
  const engine = new Engine();
  const out = JSON.parse(engine.evaluateWithTrace('{"+": ["x", 1]}', 'null'));

  assert.equal(out.result, null);
  assert.equal(typeof out.error, 'string');
  assert.ok(out.structured_error, 'structured_error should accompany error');
  assert.ok(Array.isArray(out.steps));
});

test('evaluateWithTrace reports parse errors inside the envelope', () => {
  const engine = new Engine();
  const out = JSON.parse(engine.evaluateWithTrace('{ not json', 'null'));

  assert.equal(out.result, null);
  assert.equal(typeof out.error, 'string');
  assert.deepEqual(out.steps, []);
});

test('evaluateWithTrace runs on the constructing engine (config + custom ops)', () => {
  const engine = new Engine(
    { config: { preset: 'safe_arithmetic' } },
    { double: (a) => JSON.stringify(JSON.parse(a)[0] * 2) }
  );
  const withConfig = JSON.parse(
    engine.evaluateWithTrace('{"+": [1, "abc", 2]}', 'null')
  );
  assert.equal(withConfig.result, 3);
  const withCustom = JSON.parse(
    engine.evaluateWithTrace('{"double": [21]}', 'null')
  );
  assert.equal(withCustom.result, 42);
});
