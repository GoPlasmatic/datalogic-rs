// Custom operator registration on the `Engine` class — JSON-string in/out.

import test from 'node:test';
import assert from 'node:assert/strict';
import { Engine } from '../index.js';

test('engine without custom operators behaves like before', () => {
  const engine = new Engine();
  assert.equal(engine.evalStr('{"+": [1, 2]}', '{}'), '3');
});

test('engine ignores empty customOperators map', () => {
  const engine = new Engine({}, {});
  assert.equal(engine.evalStr('{"+": [1, 2]}', '{}'), '3');
});

test('custom operator returns scalar', () => {
  const engine = new Engine({}, {
    double: (argsJson) => {
      const [n] = JSON.parse(argsJson);
      return JSON.stringify(n * 2);
    },
  });
  assert.equal(engine.evalStr('{"double": [21]}', '{}'), '42');
});

test('custom operator returns string', () => {
  const engine = new Engine({}, {
    upper: (argsJson) => {
      const [s] = JSON.parse(argsJson);
      return JSON.stringify(s.toUpperCase());
    },
  });
  assert.equal(engine.evalStr('{"upper": ["hello"]}', '{}'), '"HELLO"');
});

test('custom operator returns object', () => {
  const engine = new Engine({}, {
    wrap: (argsJson) => {
      const [value] = JSON.parse(argsJson);
      return JSON.stringify({ value });
    },
  });
  const result = engine.eval({ wrap: ['hi'] }, {});
  assert.deepEqual(result, { value: 'hi' });
});

test('custom operator returns array', () => {
  const engine = new Engine({}, {
    repeat: (argsJson) => {
      const [s, n] = JSON.parse(argsJson);
      return JSON.stringify(Array.from({ length: n }, () => s));
    },
  });
  assert.deepEqual(engine.eval({ repeat: ['x', 3] }, {}), ['x', 'x', 'x']);
});

test('custom operator composes with built-ins', () => {
  const engine = new Engine({}, {
    double: (argsJson) => {
      const [n] = JSON.parse(argsJson);
      return JSON.stringify(n * 2);
    },
  });
  // map every element through `double`
  const rule = { map: [{ var: 'xs' }, { double: [{ var: '' }] }] };
  assert.deepEqual(engine.eval(rule, { xs: [1, 2, 3] }), [2, 4, 6]);
});

test('custom operator with multiple args', () => {
  const engine = new Engine({}, {
    clamp: (argsJson) => {
      const [v, lo, hi] = JSON.parse(argsJson);
      return JSON.stringify(Math.max(lo, Math.min(hi, v)));
    },
  });
  assert.equal(engine.evalStr('{"clamp": [5, 0, 3]}', '{}'), '3');
  assert.equal(engine.evalStr('{"clamp": [-5, 0, 3]}', '{}'), '0');
  assert.equal(engine.evalStr('{"clamp": [2, 0, 3]}', '{}'), '2');
});

test('two different custom operators on one engine', () => {
  const engine = new Engine({}, {
    inc: (a) => {
      const [n] = JSON.parse(a);
      return JSON.stringify(n + 1);
    },
    neg: (a) => {
      const [n] = JSON.parse(a);
      return JSON.stringify(-n);
    },
  });
  assert.equal(engine.evalStr('{"inc": [{"neg": [3]}]}', '{}'), '-2');
});

test('built-ins win over a custom operator with the same name', () => {
  const engine = new Engine({}, {
    '+': () => JSON.stringify('hijacked'),
  });
  // Built-in `+` should still apply — the custom registration is shadowed.
  assert.equal(engine.evalStr('{"+": [1, 2]}', '{}'), '3');
});

test('custom operator that throws bubbles as an error', () => {
  const engine = new Engine({}, {
    boom: () => {
      throw new Error('kaboom');
    },
  });
  assert.throws(() => engine.evalStr('{"boom": []}', '{}'), /kaboom|custom operator/);
});

test('custom operator returning invalid JSON is an error', () => {
  const engine = new Engine({}, {
    bad: () => 'not a json value',
  });
  assert.throws(() => engine.evalStr('{"bad": []}', '{}'), /invalid JSON|custom operator/);
});

test('compiled rule re-uses engine custom operators', () => {
  const engine = new Engine({}, {
    add5: (a) => {
      const [n] = JSON.parse(a);
      return JSON.stringify(n + 5);
    },
  });
  const rule = engine.compile('{"add5": [{"var": "x"}]}');
  assert.equal(rule.evaluate({ x: 10 }), 15);
  assert.equal(rule.evaluate({ x: 100 }), 105);
});
