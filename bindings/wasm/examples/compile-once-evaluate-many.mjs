// compile-once-evaluate-many: compile a rule once via CompiledRule,
// evaluate ~100k payloads, and print the last result plus a rough
// per-evaluation cost — then do the same through a parse-once
// DataHandle to show what dropping the per-call data copy + parse buys.
//
// Run from bindings/wasm/ (build first: ./build.sh):
//   node examples/compile-once-evaluate-many.mjs

import { CompiledRule, DataHandle } from '../pkg/nodejs/datalogic_wasm.js';

const ITERATIONS = 100_000;

const rule = new CompiledRule(
  '{"*": [{"var": "price"}, {"-": [1, {"var": "discount"}]}]}',
  false,
);

// Phase 1: string path — every call ships the data JSON across the
// JS↔WASM boundary and re-parses it inside the module.
let last;
let start = process.hrtime.bigint();
for (let i = 0; i < ITERATIONS; i++) {
  last = rule.evaluate(`{"price": ${100 + (i % 100)}, "discount": 0.2}`);
}
const stringNs = Number(process.hrtime.bigint() - start);

console.log(`last result: ${last}`);
console.log(`${ITERATIONS} evaluations, ~${(stringNs / ITERATIONS).toFixed(0)} ns/op (string data)`);

// Phase 2: handle path — when the same payload is evaluated repeatedly
// (pricing rules against one quote, feature flags against one context),
// parse it once into a DataHandle. Only the rule dispatch and the small
// result cross the boundary per call.
const handle = new DataHandle('{"price": 100, "discount": 0.2}');
start = process.hrtime.bigint();
for (let i = 0; i < ITERATIONS; i++) {
  last = rule.evaluateData(handle);
}
const handleNs = Number(process.hrtime.bigint() - start);
handle.free();

console.log(`last result: ${last}`);
console.log(`${ITERATIONS} evaluations, ~${(handleNs / ITERATIONS).toFixed(0)} ns/op (DataHandle)`);
