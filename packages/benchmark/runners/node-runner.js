#!/usr/bin/env node
//
// Node subprocess harness for cross-library JSONLogic benchmarks. Spawned
// once per (subject, suite) cell by `packages/benchmark/src/bin/compare.rs`.
//
// Reads JSON from stdin:
//   { library: "<npm-name>", target_ms: 200, samples: 3, cases: [...] }
//
// Each case has both pre-parsed and raw shapes:
//   { rule: <obj>, data: <val>, rule_str: "<json>", data_str: "<json>" }
// Libraries pick whichever they prefer (apply() takes parsed objects;
// our wasm `evaluate` takes JSON strings).
//
// Writes one JSON line to stdout — the median sample by elapsed time:
//   { elapsed_ns, iterations, ok_count, err_count }
//
// Adding a JS / WASM library is a one-entry edit to LIBS below.

import { readFileSync } from 'node:fs';

// Dispatch table — `setup(cases)` is async (can `await import()` and
// pre-compile per case), returns a callable `apply(case)` that throws on
// error and returns the result on success. Both error paths are equally
// cheap from JS's perspective; the outer loop tracks ok/err counts
// uniformly. `setup` may attach state to case objects (e.g. compiled
// functions) — cases are owned by this run, so mutation is fine.
const LIBS = {
  'json-logic-js': {
    setup: async () => {
      const mod = await import('json-logic-js');
      const jsonLogic = mod.default ?? mod;
      // jsonLogic.apply(rule, data) takes parsed JS objects.
      return (c) => jsonLogic.apply(c.rule, c.data);
    },
  },
  'json-logic-engine': {
    // Interpreted (no pre-compile) — fair compare to `json-logic-js` and
    // `dlrs:string` (in the sense that no caller-side compilation step
    // is taken; rule traversal happens inside `run`).
    setup: async () => {
      const { LogicEngine } = await import('json-logic-engine');
      const engine = new LogicEngine();
      return (c) => engine.run(c.rule, c.data);
    },
  },
  'json-logic-engine-compiled': {
    // Pre-compiled — fair compare to `dlrs:engine` and `dlrs:session`.
    // `engine.build(rule)` returns a JS function closed over the rule;
    // calling it with data alone runs the optimised path.
    setup: async (cases) => {
      const { LogicEngine } = await import('json-logic-engine');
      const engine = new LogicEngine();
      for (const c of cases) {
        c._jle_compiled = engine.build(c.rule);
      }
      return (c) => c._jle_compiled(c.data);
    },
  },
  '@goplasmatic/datalogic-compiled': {
    // Pre-compiled via the WASM-side `CompiledRule` class. Cuts rule
    // parse + compile + rule-string marshalling out of the per-call
    // path; data marshalling + parse + result stringify still happen
    // every call (realistic — callers do reuse rules but pass fresh
    // data). The WASM analog of `dlrs:engine`.
    setup: async (cases) => {
      const mod = await import('@goplasmatic/datalogic');
      const CompiledRule = mod.CompiledRule ?? mod.default?.CompiledRule;
      if (typeof CompiledRule !== 'function') {
        throw new Error(
          '@goplasmatic/datalogic: `CompiledRule` not found on module. Build pkg/ via `cd packages/wasm && ./build.sh`.',
        );
      }
      for (const c of cases) {
        c._wasm_compiled = new CompiledRule(c.rule_str, false);
      }
      return (c) => c._wasm_compiled.evaluate(c.data_str);
    },
  },
};

function pickIterations(pilotNs, targetMs) {
  if (pilotNs <= 0) return 1_000_000;
  const targetNs = targetMs * 1e6;
  const ratio = targetNs / pilotNs;
  return Math.max(1, Math.floor(ratio));
}

/** Median sample by elapsed time. Caller passes 3 samples → middle one. */
function median(samples) {
  const sorted = [...samples].sort((a, b) => a.elapsed_ns - b.elapsed_ns);
  return sorted[Math.floor(sorted.length / 2)];
}

async function main() {
  const input = JSON.parse(readFileSync(0, 'utf-8'));
  const { library, target_ms, samples: nSamples, cases } = input;

  const spec = LIBS[library];
  if (!spec) {
    process.stderr.write(`unknown library: ${library}\n`);
    process.exit(1);
  }
  const apply = await spec.setup(cases);

  // Pilot — one pass over all cases serves as both warm-up (so V8
  // optimises the call sites) and per-op cost estimate.
  const pilotStart = process.hrtime.bigint();
  for (const c of cases) {
    try {
      apply(c);
    } catch {
      // Errors during pilot are fine — they exercise the same code path
      // the timed loop will hit.
    }
  }
  const pilotNs = Number(process.hrtime.bigint() - pilotStart);
  const iterations = pickIterations(pilotNs, target_ms);

  const samples = [];
  for (let s = 0; s < nSamples; s++) {
    let ok = 0;
    let err = 0;
    const start = process.hrtime.bigint();
    for (let i = 0; i < iterations; i++) {
      for (const c of cases) {
        try {
          apply(c);
          ok++;
        } catch {
          err++;
        }
      }
    }
    const elapsed_ns = Number(process.hrtime.bigint() - start);
    samples.push({ elapsed_ns, iterations, ok_count: ok, err_count: err });
  }

  const m = median(samples);
  process.stdout.write(JSON.stringify(m) + '\n');
}

main().catch((e) => {
  // Some libraries throw plain objects (no `.message`, no `.stack`),
  // which would coerce to `[object Object]`. Fall through to a JSON
  // dump so the surfaced error is at least inspectable.
  const msg =
    e?.stack ?? e?.message ?? (typeof e === 'object' ? JSON.stringify(e) : String(e));
  process.stderr.write(`runner error: ${msg}\n`);
  process.exit(1);
});
