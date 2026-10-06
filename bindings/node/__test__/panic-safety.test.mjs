// Panic safety: a panic inside the native engine must surface as a thrown
// JS error (or a rejected promise), never abort the Node process.
//
// The trigger was a core bug: up to 5.8.0 `format_date` with an invalid
// chrono specifier (`%Q`) panicked inside the engine, and these tests were
// written (and seen to abort the process without the guard) against it.
// The core now reports that as an ordinary error, so the same calls take
// the error path; the tests still assert that each call throws and that
// the process (and the same engine, rule and session) keeps working
// afterwards. A caught panic is an `InternalError`, and its fields are
// checked whenever one shows up.

import test from 'node:test';
import assert from 'node:assert/strict';
import { DataHandle, Engine, apply } from '../index.js';

// All-literal form: constant folding runs it at compile time (a failed
// fold leaves the call in place, so it fails at evaluation instead).
const PANICKING_RULE = { format_date: ['2024-01-01T00:00:00Z', '%Q'] };
// Data-driven form: compiles fine, panics at evaluation.
const PANICKING_EVAL_RULE = { format_date: [{ var: 'd' }, '%Q'] };
const DATA = { d: '2024-01-01T00:00:00Z' };
const DATA_JSON = JSON.stringify(DATA);

// Accept any Error; if it is the caught-panic form, check its fields.
function isEngineFailure(e) {
  assert.ok(e instanceof Error, 'expected a JS Error');
  if (e.name === 'InternalError') {
    assert.equal(e.errorType, 'InternalError');
    assert.equal(typeof e.message, 'string');
    assert.ok(e.message.length > 0, 'panic message should be carried');
    assert.equal(e.operator, null);
    assert.deepEqual(e.nodeIds, []);
    assert.equal(e.path, null);
  }
  return true;
}

// Compile the data-driven rule. Returns null if the core has started
// rejecting the bad specifier at compile time (also acceptable).
function compileEvalRule(engine) {
  try {
    return engine.compile(PANICKING_EVAL_RULE);
  } catch (e) {
    isEngineFailure(e);
    return null;
  }
}

function assertStillWorks(engine) {
  const rule = engine.compile({ '+': [{ var: 'x' }, 1] });
  assert.equal(rule.evaluate({ x: 41 }), 42);
}

test('compile, apply and Engine one-shots throw instead of aborting', () => {
  const engine = new Engine();
  let literal = null;
  try {
    literal = engine.compile(PANICKING_RULE);
  } catch (e) {
    isEngineFailure(e);
  }
  if (literal) {
    assert.throws(() => literal.evaluate({}), isEngineFailure);
  }
  assert.throws(() => apply(PANICKING_RULE, {}), isEngineFailure);
  assert.throws(() => apply(PANICKING_EVAL_RULE, DATA), isEngineFailure);
  assert.throws(() => engine.eval(PANICKING_EVAL_RULE, DATA), isEngineFailure);
  assert.throws(() => engine.evalStr(PANICKING_EVAL_RULE, DATA_JSON), isEngineFailure);
  assert.throws(() => engine.evalMetered(PANICKING_EVAL_RULE, DATA_JSON), isEngineFailure);
  assert.equal(apply({ '==': [1, 1] }, {}), true);
  assertStillWorks(engine);
});

test('compiled Rule paths throw instead of aborting', () => {
  const engine = new Engine();
  const rule = compileEvalRule(engine);
  if (rule) {
    const handle = new DataHandle(DATA_JSON);
    assert.throws(() => rule.evaluate(DATA), isEngineFailure);
    assert.throws(() => rule.evaluateStr(DATA_JSON), isEngineFailure);
    assert.throws(() => rule.evaluateMetered(DATA_JSON), isEngineFailure);
    assert.throws(() => rule.evaluateData(handle), isEngineFailure);
    assert.throws(() => rule.evaluateDataStr(handle), isEngineFailure);
    // Same rule again: a caught panic leaves it usable.
    assert.throws(() => rule.evaluate(DATA), isEngineFailure);
  }
  assertStillWorks(engine);
});

test('Session paths throw and the session stays usable', () => {
  const engine = new Engine();
  const good = engine.compile({ '+': [{ var: 'x' }, 1] });
  const goodHandle = new DataHandle('{"x": 41}');
  const sess = engine.session();
  const bad = compileEvalRule(engine);
  if (bad) {
    const handle = new DataHandle(DATA_JSON);
    assert.throws(() => sess.evaluate(bad, DATA), isEngineFailure);
    assert.throws(() => sess.evaluateStr(bad, DATA_JSON), isEngineFailure);
    assert.throws(() => sess.evaluateData(bad, handle), isEngineFailure);
    assert.throws(() => sess.evaluateDataStr(bad, handle), isEngineFailure);
    assert.throws(() => sess.evaluateTruthy(bad, handle), isEngineFailure);
  }
  assert.equal(sess.evaluate(good, { x: 41 }), 42);
  assert.equal(sess.evaluateData(good, goodHandle), 42);
});

test('evaluateStrAsync rejects instead of aborting', async () => {
  const engine = new Engine();
  const rule = compileEvalRule(engine);
  if (rule) {
    await assert.rejects(rule.evaluateStrAsync(DATA_JSON), isEngineFailure);
  }
  const good = engine.compile({ '+': [{ var: 'x' }, 1] });
  assert.equal(await good.evaluateStrAsync('{"x": 41}'), '42');
  assertStillWorks(engine);
});
