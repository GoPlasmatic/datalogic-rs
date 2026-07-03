// getting-started: one-shot JSONLogic evaluation via the WASM build's
// nodejs target (no init() call needed under Node), plus the parse-once
// DataHandle tier.
//
// Run from bindings/wasm/ (build first: ./build.sh):
//   node examples/getting-started.mjs

import { CompiledRule, DataHandle, evaluate } from '../pkg/nodejs/datalogic_wasm.js';

const rule = JSON.stringify({
  and: [
    { '>=': [{ var: 'age' }, 18] },
    { '==': [{ var: 'status' }, 'active'] },
  ],
});
const data = JSON.stringify({ age: 25, status: 'active' });

console.log(evaluate(rule, data, false)); // true

// Parse-once data handle: the payload is parsed a single time and stays
// resident in WASM memory, so evaluations against it skip the per-call
// copy + re-parse that the string path pays. One handle can feed any
// number of rules.
const handle = new DataHandle(data);
const drinking = new CompiledRule(JSON.stringify({ '>=': [{ var: 'age' }, 21] }), false);
const senior = new CompiledRule(JSON.stringify({ '>=': [{ var: 'age' }, 65] }), false);
console.log(drinking.evaluateData(handle)); // true
console.log(senior.evaluateData(handle)); // false
handle.free(); // release the resident copy after the last evaluation
