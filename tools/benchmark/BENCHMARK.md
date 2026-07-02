# Benchmark reference

Cross-library JSONLogic matrix produced by
`tools/benchmark/src/bin/compare.rs`. This file is the canonical
performance reference — link to it from other docs (README, blog posts,
changelog) rather than re-quoting numbers inline, so updates only need
one place.

> **Captured:** 2026-05-10  •  **Apple M2 Pro (arm64)** macOS 26.3 (Tahoe)
> •  Rust 1.93.0  •  Node v24.10.0  •  release build, no `target-cpu=native`
>
> Each cell is the **median of 3** timed samples, each iteration count
> sized to hit a **~200 ms wall budget**. The unit throughout is
> **nanoseconds per evaluation** (lower is better).

The matrix shows one column per **library / API tier that takes a
precompile-once approach** — apples-to-apples cells. Convenience-API
tiers (parsing on every call, per-call session reset, raw one-shot WASM
string-string) are intentionally not in this matrix because their
numbers measure API-shape costs, not engine cost. For datalogic-rs's
own tier-by-tier numbers, see `bin/self.rs`.

## Subject reference

| Column                       | API                                                                                                                                                                          |
|------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `dlrs:engine`                | datalogic-rs native: pre-compiled `Logic` + caller-owned `Bump`, `Engine::evaluate(...)` per call                                                                            |
| `dlrs:wasm:compiled`         | `@goplasmatic/datalogic-wasm` (this repo, via Node): `new CompiledRule(ruleStr, false)` once + `.evaluate(dataStr)` per call                                                 |
| `jsonlogic-rs`               | [bestowinc/json-logic-rs] 0.5 (Rust): `apply(&Value, &Value)` — no compile API, pre-parsed in setup                                                                          |
| `json-logic-js`              | [json-logic-js] (jwadhams, JS via Node): `apply(rule, data)` — interpreted, no compile API                                                                                   |
| `json-logic-engine`          | [json-logic-engine] (TotalTechGeek, JS via Node): `engine.run(rule, data)` — interpreted                                                                                     |
| `json-logic-engine:compiled` | [json-logic-engine] (JS via Node): `engine.build(rule)` once + `fn(data)` per call (the "12.5–20× hot path" mode advertised by the library's README)                         |

[bestowinc/json-logic-rs]: https://crates.io/crates/jsonlogic-rs
[json-logic-js]: https://www.npmjs.com/package/json-logic-js
[json-logic-engine]: https://www.npmjs.com/package/json-logic-engine

## Methodology

- **Cell value**: median of 3 timed samples. The samples themselves run
  N iterations chosen by a per-subject pilot pass to land near the
  ~200 ms target — large N for the fast subjects, small N for the slow
  ones. ns/op normalises across iteration counts so cells stay
  comparable.
- **Self benchmark (`bin/self.rs`)**: same median-of-3 discipline, with
  the min/max spread printed per suite (`±n%`), and the evaluation
  result passed through `std::hint::black_box` inside the timed loop so
  the optimizer cannot elide unused work. Numbers from reports generated
  before this change are slightly flattered; regenerate before
  comparing.
- **Folded vs non-folded split (`bin/self.rs`)**: many suite rules have
  no data dependency, so the compiler constant-folds them to a literal
  (`Logic::is_constant`) and their timed cost is literal-return
  overhead, not engine work. The whole-suite number stays the headline
  (comparable with older reports), and two additional passes with the
  same discipline time the folded rules and the rest separately. The
  summary reports three geomeans of per-suite averages: overall,
  folded-only, non-folded-only. Quote the non-folded geomean when the
  claim is about evaluating data-dependent rules.
- **Pairwise shared-suite ratios (`bin/compare.rs`)**: the per-column
  mean rows aggregate whatever suites each column completed, so when
  subjects `ERR` on different suites, dividing two column geomeans
  compares incomparable suite sets. After the matrix, the runner prints
  a ratio table where each pair's number is the geomean of per-suite
  ratios computed only over suites where **both** subjects have finite
  cells, along with the shared-suite count. Quote these ratios, not
  column-geomean quotients.
- **Pre-parse where possible**: subjects with a "compile" or "parse"
  step do it in setup (outside the timed loop). The cells measure
  per-call evaluation work, not per-call API shape.
- **Aggregation rows**: arithmetic mean and **geometric mean** over the
  finite cells in each column. Geomean is the right average for
  cross-library comparison — one slow suite doesn't dominate the way
  it does with arithmetic mean.
- **Negative-test cases dropped**: suites include cases like
  `{ "rule": ..., "error": { "type": "NaN" } }`. These are filtered out
  for the cross-library run (`load_suite_for_compare`) because libraries
  disagree on what "errors" and on how expensive their error path is —
  including them would penalise verbose-error subjects.
- **Cell markers**:
  - **`<n>`** — median ns/op
  - **`<n>*`** — partial coverage; subject errored on some cases in this
    suite (ns/op averages over total evals, including errored ones)
  - **`ERR`** — subject errored on >50% of cases in the suite
  - **`—`** — subject couldn't run the suite at all (precompile failed,
    runtime missing, or operator unsupported)

