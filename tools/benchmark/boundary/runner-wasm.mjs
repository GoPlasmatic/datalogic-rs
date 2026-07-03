#!/usr/bin/env node
// Boundary-benchmark runner for the WASM binding (`runtime: "wasm"`),
// @goplasmatic/datalogic-wasm loaded from the in-tree pkg build — the
// same `bindings/wasm/pkg` artifact the compare harness consumes via its
// `file:../../../bindings/wasm/pkg` npm dep (build with
// `cd bindings/wasm && ./build.sh` first; this runner requires the
// nodejs target inside the unified pkg directly, so no npm install is
// needed).
//
// Modes:
//   session-evaluate-str       engine.compile once + session.evaluate(rule, dataStr)
//   compiledrule-evaluate-str  new CompiledRule(ruleStr, false) once + .evaluate(dataStr)
//   oneshot-evaluate           evaluate(ruleStr, dataStr, false) — compile per call
//
// Emits JSON lines: {"runtime":"wasm","mode":...,"workload":...,"ns_op":...}
// Usage: node runner-wasm.mjs <workloads-dir> [--modes=a,b] [--workloads=x,y]

import { createRequire } from 'node:module';
import {
  JIT_WARMUP,
  emit,
  flushSink,
  loadWorkloads,
  measure,
  parseArgs,
  verifyStr,
} from './harness.mjs';

const RUNTIME = 'wasm';
const require = createRequire(import.meta.url);
const wasm = require('../../../bindings/wasm/pkg/nodejs/datalogic_wasm.js');
const { Engine, CompiledRule, evaluate } = wasm;

const { dir, modes, workloads } = parseArgs(process.argv.slice(2), RUNTIME);

const engine = new Engine({});

for (const w of loadWorkloads(dir)) {
  if (workloads && !workloads.includes(w.name)) continue;

  const rule = engine.compile(w.rule);
  const session = engine.session();
  const compiled = new CompiledRule(w.rule, false);

  const MODES = {
    'session-evaluate-str': {
      verify: () => verifyStr(RUNTIME, 'session-evaluate-str', w.name,
        session.evaluate(rule, w.data), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) sink += session.evaluate(rule, w.data).length;
        return sink;
      },
    },
    'compiledrule-evaluate-str': {
      verify: () => verifyStr(RUNTIME, 'compiledrule-evaluate-str', w.name,
        compiled.evaluate(w.data), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) sink += compiled.evaluate(w.data).length;
        return sink;
      },
    },
    'oneshot-evaluate': {
      verify: () => verifyStr(RUNTIME, 'oneshot-evaluate', w.name,
        evaluate(w.rule, w.data, false), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) sink += evaluate(w.rule, w.data, false).length;
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
