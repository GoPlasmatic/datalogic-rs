#!/usr/bin/env node
// Boundary-benchmark runner for the Node binding (`runtime: "node"`),
// @goplasmatic/datalogic-node loaded from the in-tree build
// (`cd bindings/node && npx napi build --platform --release` first).
//
// Modes:
//   session-evaluateStr-str        session.evaluateStr(rule, dataStr) — hot path
//   rule-evaluateStr-str           rule.evaluateStr(dataStr)
//   rule-evaluate-obj              rule.evaluate(dataObj) — the object path
//   stringify-str-parse-roundtrip  JSON.parse(rule.evaluateStr(JSON.stringify(obj)))
//   engine-eval-oneshot            engine.evalStr(ruleStr, dataStr) — compile per call
//
// Emits JSON lines: {"runtime":"node","mode":...,"workload":...,"ns_op":...}
// Usage: node runner-node.mjs <workloads-dir> [--modes=a,b] [--workloads=x,y]

import { createRequire } from 'node:module';
import {
  JIT_WARMUP,
  emit,
  flushSink,
  loadWorkloads,
  measure,
  parseArgs,
  verifyDeep,
  verifyStr,
} from './harness.mjs';

const RUNTIME = 'node';
const require = createRequire(import.meta.url);
// The napi build drops index.js + the platform .node next to the binding
// crate; requiring it directly avoids an npm-install step.
const { Engine } = require('../../../bindings/node/index.js');

const { dir, modes, workloads } = parseArgs(process.argv.slice(2), RUNTIME);

const engine = new Engine();

for (const w of loadWorkloads(dir)) {
  if (workloads && !workloads.includes(w.name)) continue;

  const rule = engine.compile(w.rule);
  const session = engine.session();
  // Single hot object identity across the whole run, matching the
  // documented capture (perfectly warm shape/inline caches).
  const dataObj = JSON.parse(w.data);
  const ruleObj = JSON.parse(w.rule);

  const MODES = {
    'session-evaluateStr-str': {
      verify: () => verifyStr(RUNTIME, 'session-evaluateStr-str', w.name,
        session.evaluateStr(rule, w.data), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) sink += session.evaluateStr(rule, w.data).length;
        return sink;
      },
    },
    'rule-evaluateStr-str': {
      verify: () => verifyStr(RUNTIME, 'rule-evaluateStr-str', w.name,
        rule.evaluateStr(w.data), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) sink += rule.evaluateStr(w.data).length;
        return sink;
      },
    },
    'rule-evaluate-obj': {
      verify: () => verifyDeep(RUNTIME, 'rule-evaluate-obj', w.name,
        rule.evaluate(dataObj), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) {
          const res = rule.evaluate(dataObj);
          sink += res === null ? 0 : 1;
        }
        return sink;
      },
    },
    'stringify-str-parse-roundtrip': {
      verify: () => verifyDeep(RUNTIME, 'stringify-str-parse-roundtrip', w.name,
        JSON.parse(rule.evaluateStr(JSON.stringify(dataObj))), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) {
          const res = JSON.parse(rule.evaluateStr(JSON.stringify(dataObj)));
          sink += res === null ? 0 : 1;
        }
        return sink;
      },
    },
    // The convenience tier the way JS callers naturally hold it: object
    // rule and object data, compiled per call, object result. (String
    // in/out one-shots measure noticeably faster — the object shape is
    // what the documented capture reflects.)
    'engine-eval-oneshot': {
      verify: () => verifyDeep(RUNTIME, 'engine-eval-oneshot', w.name,
        engine.eval(ruleObj, dataObj), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) {
          const res = engine.eval(ruleObj, dataObj);
          sink += res === null ? 0 : 1;
        }
        return sink;
      },
    },
  };

  for (const [mode, spec] of Object.entries(MODES)) {
    if (modes && !modes.includes(mode)) continue;
    spec.verify();
    emit(RUNTIME, mode, w.name, measure(spec.batch, JIT_WARMUP));
  }
}

flushSink(RUNTIME);