## Matrix

```
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 44 suites) ===

| Suite                             | dlrs:engine | jsonlogic-rs | dlrs:wasm:compiled | json-logic-js | json-logic-engine | json-logic-engine:compiled |
|-----------------------------------|------------:|-------------:|-------------------:|--------------:|------------------:|---------------------------:|
| compatible.json                   |        12.5 |        448.1 |              675.8 |         264.6 |             112.8 |                       75.0 |
| arithmetic/plus.json              |         2.8 |       224.4* |              518.6 |        393.4* |              73.0 |                       22.6 |
| arithmetic/multiply.json          |         2.8 |       212.4* |              466.0 |        538.4* |              77.1 |                       22.8 |
| arithmetic/minus.json             |         3.2 |       141.6* |              751.9 |        366.6* |              74.1 |                       23.9 |
| arithmetic/divide.json            |         3.2 |          ERR |              624.6 |        381.5* |              76.6 |                       22.8 |
| arithmetic/modulo.json            |         3.1 |       185.4* |              746.0 |        468.0* |             116.9 |                       24.0 |
| arithmetic/min.json               |        15.3 |       351.6* |             1261.6 |       1677.5* |             152.0 |                       29.9 |
| arithmetic/max.json               |        15.2 |       357.7* |             1077.6 |       1697.0* |             130.5 |                       31.1 |
| arithmetic/chain.json             |        30.4 |          ERR |             2245.2 |           ERR |             222.0 |                       94.8 |
| arithmetic/abs.json               |         6.4 |       151.6* |                ERR |           ERR |               ERR |                          — |
| arithmetic/ceil.json              |         5.1 |       134.8* |                ERR |           ERR |               ERR |                          — |
| arithmetic/floor.json             |         5.1 |       140.0* |                ERR |           ERR |               ERR |                          — |
| comparison/softEquals.json        |         2.6 |       141.1* |              458.3 |        287.1* |              71.2 |                       24.3 |
| comparison/strictEquals.json      |         2.5 |       142.1* |              450.1 |        269.9* |              66.4 |                       19.6 |
| comparison/softNotEquals.json     |         2.5 |       144.7* |              458.1 |        314.4* |              72.1 |                       22.9 |
| comparison/strictNotEquals.json   |         2.4 |       145.1* |              456.3 |        280.5* |              71.6 |                       19.2 |
| comparison/greaterThan.json       |         2.5 |        197.0 |              457.0 |        292.7* |              73.8 |                       23.5 |
| comparison/greaterThanEquals.json |         2.5 |        202.7 |              566.8 |        373.8* |              69.6 |                       20.4 |
| comparison/lessThan.json          |         2.4 |        193.4 |              439.9 |        237.9* |              70.3 |                       19.5 |
| comparison/lessThanEquals.json    |         2.6 |        192.7 |              601.9 |        518.0* |             109.5 |                       23.2 |
| control/if.json                   |         3.1 |        244.3 |              504.6 |        286.9* |              89.1 |                       26.5 |
| control/and.json                  |         2.3 |       148.7* |              415.3 |         104.1 |              73.7 |                       22.9 |
| control/or.json                   |         2.5 |       148.5* |              426.9 |        280.6* |              71.3 |                       24.8 |
| truthiness.json                   |         4.1 |        159.5 |              637.7 |       1581.2* |             114.3 |                       34.3 |
| additional.json                   |        13.7 |       374.7* |             2195.4 |           ERR |             520.0 |                      102.1 |
| coalesce.json                     |         5.9 |        167.6 |                ERR |           ERR |             118.2 |                       26.3 |
| chained.json                      |        42.1 |       527.4* |             2952.7 |           ERR |             303.9 |                      111.8 |
| exists.json                       |         6.8 |        121.3 |                ERR |           ERR |             147.2 |                       70.6 |
| val.json                          |         9.1 |       170.7* |              970.7 |           ERR |             164.9 |                       27.5 |
| val-compat.json                   |        16.7 |          ERR |             1325.0 |           ERR |             239.5 |                       90.0 |
| val.extra.json                    |        63.7 |      1025.9* |             2703.7 |           ERR |             242.1 |                      127.4 |
| scopes.json                       |        93.5 |          ERR |             4698.7 |           ERR |            1238.0 |                      383.2 |
| empty-objects.json                |         7.4 |         22.7 |             1482.3 |         328.7 |             221.8 |                       39.4 |
| structured-objects.json           |           — |        365.5 |                  — |       1477.8* |               ERR |                          — |
| try.json                          |       134.6 |        351.4 |                ERR |           ERR |             275.7 |                       74.7 |
| try.extra.json                    |       135.0 |        473.1 |                ERR |           ERR |            4260.4 |                      159.0 |
| datetime/datetime.json            |        10.3 |       380.5* |              769.2 |           ERR |               ERR |                          — |
| datetime/duration.json            |        12.8 |       336.9* |              787.9 |           ERR |               ERR |                          — |
| datetime/now.json                 |       118.6 |        362.1 |                ERR |           ERR |               ERR |                          — |
| length.json                       |        11.1 |        285.8 |                ERR |           ERR |             184.0 |                      201.1 |
| sort.json                         |        59.7 |       247.1* |                ERR |           ERR |               ERR |                          — |
| slice.json                        |        79.5 |       220.9* |                ERR |           ERR |               ERR |                          — |
| array/map.json                    |       116.0 |          ERR |             2831.6 |           ERR |           6167.6* |                    3257.7* |
| string/string.json                |        24.7 |        208.5 |                ERR |           ERR |               ERR |                          — |
| arithmetic mean                   |        25.5 |        257.7 |             1127.7 |         564.6 |             472.7 |                      155.8 |
| geometric mean                    |         9.7 |        218.0 |              855.6 |         423.5 |             160.3 |                       47.2 |
```

