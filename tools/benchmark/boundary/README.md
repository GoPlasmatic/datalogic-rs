# Boundary benchmark harness

In-tree reproduction of the per-binding boundary measurements in
[`BINDINGS-OVERHEAD.md`](../BINDINGS-OVERHEAD.md) — the full cost a real
caller pays per evaluation through each language binding, as opposed to
the engine-only numbers in [`BENCHMARK.md`](../BENCHMARK.md). This is
"step 0" of that document's section 6 and the acceptance harness the
C ABI v2 arc was measured with (see that document's "Outcome" section).

## Quick start

```bash
cd tools/benchmark/boundary
./run.sh                        # the default (toolchain-light) five
./run.sh all                    # all nine runtimes
python3 render.py               # markdown tables from the newest run
```

`run.sh` builds each requested runtime's prerequisites, runs its runner
across all three workloads, and appends every runner's JSON lines to
`output/boundary-<timestamp>.jsonl`. `render.py` turns a results file
into the two tables BINDINGS-OVERHEAD.md carries (hot-path + full
appendix).

## Workloads

Defined byte-exactly in the appendix of BINDINGS-OVERHEAD.md and checked
in under [`workloads/`](./workloads) so runs are byte-stable:

| name        | rule    | data    | expected result |
|-------------|---------|---------|-----------------|
| simple      |    74 B |    68 B | `true` |
| eligibility |   458 B |   955 B | `"approved:APP-2481"` |
| array100    |    89 B | 8,279 B | the 49-element qty array |

`workloads/generate.py` regenerates them (and `--check` verifies the
checked-in bytes, which `run.sh` does before every run). Don't edit the
JSON files by hand — change the generator, rerun it, and expect the
documented sizes to still hold (the generator refuses to emit drift).

## Methodology

Identical across every runner (from BINDINGS-OVERHEAD.md's appendix):

- **Warmup**: 2,000 iterations; 5,000 on JIT runtimes (Node, JVM, PHP
  with JIT, WASM-on-Node).
- **Pilot**: batch size doubles until one batch takes >= 10 ms, then N is
  sized so one timed sample lands near **~250 ms**.
- **Median of 5 samples**, ns/op.
- **Results consumed** (accumulated lengths / `black_box`) so work can't
  be elided; every runner prints its sink to stderr.
- **Correctness first**: every runner evaluates each workload once and
  byte-compares (string paths) or deep-compares (object paths) against
  `workloads/<name>.expected.json`, aborting on mismatch. Nothing gets
  timed unless it produced the right answer.
- `session-evaluate-many-100` modes time one `evaluate_many` call over
  100 separately-compiled copies of the workload rule against one data
  handle and report **ns per evaluation** (call time / 100).

Output line schema, one per (runtime, mode, workload):

```json
{"runtime": "c-abi", "mode": "session-evaluate", "workload": "simple", "ns_op": 123.0}
```

## Runners

| runtime   | runner | status | invocation (after prerequisites) |
|-----------|--------|--------|----------------------------------|
| rust-core | `../src/bin/boundary_core.rs` | verified | `cargo run --release -p datalogic-bench --bin boundary_core -- workloads` |
| c-abi     | `runner-c.c` | verified | `cc -O2 runner-c.c -I ../../../bindings/c/include -L ../../../bindings/c/target/release -ldatalogic_c -o output/build/runner-c && output/build/runner-c workloads` |
| node      | `runner-node.mjs` | verified | `node runner-node.mjs workloads` (after `npx napi build --platform --release` in bindings/node) |
| python    | `runner-python.py` | verified | `.venv/bin/python runner-python.py workloads` (after `maturin build --release` + wheel install into `.venv`) |
| wasm      | `runner-wasm.mjs` | verified | `node runner-wasm.mjs workloads` (after `./build.sh` in bindings/wasm; loads `pkg/nodejs/`, the same artifact the compare harness uses) |
| go        | `runner-go/` | verified | `cd runner-go && go run . ../workloads` |
| dotnet    | `runner-dotnet/` | verified | `dotnet run -c Release --project runner-dotnet -- workloads` |
| jvm       | `runner-jvm/Boundary.java` | verified | via `./run.sh jvm` (mvn package + dependency classpath + javac + java) |
| php       | `runner-php.php` | verified | `php -d opcache.enable_cli=1 -d opcache.jit=tracing -d opcache.jit_buffer_size=64M runner-php.php workloads` |

Every runner accepts `<workloads-dir> [--modes=a,b] [--workloads=x,y]`.
All nine produced the 2026-07-03 v2 capture in BINDINGS-OVERHEAD.md.
The go/dotnet/jvm/php four are in the "extended" set only because they
need their language toolchains installed; the jvm runner additionally
needs a real JDK on PATH/JAVA_HOME (the macOS system `java` stub has no
runtime) and gets the binding's Jackson dependency on the classpath
from `run.sh`.

## Modes

Per-runtime mode lists mirror the tier tables in BINDINGS-OVERHEAD.md;
new v2 tiers are additive:

- **rust-core**: `eval-preparsed`, `parseddata-eval` (new: core
  `ParsedData` handle, eval only), `parse-eval`, `parse-eval-serialize`
  (the string-contract floor), `parse-eval-serialize-fresharena`,
  `serde-value-in-out`.
- **c-abi**: `session-evaluate`, `session-evaluate-data` (new: parsed
  data handle), `session-evaluate-many-100` (new: batch), `rule-evaluate`,
  `engine-apply-oneshot`.
- **node**: `session-evaluateStr-str`, `session-evaluate-data`,
  `session-evaluate-many-100`, `rule-evaluateStr-str`,
  `rule-evaluate-obj`, `stringify-str-parse-roundtrip`,
  `engine-eval-oneshot` (object rule + object data, matching the
  documented capture).
- **python**: `session-evaluate-str`, `session-evaluate-data`,
  `session-evaluate-many-100`, `rule-evaluate-str`,
  `rule-evaluate-dict`, `dumps-str-loads-roundtrip`,
  `engine-eval-oneshot` (dict rule + dict data).
- **wasm**: `session-evaluate-str`, `session-evaluate-data`,
  `session-evaluate-many-100`, `compiledrule-evaluate-str`,
  `oneshot-evaluate`.
- **go / dotnet / jvm / php**: `session-evaluate`,
  `session-evaluate-data`, `session-evaluate-many-100`,
  `rule-evaluate`, `engine-apply-oneshot` (+ php
  `encode-eval-decode-roundtrip`).

## Reading the numbers

- Judge every binding by its distance from the **string-contract floor**
  (rust-core `parse-eval-serialize`); `render.py`'s hot-path table
  computes that column.
- Cross-process run-to-run variance is roughly ±5%; single-digit-percent
  differences between adjacent rows are noise.
- Builds are portable (run.sh invokes cargo from the repo root, so the
  benchmark crate's cwd-scoped `-C target-cpu=native` config does not
  apply). Numbers are still machine-specific — compare runs from the
  same machine.
