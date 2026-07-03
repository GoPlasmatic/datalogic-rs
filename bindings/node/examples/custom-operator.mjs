// custom-operator: register a JS `double` operator and call it from a rule.
// Custom operators receive their pre-evaluated arguments as a JSON-array
// string and return a JSON-value string. Built-in names always win.
//
// Run from bindings/node/ (build first: npm install && npx napi build --platform --release):
//   node examples/custom-operator.mjs

import { Engine } from '../index.js';

const engine = new Engine({}, {
  double: (argsJson) => JSON.stringify(JSON.parse(argsJson)[0] * 2),
});

console.log(engine.evalStr('{"double": [21]}', '{}')); // 42
