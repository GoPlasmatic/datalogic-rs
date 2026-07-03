// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation
// cost — first via JS objects, then via pre-parsed data handles (the
// hot path: zero parse work per call), and finally as one batch call.
//
// Run from bindings/node/ (build first: npm install && npx napi build --platform --release):
//   node examples/compile-once-evaluate-many.mjs

import { Engine, DataHandle } from '../index.js';

const ITERATIONS = 100_000;

const engine = new Engine();
const rule = engine.compile({
  '*': [{ var: 'price' }, { '-': [1, { var: 'discount' }] }],
});

// Tier 1 — compiled rule, JS-object data (converted per call).
let last;
let start = process.hrtime.bigint();
for (let i = 0; i < ITERATIONS; i++) {
  last = rule.evaluate({ price: 100 + (i % 100), discount: 0.2 });
}
let elapsedNs = Number(process.hrtime.bigint() - start);
console.log(`object data:  last result ${last}, ${ITERATIONS} evaluations, ~${(elapsedNs / ITERATIONS).toFixed(0)} ns/op`);

// Tier 2 — session + pre-parsed data handles: parse each distinct
// payload once, then every evaluation skips JSON parsing entirely.
const handles = Array.from({ length: 100 }, (_, i) =>
  new DataHandle(JSON.stringify({ price: 100 + i, discount: 0.2 })),
);
const session = engine.session();

start = process.hrtime.bigint();
for (let i = 0; i < ITERATIONS; i++) {
  last = session.evaluateDataStr(rule, handles[i % 100]);
}
elapsedNs = Number(process.hrtime.bigint() - start);
console.log(`data handles: last result ${last}, ${ITERATIONS} evaluations, ~${(elapsedNs / ITERATIONS).toFixed(0)} ns/op`);

// Tier 3 — one native call for the whole set: per-item results (and
// per-item errors) come back in order, Promise.allSettled-style.
const results = session.evaluateBatch(rule, handles);
console.log(`batch:        ${results.length} results in one call, first ${results[0].value}, last ${results[results.length - 1].value}`);
