// compile-once-evaluate-many: compile a rule once via CompiledRule,
// evaluate ~100k payloads, and print the last result plus a rough
// per-evaluation cost.
//
// Run from bindings/wasm/ (build first: ./build.sh):
//   node examples/compile-once-evaluate-many.mjs

import { CompiledRule } from '../pkg/nodejs/datalogic_wasm.js';

const ITERATIONS = 100_000;

const rule = new CompiledRule(
  '{"*": [{"var": "price"}, {"-": [1, {"var": "discount"}]}]}',
  false,
);

let last;
const start = process.hrtime.bigint();
for (let i = 0; i < ITERATIONS; i++) {
  last = rule.evaluate(`{"price": ${100 + (i % 100)}, "discount": 0.2}`);
}
const elapsedNs = Number(process.hrtime.bigint() - start);

console.log(`last result: ${last}`);
console.log(`${ITERATIONS} evaluations, ~${(elapsedNs / ITERATIONS).toFixed(0)} ns/op`);
