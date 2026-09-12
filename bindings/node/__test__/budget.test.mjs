// Operation budget: what an evaluation costs, and what happens when it
// costs too much.
//
// The count is the engine's to define, so these assert the properties the
// binding promises — a cost is reported, a ceiling is enforced, the error
// carries both numbers — rather than exact figures that a new fast path in
// the core would move.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine, DataHandle } from '../index.js';

const MAP = '{"map": [{"var": "xs"}, {"*": [{"var": ""}, 2]}]}';
const small = '{"xs": [1, 2, 3]}';
const large = JSON.stringify({ xs: Array.from({ length: 200 }, (_, i) => i) });

test('evalMetered reports the result and what it cost', () => {
  const { result, ops } = new Engine().evalMetered(MAP, small);
  assert.equal(result, '[2,4,6]');
  assert.ok(ops >= 3, `expected at least one operation per item, got ${ops}`);
});

test('a constant-folded rule costs nothing', () => {
  assert.equal(new Engine().evalMetered('{"+": [1, 2]}', 'null').ops, 0);
});

test('an explicit budget refuses an evaluation that would cross it', () => {
  assert.throws(
    () => new Engine().evalMetered(MAP, large, 10),
    (e) => {
      assert.equal(e.errorType, 'BudgetExceeded');
      assert.equal(e.budget, 10);
      assert.ok(e.spent > 10, 'error reports what the rule asked for');
      return true;
    },
  );
});

test('a budget that fits is not refused', () => {
  const { ops } = new Engine().evalMetered(MAP, small);
  assert.equal(new Engine().evalMetered(MAP, small, ops).ops, ops);
  assert.throws(() => new Engine().evalMetered(MAP, small, ops - 1));
});

test('the ops_budget config key bounds every entry point, not just the metered one', () => {
  const engine = new Engine({ config: { ops_budget: 10 } });
  assert.throws(() => engine.evalStr(MAP, large), (e) => e.errorType === 'BudgetExceeded');
  assert.throws(() => engine.eval(MAP, JSON.parse(large)), (e) => e.errorType === 'BudgetExceeded');
  // ...and an explicit argument overrides it for one call.
  assert.ok(engine.evalMetered(MAP, large, 1_000_000).ops > 10);
});

test('metering falls back to the configured budget when no argument is given', () => {
  const engine = new Engine({ config: { ops_budget: 10 } });
  assert.throws(() => engine.evalMetered(MAP, large), (e) => e.errorType === 'BudgetExceeded');
});

test('try cannot recover from an exhausted budget', () => {
  const engine = new Engine({ config: { ops_budget: 10 } });
  assert.throws(
    () => engine.evalStr(`{"try": [${MAP}, "fallback"]}`, large),
    (e) => e.errorType === 'BudgetExceeded',
  );
});

test('Rule.evaluateMetered meters a compiled rule', () => {
  const engine = new Engine();
  const rule = engine.compile(MAP);
  const { result, ops } = rule.evaluateMetered(small);
  assert.equal(result, '[2,4,6]');
  assert.ok(ops >= 3);
  assert.throws(() => rule.evaluateMetered(large, 5), (e) => e.errorType === 'BudgetExceeded');
});

test('a rejected budget argument is an argument error, not a silent truncation', () => {
  const engine = new Engine();
  for (const bad of [0, -1, 2.5]) {
    assert.throws(() => engine.evalMetered(MAP, small, bad), (e) => {
      assert.match(e.message, /whole number/);
      return true;
    }, `budget ${bad} should be rejected`);
  }
});

test('an unset budget leaves evaluation unbounded', () => {
  const { result } = new Engine().evalMetered(MAP, large);
  assert.equal(JSON.parse(result).length, 200);
});

test('metering accepts object input as well as JSON strings', () => {
  const { result, ops } = new Engine().evalMetered(JSON.parse(MAP), { xs: [1, 2, 3] });
  assert.equal(result, '[2,4,6]');
  assert.ok(ops >= 3);
});

test('DataHandle evaluation still honours the configured budget', () => {
  const engine = new Engine({ config: { ops_budget: 10 } });
  const rule = engine.compile(MAP);
  const handle = new DataHandle(large);
  assert.throws(() => rule.evaluateDataStr(handle), (e) => e.errorType === 'BudgetExceeded');
});
