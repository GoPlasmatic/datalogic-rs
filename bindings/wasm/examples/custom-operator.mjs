// custom-operator: register a JS `double` operator on an Engine and call
// it from a rule. Custom operators receive their pre-evaluated arguments
// as a JSON-array string and return a JSON-value string. Built-in names
// always win. The second half runs a rule set (including the custom
// operator) against one parse-once DataHandle in a single batch call.
//
// Run from bindings/wasm/ (build first: ./build.sh):
//   node examples/custom-operator.mjs

import { DataHandle, Engine } from '../pkg/nodejs/datalogic_wasm.js';

const engine = new Engine({
  customOperators: {
    double: (argsJson) => JSON.stringify(JSON.parse(argsJson)[0] * 2),
  },
});

console.log(engine.evalStr('{"double": [21]}', '{}')); // 42

// Rule-set shape: many rules, one payload, one boundary call. Item
// failures come back as allSettled-style rejections instead of throwing,
// so one bad rule never takes down the batch.
const session = engine.session();
const handle = new DataHandle('{"n": 21}');
const rules = [
  engine.compile('{"double": [{"var": "n"}]}'),
  engine.compile('{"throw": "boom"}'), // fails per-item, not per-call
  engine.compile('{"+": [{"var": "n"}, 1]}'),
];

for (const outcome of session.evaluateMany(rules, handle)) {
  console.log(
    outcome.status === 'fulfilled'
      ? `ok: ${outcome.value}`
      : `err: ${outcome.reason.tag} (${outcome.reason.message})`,
  );
}
// ok: 42
// err: Thrown (...)
// ok: 22
handle.free();
