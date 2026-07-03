// custom-operator: register a JS `double` operator on an Engine and call
// it from a rule. Custom operators receive their pre-evaluated arguments
// as a JSON-array string and return a JSON-value string. Built-in names
// always win.
//
// Run from bindings/wasm/ (build first: ./build.sh):
//   node examples/custom-operator.mjs

import { Engine } from '../pkg/nodejs/datalogic_wasm.js';

const engine = new Engine({
  customOperators: {
    double: (argsJson) => JSON.stringify(JSON.parse(argsJson)[0] * 2),
  },
});

console.log(engine.evalStr('{"double": [21]}', '{}')); // 42