`*` partial coverage — subject errored on some cases in this suite.

## Quick reading

Geomeans across all 44 suites (lower is better):

| Subject                        | Geomean ns/op | Relative to `dlrs:engine` |
|--------------------------------|--------------:|--------------------------:|
| `dlrs:engine`                  |           9.7 | 1.0×                      |
| `json-logic-engine:compiled`   |          47.2 | 4.9×                      |
| `json-logic-engine` (interp.)  |         160.3 | 16.5×                     |
| `jsonlogic-rs`                 |         218.0 | 22.5×                     |
| `json-logic-js`                |         423.5 | 43.7×                     |
| `dlrs:wasm:compiled`           |         855.6 | 88.2×                     |

The "Relative to" column above divides column geomeans that cover
different suite subsets (each column skips the suites it `ERR`ed on), so
treat it as a rough read. The compare binary now prints pairwise
shared-suite ratios (geomean of per-suite ratios over suites both
subjects completed, with the shared-suite count); quote those when the
ratio is the claim. Re-capture this section to include them.

Headline takeaways:

- **`dlrs:engine` is the fastest cell on nearly every suite** — single-digit
  ns/op on basic arithmetic, comparison, and control-flow; double-digit on
  heavier `try` / `chained` / `scopes` patterns.
- **`json-logic-engine:compiled` (~47 ns) is the strongest non-dlrs
  contender** — a real, modern competitor and the only JS library in the
  same order of magnitude. Still ~5× behind `dlrs:engine` but well ahead
  of the reference `json-logic-js` (~423 ns).
