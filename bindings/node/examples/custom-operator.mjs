// custom-operator: register a JS `double` operator and call it from a rule.
// Custom operators receive their pre-evaluated arguments as a JSON-array
// string and return a JSON-value string. Built-in names always win.
//
// Run from bindings/node/ (build first: npm install && npx napi build --platform --release):
//   node examples/custom-operator.mjs

import { Engine } from '../index.js';

const engine = new Engine({}, {
  double: (argsJson) => {
    const args = JSON.parse(argsJson);
    if (args.length === 0) throw new Error('double expects one numeric argument');
    return JSON.stringify(args[0] * 2);
  },
});

console.log(engine.evalStr('{"double": [21]}', '{}')); // 42

// Custom operators compose with built-ins.
console.log(engine.evalStr(
  '{"map": [{"var": "xs"}, {"double": [{"var": ""}]}]}',
  '{"xs": [1, 2, 3]}',
)); // [2,4,6]

// The operator's error path surfaces as a regular EvaluateError.
try {
  engine.evalStr('{"double": []}', '{}');
} catch (e) {
  console.log(`${e.name}: ${e.message}`); // EvaluateError: ... double expects one numeric argument
}
