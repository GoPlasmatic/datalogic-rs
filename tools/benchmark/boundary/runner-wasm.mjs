#!/usr/bin/env node
// Boundary-benchmark runner for the WASM binding (`runtime: "wasm"`),
// @goplasmatic/datalogic-wasm loaded from the in-tree pkg build — the
// same `bindings/wasm/pkg` artifact the compare harness consumes via its
// `file:../../../bindings/wasm/pkg` npm dep (build with
// `cd bindings/wasm && ./build.sh` first; this runner requires the
// nodejs target inside the unified pkg directly, so no npm install is
// needed). Set DLRS_WASM_PKG to an alternate pkg directory to measure a
// different build variant (e.g. the opt-in `WASM_PROFILE=speed` build)
// without touching the default artifact.
//
// Modes:
//   session-evaluate-str       engine.compile once + session.evaluate(rule, dataStr)
//   session-evaluate-data      same, against a parse-once DataHandle (v2 hot path)
//   session-evaluate-many-100  one session.evaluateMany call over 100
//                              separately-compiled copies of the workload
//                              rule × one DataHandle (ns_op = call/100)
//   compiledrule-evaluate-str  new CompiledRule(ruleStr, false) once + .evaluate(dataStr)
//   oneshot-evaluate           evaluate(ruleStr, dataStr, false) — compile per call
//
// Emits JSON lines: {"runtime":"wasm","mode":...,"workload":...,"ns_op":...}
// Usage: node runner-wasm.mjs <workloads-dir> [--modes=a,b] [--workloads=x,y]

import { createRequire } from 'node:module';
import { join } from 'node:path';
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
const MANY_N = 100;
const require = createRequire(import.meta.url);
const PKG_DIR = process.env.DLRS_WASM_PKG ?? '../../../bindings/wasm/pkg';
const wasm = require(join(PKG_DIR, 'nodejs/datalogic_wasm.js'));
const { Engine, CompiledRule, DataHandle, evaluate } = wasm;

const { dir, modes, workloads } = parseArgs(process.argv.slice(2), RUNTIME);

const engine = new Engine({});

for (const w of loadWorkloads(dir)) {
  if (workloads && !workloads.includes(w.name)) continue;

  const rule = engine.compile(w.rule);
  const session = engine.session();
  const compiled = new CompiledRule(w.rule, false);

  // v2: parse-once data handle.
  const dataHandle = new DataHandle(w.data);

  // 100 identical rules, compiled separately (a rule-set of identical
  // rules — separate compiles so the batch doesn't flatter one hot
  // compiled tree). Mirrors runner-go.
  const manyRules = Array.from({ length: MANY_N }, () => engine.compile(w.rule));

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
    'session-evaluate-data': {
      verify: () => verifyStr(RUNTIME, 'session-evaluate-data', w.name,
        session.evaluateData(rule, dataHandle), w.expected),
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) sink += session.evaluateData(rule, dataHandle).length;
        return sink;
      },
    },
    'session-evaluate-many-100': {
      verify: () => {
        const results = session.evaluateMany(manyRules, dataHandle);
        for (const r of results) {
          if (r.status !== 'fulfilled') {
            process.stderr.write(
              `${RUNTIME}: evaluateMany item rejected for workload=${w.name}: ` +
                `${JSON.stringify(r.reason)}\n`,
            );
            process.exit(1);
          }
          verifyStr(RUNTIME, 'session-evaluate-many-100', w.name, r.value, w.expected);
        }
      },
      batch: (n) => {
        let sink = 0;
        for (let i = 0; i < n; i++) {
          const results = session.evaluateMany(manyRules, dataHandle);
          sink += results[0].value.length + results[MANY_N - 1].value.length;
        }
        return sink;
      },
      // ns_op is reported per evaluation: call/100 (mirrors runner-go).
      perCallEvals: MANY_N,
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
    emit(RUNTIME, mode, w.name, measure(spec.batch, JIT_WARMUP) / (spec.perCallEvals ?? 1));
  }
}

flushSink(RUNTIME);
