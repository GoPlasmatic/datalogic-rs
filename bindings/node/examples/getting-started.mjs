// getting-started: one-shot JSONLogic evaluation, plus the typed-result
// tier for boolean predicates and the async string tier.
//
// Run from bindings/node/ (build first: npm install && npx napi build --platform --release):
//   node examples/getting-started.mjs

import { apply, Engine, DataHandle } from '../index.js';

const rule = {
  and: [
    { '>=': [{ var: 'age' }, 18] },
    { '==': [{ var: 'status' }, 'active'] },
  ],
};
const data = { age: 25, status: 'active' };

// One-shot: compile + evaluate in a single call.
console.log(apply(rule, data)); // true

// Typed result: for predicates, skip the JSON result round trip.
// Compile the rule, parse the data once into a handle, and read the
// result directly as a boolean.
const engine = new Engine();
const compiled = engine.compile(rule);
const parsed = new DataHandle(JSON.stringify(data));

const session = engine.session();
console.log(session.evaluateBool(compiled, parsed)); // true

// Async: evaluate on the libuv pool — same result, off the event loop.
console.log(await compiled.evaluateStrAsync(JSON.stringify(data))); // true
