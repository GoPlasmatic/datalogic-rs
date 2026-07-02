// Engine configuration via the `config` constructor option.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine } from '../index.js';

test('strict preset changes a result the default engine coerces', () => {
  const rule = '{"+": [1, {"var": "t"}]}';
  const data = '{"t": true}';
  // Default numeric coercion turns `true` into 1.
  assert.equal(new Engine().evalStr(rule, data), '2');
  // The strict preset rejects non-numeric operands instead.
  const strict = new Engine({ config: { preset: 'strict' } });
  assert.throws(() => strict.evalStr(rule, data), (e) => {
    assert.equal(e.name, 'EvaluateError');
    return true;
  });
});

test('safe_arithmetic preset ignores non-numeric operands', () => {
  const rule = '{"+": [1, "abc", 2]}';
  assert.throws(() => new Engine().evalStr(rule, 'null'));
  const safe = new Engine({ config: { preset: 'safe_arithmetic' } });
  assert.equal(safe.evalStr(rule, 'null'), '3');
});

test('config accepts a JSON string with nested keys', () => {
  const rule = '{"+": [1, {"var": "n"}]}';
  const data = '{"n": null}';
  // Default: null coerces to zero.
  assert.equal(new Engine().evalStr(rule, data), '1');
  const engine = new Engine({
    config: '{"numeric_coercion": {"null_to_zero": false}}',
  });
  assert.throws(() => engine.evalStr(rule, data));
});

test('individual keys override the preset they sit on', () => {
  const rule = '{"+": [1, "abc", 2]}';
  const engine = new Engine({
    config: { preset: 'strict', arithmetic_nan_handling: 'ignore_value' },
  });
  assert.equal(engine.evalStr(rule, 'null'), '3');
});

test('unknown preset throws with the core message', () => {
  assert.throws(() => new Engine({ config: { preset: 'bogus' } }), (e) => {
    assert.equal(e.errorType, 'ConfigurationError');
    assert.match(e.message, /unknown preset "bogus"/);
    return true;
  });
});

test('unknown config key throws', () => {
  assert.throws(() => new Engine({ config: { divide_by_zero: 'return_null' } }), (e) => {
    assert.equal(e.errorType, 'ConfigurationError');
    assert.match(e.message, /unknown config key "divide_by_zero"/);
    return true;
  });
});

test('malformed config JSON string throws', () => {
  assert.throws(() => new Engine({ config: '{ not json' }), (e) => {
    assert.equal(e.errorType, 'ConfigurationError');
    assert.match(e.message, /not valid JSON/);
    return true;
  });
});

test('config: null means no config, like the other optional fields', () => {
  const engine = new Engine({ config: null });
  assert.equal(engine.evalStr('{"+": [1, 2]}', 'null'), '3');
});

test('config composes with custom operators', () => {
  const engine = new Engine(
    { config: { preset: 'safe_arithmetic' } },
    { double: (a) => JSON.stringify(JSON.parse(a)[0] * 2) }
  );
  assert.equal(engine.evalStr('{"double": [21]}', 'null'), '42');
  assert.equal(engine.evalStr('{"+": [1, "abc", 2]}', 'null'), '3');
});
