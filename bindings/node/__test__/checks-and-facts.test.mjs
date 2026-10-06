// The 5.8 introspection surface: check / compileChecked, operators(),
// rule facts, per-compile templating mode, truthy and strict operator
// names.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Engine } from '../index.js';

test('check reports every problem with a JSON pointer', () => {
  const engine = new Engine();
  const diags = engine.check({ if: [true, { vr: 'x' }, { map: [1] }] });
  assert.deepEqual(
    diags.map((d) => [d.code, d.severity, d.pointer]),
    [
      ['UnknownOperator', 'error', '/if/1'],
      ['ArgumentCount', 'error', '/if/2'],
    ],
  );
  assert.match(diags[0].message, /did you mean `var`/);
  assert.equal(diags[0].operator, 'vr');
  assert.deepEqual(engine.check('{"+": [1, 2]}'), []);
});

test('check takes a mode', () => {
  const engine = new Engine();
  const template = { a: { var: 'x' }, b: 1 };
  assert.equal(engine.check(template)[0].code, 'NotAnOperator');
  assert.deepEqual(engine.check(template, 'template'), []);
  assert.throws(() => engine.check(template, 'loose'), { errorType: 'InvalidArguments' });
});

test('compileChecked refuses an error and carries the diagnostics', () => {
  const engine = new Engine();
  assert.throws(
    () => engine.compileChecked({ if: [{ bogus: 1 }, { map: [1] }] }),
    (err) => {
      assert.equal(err.name, 'CompileError');
      assert.equal(err.errorType, 'CompileError');
      assert.equal(err.diagnostics.length, 2);
      return true;
    },
  );
  const rule = engine.compileChecked({ '+': [1, { var: 'x' }] });
  assert.equal(rule.evaluate({ x: 2 }), 3);
});

test('operators() is the documented catalogue', () => {
  const engine = new Engine();
  const docs = JSON.parse(
    readFileSync(new URL('../../../docs/src/operators/operators.json', import.meta.url)),
  );
  assert.deepEqual(engine.operators(), docs);
});

test('facts describe what a rule reads', () => {
  const engine = new Engine();
  const facts = engine.compile({ '+': [{ var: 'a.b' }, { var: 'c' }] }).facts();
  assert.deepEqual(facts.reads, [['a', 'b'], ['c']]);
  assert.deepEqual(facts.operators, ['+', 'val']);
  assert.equal(facts.reads_complete, true);
  assert.equal(facts.deterministic, true);
  assert.equal(engine.compile({ now: [] }).facts().deterministic, false);
});

test('compileTemplate and compileStrict choose the mode per compile', () => {
  const strict = new Engine();
  const templating = new Engine({ templating: true });
  const template = { user: { var: 'name' }, source: 'api' };
  assert.throws(() => strict.compile(template));
  assert.deepEqual(strict.compileTemplate(template).evaluate({ name: 'ana' }), {
    user: 'ana',
    source: 'api',
  });
  assert.throws(() => templating.compileStrict(template));
});

test('truthy follows the engine rules', () => {
  const engine = new Engine();
  assert.equal(engine.truthy({}), false);
  assert.equal(engine.truthy([]), false);
  assert.equal(engine.truthy({ a: 1 }), true);
  // A string is JSON text, as in WASM: `"0"` is the string, `[]` the array.
  assert.equal(engine.truthy('"0"'), true);
  assert.equal(engine.truthy('[]'), false);
  assert.throws(() => engine.truthy('{'));
  const python = new Engine({ config: { truthy_evaluator: 'python' } });
  assert.equal(python.truthy(0), false);
});

test('strictOperatorNames refuses a built-in name', () => {
  const op = (args) => JSON.stringify(JSON.parse(args).length);
  assert.throws(() => new Engine({ strictOperatorNames: true }, { length: op }), {
    errorType: 'ConfigurationError',
  });
  const engine = new Engine({ strictOperatorNames: true }, { count: op });
  assert.equal(engine.eval({ count: [1, 2] }, null), 2);
  // Without the option the registration is accepted (and never runs).
  assert.doesNotThrow(() => new Engine({}, { length: op }));
});

test('evaluateInt / evaluateFloat are the shared typed names', async () => {
  const { DataHandle } = await import('../index.js');
  const engine = new Engine();
  const session = engine.session();
  const data = new DataHandle('{"x": 3, "y": 1.5}');
  assert.equal(session.evaluateInt(engine.compile({ var: 'x' }), data), 3);
  assert.equal(session.evaluateFloat(engine.compile({ var: 'y' }), data), 1.5);
  assert.equal(session.evaluateNumber(engine.compile({ var: 'y' }), data), 1.5);
  assert.throws(() => session.evaluateInt(engine.compile({ var: 'y' }), data), { errorType: 'TypeMismatch' });
});
