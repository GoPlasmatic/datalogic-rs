// Operator introspection: `builtinOperatorNames()` and
// `Engine.customOperatorNames()`.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine, builtinOperatorNames } from '../index.js';

test('builtinOperatorNames lists canonical names before aliases', () => {
  const names = builtinOperatorNames();
  assert.ok(Array.isArray(names));
  for (const expected of ['val', 'var', 'if', '?:', 'switch', 'match', 'sem_ver', 'fractional', 'now']) {
    assert.ok(names.includes(expected), `missing ${expected}`);
  }
  assert.ok(names.indexOf('val') < names.indexOf('var'));
  assert.ok(names.indexOf('switch') < names.indexOf('match'));
});

test('customOperatorNames reflects registered custom operators', () => {
  const plain = new Engine();
  assert.deepEqual(plain.customOperatorNames(), []);
  const custom = new Engine(undefined, { double: (args) => String(JSON.parse(args)[0] * 2) });
  assert.deepEqual(custom.customOperatorNames(), ['double']);
  assert.ok(!builtinOperatorNames().includes('double'));
});