- **`dlrs:wasm:compiled` (~856 ns) ≈ ~88× `dlrs:engine`** — the cost is
  the V8↔WASM boundary on every call (data marshall + JSON parse + eval +
  result stringify + result marshall). Eval itself is fast; the ABI is
  what hurts. To go below this number you'd have to bypass V8 entirely
  (e.g. host the WASM in `wasmtime` from native Rust) — out of scope.
- **`ERR` cells reflect operator-set differences**, not raw failure:
  - Note: the `dlrs:wasm:compiled` ERR cells are stale. This matrix was
    captured when `@goplasmatic/datalogic-wasm` shipped a curated feature
    set; the build now enables every operator family plus `flagd`, `trace`,
    and `templating`, so these cells should be re-captured. The suites
    affected: `try`, `length`, `sort`, `slice`, `coalesce`, `exists`,
    `arithmetic/{abs,ceil,floor}`, `string/string.json`, `datetime/now.json`.
  - `json-logic-js` ERRs on extension suites (it ships only the spec).
  - `json-logic-engine`'s compiled mode (`engine.build`) eagerly
    validates operator names and ERRs on unknown ones; the interpreted
    mode is more lenient and runs more suites.
  - `jsonlogic-rs` ERRs on a handful of arithmetic and val-compat
    cases — small subset of the spec it doesn't model the same way.
- **`structured-objects.json` shows `—`** for the precompile tiers because
  their `Engine::new()` doesn't enable templating mode — building with
  `Engine::builder().with_templating(true).build()` would unblock those
  columns at the cost of slightly slower non-template paths. Out of
  scope for this matrix.

## Reproduce

One-time setup for the Node subjects:

```bash
cd bindings/wasm && ./build.sh
cd tools/benchmark/runners && npm install
```

Run the matrix:

```bash
cargo run --release -p datalogic-bench --bin compare \
  --features subject-jsonlogic-rs -- --all
```

Drop `--features subject-jsonlogic-rs` to skip the gated Rust competitor.
Pass a single suite name (e.g. `arithmetic/plus.json`) instead of `--all`
for fast iteration. See `tools/benchmark/README.md` for the full flag
reference and the recipe to add more subjects.

## Macro tier (self benchmark)

The operator suites use payloads of at most a few hundred bytes, so the
matrix above measures operator dispatch, not data-volume behaviour. The
self benchmark's macro tier fills that gap with suites **synthesized in
code** (`tools/benchmark/src/macro_suites.rs`, nothing large checked in):

| Suite                 | Payload and rules                                                                          |
|-----------------------|--------------------------------------------------------------------------------------------|
| `macro/array-1k`      | 1,000-element numeric permutation + object rows; filter / map / reduce / sort / `in` scans |
| `macro/array-10k`     | Same rules over 10,000 elements                                                            |
| `macro/object-128key` | 128-key object; shallow + dotted-deep `var` lookups, `merge` of two 64-element arrays      |
| `macro/deep-48`       | 48 levels of nesting; one 49-segment dotted `var` path                                     |
| `macro/string-10kb`   | Two ~10 KB strings; `cat`, `substr` (middle and negative-start), substring `in`            |
| `macro/eligibility`   | Realistic eligibility rule: and/or/comparisons/`missing`/`reduce` over a medium object     |

Run it with:

