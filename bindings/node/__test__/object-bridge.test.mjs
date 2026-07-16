// Object-path ↔ string-path equivalence.
//
// `rule.evaluate(obj)` crosses the boundary through the napi serde
// bridge (JS object → serde_json::Value → arena) while
// `rule.evaluateStr(jsonText)` parses the JSON text directly. Both must
// produce deep-equal results — this is the regression gate any future
// object-converter change (the "B6" tier) has to pass.
//
// The corpus is (a) a hand-built edge battery covering the tricky
// conversion dimensions (unicode keys/strings, int vs float, null/
// bools, empty containers, nesting, undefined handling) and (b) a
// sweep of JSONLogic conformance suites from
// crates/datalogic-rs/tests/suites, replayed through both paths.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { Engine } from '../index.js';

const SUITES_DIR = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../crates/datalogic-rs/tests/suites',
);

// Representative slice of the conformance battery: arithmetic,
// comparison, iteration, control flow, strings, truthiness, structured
// output, and scoped variables.
const SUITE_FILES = [
  'arithmetic/plus.json',
  'arithmetic/multiply.json',
  'arithmetic/divide.json',
  'arithmetic/chain.json',
  'comparison/softEquals.json',
  'comparison/strictEquals.json',
  'comparison/greaterThan.json',
  'array/map.json',
  'array/merge.json',
  'array/reduce.json',
  'control/if.json',
  'control/and.json',
  'control/or.json',
  'string/string.json',
  'truthiness.json',
  'coalesce.json',
  'val.json',
];

const engine = new Engine();

// Known, documented divergence of the current serde bridge: JS objects
// arrive through a sorted map (BTreeMap keys), while JSON text
// preserves document order. Rules that *iterate* an object therefore
// see entries in a different order on the two paths. These cases are
// compared as multisets (sorted) instead of ordered arrays.
const OBJECT_ITERATION_ORDER_CASES = new Set([
  'array/map.json|Object iteration - extract values from data context',
  'array/map.json|Object iteration - create key-value string pairs using scope traversal',
  'array/map.json|Conditionally mapping object fields - show or hide sensitive data',
]);

/**
 * The object path preserves full 64-bit integer precision by returning
 * BigInt for values outside Number's safe range, while the string path
 * (`JSON.parse`) rounds them to the nearest double. Collapse BigInts to
 * Numbers so the comparison checks semantic agreement at double
 * precision — the most the JSON text path can represent.
 */
function collapseBigInts(value) {
  if (typeof value === 'bigint') return Number(value);
  if (Array.isArray(value)) return value.map(collapseBigInts);
  if (value !== null && typeof value === 'object') {
    const out = {};
    for (const [k, v] of Object.entries(value)) out[k] = collapseBigInts(v);
    return out;
  }
  return value;
}

/**
 * Evaluate one case through both paths and compare.
 *
 * The binding's dual-input convention treats a JS string argument as
 * JSON text, so string-typed rules/data are passed JSON-encoded (both
 * paths then see the identical document).
 */
function assertPathsAgree(rule, data, label, { orderInsensitive = false } = {}) {
  const ruleArg = typeof rule === 'string' ? JSON.stringify(rule) : rule;
  let compiled;
  try {
    compiled = engine.compile(ruleArg);
  } catch {
    return { skipped: true }; // uncompilable under default config — nothing to compare
  }

  const dataArg = typeof data === 'string' ? JSON.stringify(data) : data;

  let objResult;
  let objErr = null;
  try {
    objResult = collapseBigInts(compiled.evaluate(dataArg));
  } catch (e) {
    objErr = e;
  }

  let strResult;
  let strErr = null;
  try {
    strResult = JSON.parse(compiled.evaluateStr(JSON.stringify(data === undefined ? null : data)));
  } catch (e) {
    strErr = e;
  }

  if (objErr || strErr) {
    assert.ok(
      objErr && strErr,
      `${label}: one path threw and the other did not ` +
        `(obj: ${objErr?.message ?? 'ok'}, str: ${strErr?.message ?? 'ok'})`,
    );
    return { threw: true };
  }
  if (orderInsensitive && Array.isArray(objResult) && Array.isArray(strResult)) {
    const canon = (arr) => arr.map((v) => JSON.stringify(v)).sort();
    assert.deepEqual(canon(objResult), canon(strResult), `${label}: paths disagree (as multisets)`);
    return {};
  }
  assert.deepEqual(objResult, strResult, `${label}: object and string paths disagree`);
  return {};
}

test('edge corpus: object path matches string path', () => {
  const echo = { var: '' };
  const cases = [
    // unicode keys and strings
    { 'ключ': 'значение', '日本語': ['α', 'β'], emoji: '🎉🎊' },
    // int vs float
    { i: 3, f: 2.5, negi: -7, negf: -0.25, zero: 0, big: 4294967295, beyond: 5000000000 },
    // null / bools
    { t: true, f: false, n: null },
    // empty containers
    { obj: {}, arr: [], nested: { deep: { empty: [] } } },
    // deep nesting and mixed arrays
    { a: [1, [2, [3, [4, 'five', null, true]]]], o: { x: { y: { z: [0.5] } } } },
    // scalars at the top level
    42, 2.5, true, false, null, [1, 2, 3], 'plain string',
  ];
  for (const data of cases) {
    assertPathsAgree(echo, data, `echo ${JSON.stringify(data)?.slice(0, 40)}`);
  }

  // undefined handling: object properties are dropped (JSON.stringify
  // drops them too, so both paths agree)...
  assertPathsAgree(echo, Object.assign({ b: 1 }, { a: undefined }), 'undefined property');
  // ...while undefined array elements throw on the object path only —
  // pin that this stays a throw rather than silently diverging.
  const compiled = engine.compile(echo);
  assert.throws(() => compiled.evaluate([1, undefined, 2]), /undefined cannot be represented/);
});

test('conformance suites: object path matches string path', () => {
  let compared = 0;
  let threw = 0;
  let skipped = 0;
  for (const file of SUITE_FILES) {
    const cases = JSON.parse(readFileSync(join(SUITES_DIR, file), 'utf-8'));
    for (const c of cases) {
      if (typeof c === 'string') continue; // section header
      const outcome = assertPathsAgree(
        c.rule,
        c.data === undefined ? null : c.data,
        `${file}: ${c.description ?? JSON.stringify(c.rule)?.slice(0, 60)}`,
        { orderInsensitive: OBJECT_ITERATION_ORDER_CASES.has(`${file}|${c.description}`) },
      );
      if (outcome.skipped) skipped++;
      else if (outcome.threw) threw++;
      else compared++;
    }
  }
  // Guard against the corpus silently rotting away.
  assert.ok(compared > 300, `expected a substantial corpus, compared only ${compared}`);
  // (threw + skipped are legitimate: error-expected cases and rules
  // that need non-default engine config.)
});
