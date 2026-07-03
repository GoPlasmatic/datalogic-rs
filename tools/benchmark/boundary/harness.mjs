// Shared timing harness for the JS-hosted boundary runners
// (runner-node.mjs, runner-wasm.mjs). Implements the discipline from
// BINDINGS-OVERHEAD.md's appendix: warmup (5,000 iterations on JIT
// runtimes), pilot pass sizing N so one timed sample lands near ~250 ms,
// median of 5 samples, results consumed via a sink.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

export const JIT_WARMUP = 5000;
const TARGET_SAMPLE_NS = 250e6;
const PILOT_MIN_NS = 10e6;
const SAMPLES = 5;

// Results are consumed into this accumulator; printed at exit so the
// work feeding it stays observable. (Native/wasm calls are opaque to the
// JIT anyway; the sink is belt and braces.)
let globalSink = 0;

/**
 * `batch(n)` must run n iterations and return a number derived from the
 * results (e.g. accumulated string lengths). Returns median ns/op.
 */
export function measure(batch, warmup = JIT_WARMUP) {
  globalSink += batch(warmup);

  let n = 32;
  let perOp;
  for (;;) {
    const t0 = process.hrtime.bigint();
    globalSink += batch(n);
    const elapsed = Number(process.hrtime.bigint() - t0);
    if (elapsed >= PILOT_MIN_NS) {
      perOp = elapsed / n;
      break;
    }
    n *= 2;
  }

  const iters = Math.max(1, Math.round(TARGET_SAMPLE_NS / perOp));
  const samples = [];
  for (let s = 0; s < SAMPLES; s++) {
    const t0 = process.hrtime.bigint();
    globalSink += batch(iters);
    samples.push(Number(process.hrtime.bigint() - t0) / iters);
  }
  samples.sort((a, b) => a - b);
  return samples[Math.floor(SAMPLES / 2)];
}

export function flushSink(runtime) {
  process.stderr.write(`${runtime}: sink=${globalSink}\n`);
}

export function loadWorkloads(dir) {
  return ['simple', 'eligibility', 'array100'].map((name) => ({
    name,
    rule: readFileSync(join(dir, `${name}.rule.json`), 'utf-8'),
    data: readFileSync(join(dir, `${name}.data.json`), 'utf-8'),
    expected: readFileSync(join(dir, `${name}.expected.json`), 'utf-8'),
  }));
}

export function emit(runtime, mode, workload, nsOp) {
  process.stdout.write(
    JSON.stringify({ runtime, mode, workload, ns_op: Number(nsOp.toFixed(3)) }) + '\n',
  );
}

/** Byte-compare a JSON-string result against the checked-in expectation. */
export function verifyStr(runtime, mode, workload, got, expected) {
  if (got !== expected) {
    process.stderr.write(
      `${runtime}: verification failed for mode=${mode} workload=${workload}\n` +
        `  expected: ${expected}\n  got:      ${got}\n`,
    );
    process.exit(1);
  }
}

/**
 * Deep-compare an object-shaped result. The three workload results are a
 * boolean, a string, and an array of numbers, so JSON.stringify is a
 * stable canonical form (no object-key-order concerns) and matches the
 * compact serialization in the expected files.
 */
export function verifyDeep(runtime, mode, workload, got, expected) {
  verifyStr(runtime, mode, workload, JSON.stringify(got), expected);
}

/** Common CLI: <workloads-dir> [--modes=a,b] [--workloads=x,y]. */
export function parseArgs(argv, runtime) {
  let dir = null;
  let modes = null;
  let workloads = null;
  for (const arg of argv) {
    if (arg.startsWith('--modes=')) modes = arg.slice(8).split(',');
    else if (arg.startsWith('--workloads=')) workloads = arg.slice(12).split(',');
    else dir = arg;
  }
  if (!dir) {
    process.stderr.write(`usage: ${runtime} <workloads-dir> [--modes=a,b] [--workloads=x,y]\n`);
    process.exit(1);
  }
  return { dir, modes, workloads };
}
