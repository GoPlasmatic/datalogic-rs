// getting-started: one-shot JSONLogic evaluation.
//
// Run from bindings/node/ (build first: npm install && npx napi build --platform --release):
//   node examples/getting-started.mjs

import { apply } from '../index.js';

const rule = {
  and: [
    { '>=': [{ var: 'age' }, 18] },
    { '==': [{ var: 'status' }, 'active'] },
  ],
};
const data = { age: 25, status: 'active' };

console.log(apply(rule, data)); // true
