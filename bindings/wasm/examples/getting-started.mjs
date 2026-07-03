// getting-started: one-shot JSONLogic evaluation via the WASM build's
// nodejs target (no init() call needed under Node).
//
// Run from bindings/wasm/ (build first: ./build.sh):
//   node examples/getting-started.mjs

import { evaluate } from '../pkg/nodejs/datalogic_wasm.js';

const rule = JSON.stringify({
  and: [
    { '>=': [{ var: 'age' }, 18] },
    { '==': [{ var: 'status' }, 'active'] },
  ],
});
const data = JSON.stringify({ age: 25, status: 'active' });

console.log(evaluate(rule, data, false)); // true