```bash
cargo run --release -p datalogic-bench --bin self -- --macro
```

Timing discipline matches the micro suites (median of 3 reps, `black_box`
around every evaluation, session reset per iteration, arena pre-sized
from warm-up), except the per-suite iteration count is scaled from a
pilot pass so one timed rep lands near ~250 ms; a fixed 100k iterations
on a 10k-element array would run for minutes per suite. Every macro case
is sanity-evaluated before timing; a rule that errors aborts the run
instead of silently timing the error path. ns/op is per whole-rule
evaluation: `macro/array-10k` at ~120 µs/op means one filter/map/sort
pass over 10k elements costs ~120 µs, i.e. ~12 ns per element touched.

The macro tier is currently self-benchmark only; a cross-engine macro
comparison remains future work.

## Caveats

- Numbers are macOS / Apple Silicon. Linux x86_64 will produce a
  different distribution — wasm-bindgen, V8, and chrono all behave
  somewhat differently across hosts. **Don't quote absolute numbers
  across machines; quote ratios.**
- Timing is wall-clock. There is no GC pause / thermal throttle
  detection. The 3-sample median rejects single-event outliers but
  doesn't bound systematic drift; rerun a few times before drawing
  fine-grained conclusions.
- **The benchmark build is not the published-artifact build.** The Rust
  rows compile with the root workspace's `lto = "fat"`,
  `codegen-units = 1` release profile on the host CPU; published wheels,
  prebuilds, and the WASM package are built by the release matrix with
  their own profiles (the WASM workspace optimises for size). Treat the
  matrix as engine-vs-engine on equal footing, not as a promise for a
  specific packaged binary.
- **Matrix suite payloads are small** (hundreds of bytes). The matrix
  says nothing about 1k+-element arrays, 100+-key objects, or deep
  nesting; for datalogic-rs itself those are covered by the
  [macro tier](#macro-tier-self-benchmark) of the self benchmark.
  Cross-engine macro comparison remains future work. The honest micro
  headline: single-digit nanoseconds for folded/scalar rules, 10-120 ns
  for context-dependent rules.
- Local-only by design — never run in CI.

## Optimization backlog (candidates, not yet implemented)

Remaining engine-level candidates from the 2026-07 performance pass
(implemented that pass: zero-copy `substr`/string-`slice`, arena-backed
sort scratch, `itoa`/`ryu` number rendering, split context stack — corpus
geomean -2.3%, targeted suites -12% to -45%). Guardrails for anyone
picking these up: the conformance suite, the optimized-vs-traced
differential property test, `tests/layout_test.rs`, and the folded /
non-folded split above (quote the non-folded geomean).

| # | Candidate | Where | Expected gain | Effort |
|---|-----------|-------|---------------|--------|
| 1 | Pre-convert composite literals at compile time (give `Logic` a private `Bump`, or a borrowing `to_arena` that reuses `&str`) | `engine/mod.rs`, `node/populate.rs` | Large on membership lists / switch tables / templated objects; measurable via `--macro` | M |
| 3 | Stop heap round-trips in throw/try and NaN errors (`Arc` the payload, intern NaN, borrowed thrown-value channel) | `dispatch.rs`, `error_handling.rs`, `error/mod.rs` | 30-50% on try/throw-heavy rules (`try.json` ~121 ns is the slowest tier) | M |
| 7 | Fast path for ISO-date string comparison (datetime feature) | `operators/comparison/mod.rs`, `datetime/mod.rs` | Large on date-field filters/sorts | M |
| 8 | Shrink `Error` so `Result` returns are thin (currently 80 bytes, guarded by `tests/layout_test.rs`) | `error/mod.rs`, `error/kind.rs` | 1-5% geomean | M |
| 9 | Indexed lookup / O(n log n) equality for wide objects | `arena/value/lookup.rs`, datavalue crate | Large on 100+ key payloads; measurable via `--macro` object-128key | L |

Also open: running the macro tier cross-engine (it currently exists for
the self benchmark only).
