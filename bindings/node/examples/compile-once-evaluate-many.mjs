// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation cost.
//
// Run from bindings/node/ (build first: npm install && npx napi build --platform --release):
//   node examples/compile-once-evaluate-many.mjs

import { Engine } from '../index.js';

const ITERATIONS = 100_000;

const engine = new Engine();
const rule = engine.compile({
  '*': [{ var: 'price' }, { '-': [1, { var: 'discount' }] }],
});

let last;
const start = process.hrtime.bigint();
for (let i = 0; i < ITERATIONS; i++) {
  last = rule.evaluate({ price: 100 + (i % 100), discount: 0.2 });
}
const elapsedNs = Number(process.hrtime.bigint() - start);

console.log(`last result: ${last}`);
console.log(`${ITERATIONS} evaluations, ~${(elapsedNs / ITERATIONS).toFixed(0)} ns/op`);
