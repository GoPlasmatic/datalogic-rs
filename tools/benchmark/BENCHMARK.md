# Benchmark reference

Cross-library JSONLogic matrix produced by
`tools/benchmark/src/bin/compare.rs`. This file is the canonical
performance reference — link to it from other docs (README, blog posts,
changelog) rather than re-quoting numbers inline, so updates only need
one place.

This matrix measures **engine cost** (pre-parsed inputs, compile-once
subjects). For the per-call **boundary cost of each language binding**
(Node, Python, WASM, C, Go, JVM, .NET, PHP) and the catalog of options to
reduce it, see [BINDINGS-OVERHEAD.md](./BINDINGS-OVERHEAD.md).

> **Captured:** 2026-07-03  •  **Apple M2 Pro (arm64)** macOS 26.5 (Tahoe)
> •  Rust 1.96.0  •  Node v22.22.2  •  release build, no `target-cpu=native`
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
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 50 suites) ===

| Suite                             | dlrs:engine | jsonlogic-rs | dlrs:wasm:compiled | json-logic-js | json-logic-engine | json-logic-engine:compiled |
|-----------------------------------|------------:|-------------:|-------------------:|--------------:|------------------:|---------------------------:|
| compatible.json                   |         8.5 |        453.5 |              593.8 |         241.0 |             117.6 |                       62.0 |
| arithmetic/plus.json              |         2.9 |       227.4* |              492.0 |        384.7* |             100.6 |                       30.1 |
| arithmetic/multiply.json          |         2.9 |       215.1* |              452.5 |        520.4* |              99.7 |                       31.0 |
| arithmetic/minus.json             |         3.2 |       143.0* |              695.1 |        375.7* |             163.3 |                       31.4 |
| arithmetic/divide.json            |         3.4 |          ERR |              585.6 |        344.5* |             146.7 |                       33.0 |
| arithmetic/modulo.json            |         3.4 |       184.1* |              635.3 |        441.2* |             149.5 |                       30.9 |
| arithmetic/min.json               |        14.0 |       363.7* |              881.6 |       1489.1* |             211.9 |                       40.1 |
| arithmetic/max.json               |        13.9 |       360.8* |              997.3 |       1511.3* |             266.9 |                       38.9 |
| arithmetic/chain.json             |        26.0 |          ERR |             2037.9 |           ERR |             437.6 |                      111.5 |
| comparison/softEquals.json        |         2.6 |       143.2* |              424.0 |        293.1* |              80.1 |                       31.6 |
| comparison/strictEquals.json      |         2.5 |       143.3* |              419.0 |        282.3* |              64.6 |                       24.8 |
| comparison/softNotEquals.json     |         2.6 |       146.3* |              418.9 |        299.7* |              91.1 |                       31.3 |
| comparison/strictNotEquals.json   |         2.5 |       146.6* |              419.4 |        283.8* |              77.7 |                       26.6 |
| comparison/greaterThan.json       |         2.5 |        201.3 |              430.5 |        298.2* |              77.9 |                       29.7 |
| comparison/greaterThanEquals.json |         2.7 |        206.1 |              480.8 |        360.0* |              98.0 |                       29.2 |
| comparison/lessThan.json          |         2.4 |        196.7 |              409.8 |        238.6* |              57.7 |                       24.4 |
| comparison/lessThanEquals.json    |         2.6 |        198.9 |              630.0 |        456.8* |             153.6 |                       32.1 |
| control/if.json                   |         2.3 |        249.8 |              476.7 |        294.7* |              88.2 |                       31.6 |
| control/and.json                  |         2.3 |       152.1* |              410.2 |         180.5 |             116.9 |                       31.8 |
| control/or.json                   |         2.5 |       150.1* |              408.2 |        375.5* |             128.4 |                       32.2 |
| control/switch.json               |        25.5 |       431.0* |              774.5 |           ERR |               ERR |                          — |
| truthiness.json                   |         4.0 |        162.0 |              565.9 |       1464.9* |             202.2 |                       35.4 |
| additional.json                   |        13.7 |       387.4* |             2035.1 |           ERR |             686.3 |                       80.8 |
| coalesce.json                     |         5.4 |        169.7 |              740.5 |           ERR |             196.7 |                       29.5 |
| chained.json                      |        37.4 |       525.7* |             2068.2 |           ERR |             645.9 |                      119.8 |
| exists.json                       |         7.2 |        122.9 |              860.3 |           ERR |             333.0 |                       72.2 |
| val.json                          |         7.8 |       172.2* |              962.8 |           ERR |             208.2 |                       35.5 |
| val-compat.json                   |        14.7 |          ERR |             1240.2 |           ERR |             332.7 |                      135.5 |
| val.extra.json                    |        63.5 |      1040.0* |             2095.2 |           ERR |             559.0 |                      207.2 |
| scopes.json                       |       101.1 |          ERR |             3550.1 |           ERR |            2061.5 |                      489.1 |
| empty-objects.json                |         3.0 |         23.0 |             1119.8 |         125.8 |             196.8 |                       51.6 |
| structured-objects.json           |           — |        370.8 |                ERR |       1397.5* |               ERR |                        ERR |
| try.json                          |        52.4 |        359.1 |             1842.4 |           ERR |             471.3 |                      125.7 |
| try.extra.json                    |        60.5 |        477.3 |             1886.0 |           ERR |            7715.2 |                      285.3 |
| datetime/datetime.json            |         9.6 |       383.3* |              761.6 |           ERR |               ERR |                          — |
| datetime/duration.json            |        12.5 |       344.7* |              772.0 |           ERR |               ERR |                        ERR |
| datetime/now.json                 |       121.4 |        366.1 |             2108.6 |           ERR |               ERR |                          — |
| length.json                       |        11.1 |        291.0 |             1426.6 |           ERR |             393.3 |                      249.9 |
| sort.json                         |        45.0 |       252.9* |             1935.5 |           ERR |               ERR |                          — |
| slice.json                        |        33.4 |       225.9* |             1196.2 |           ERR |               ERR |                          — |
| array/map.json                    |        77.9 |          ERR |             2598.6 |           ERR |           5340.2* |                    2992.5* |
| array/merge.json                  |        13.2 |        224.3 |              713.0 |         393.1 |             163.4 |                       19.8 |
| array/reduce.json                 |        42.2 |      3868.5* |             2139.3 |       1874.5* |             670.1 |                     292.2* |
| string/string.json                |        22.8 |        211.5 |             1077.3 |           ERR |               ERR |                          — |
| arithmetic/abs.json               |         3.2 |       154.2* |              808.7 |           ERR |               ERR |                          — |
| arithmetic/ceil.json              |         3.0 |       137.6* |              772.6 |           ERR |               ERR |                          — |
| arithmetic/floor.json             |         3.0 |       142.0* |              783.0 |           ERR |               ERR |                          — |
| flagd/fractional.json             |        37.5 |        502.2 |             1482.3 |           ERR |               ERR |                          — |
| flagd/sem_ver.json                |         6.4 |        286.8 |              589.8 |           ERR |               ERR |                          — |
| type.json                         |         6.8 |        164.8 |              856.7 |           ERR |               ERR |                          — |
| arithmetic mean                   |        19.4 |        348.4 |             1062.5 |         580.3 |             636.2 |                      165.5 |
| geometric mean                    |         9.0 |        243.7 |              881.9 |         433.5 |             236.0 |                       60.4 |
```

`*` partial coverage — subject errored on some cases in this suite.

### Pairwise shared-suite ratios

Quote these instead of dividing the per-column geomeans; each pair is
computed only over the suites both subjects completed.

```
  jsonlogic-rs                   30.3x slower than dlrs:engine                over 44 shared suites
  dlrs:wasm:compiled             98.4x slower than dlrs:engine                over 49 shared suites
  json-logic-js                 102.8x slower than dlrs:engine                over 23 shared suites
  json-logic-engine              30.7x slower than dlrs:engine                over 36 shared suites
  json-logic-engine:compiled      7.9x slower than dlrs:engine                over 36 shared suites
  dlrs:wasm:compiled              3.4x slower than jsonlogic-rs               over 44 shared suites
  json-logic-js                   2.1x slower than jsonlogic-rs               over 23 shared suites
  jsonlogic-rs                    1.2x slower than json-logic-engine          over 31 shared suites
  jsonlogic-rs                    4.9x slower than json-logic-engine:compiled over 31 shared suites
  dlrs:wasm:compiled              1.4x slower than json-logic-js              over 23 shared suites
  dlrs:wasm:compiled              3.6x slower than json-logic-engine          over 36 shared suites
  dlrs:wasm:compiled             14.0x slower than json-logic-engine:compiled over 36 shared suites
  json-logic-js                   3.2x slower than json-logic-engine          over 23 shared suites
  json-logic-js                  11.6x slower than json-logic-engine:compiled over 23 shared suites
  json-logic-engine               3.9x slower than json-logic-engine:compiled over 36 shared suites
```

## Quick reading

Geomeans across the 50 timed suites (53 discovered; the 3 negative-only
suites skip). Lower is better:

| Subject                        | Geomean ns/op (own suite set) | Pairwise vs `dlrs:engine` (shared suites) |
|--------------------------------|------------------------------:|------------------------------------------:|
| `dlrs:engine`                  |                           9.0 | 1.0×                                       |
| `json-logic-engine:compiled`   |                          60.4 | 7.9× (36)                                  |
| `json-logic-engine` (interp.)  |                         236.0 | 30.7× (36)                                 |
| `jsonlogic-rs`                 |                         243.7 | 30.3× (44)                                 |
| `json-logic-js`                |                         433.5 | 102.8× (23)                                |
| `dlrs:wasm:compiled`           |                         881.9 | 98.4× (49)                                 |

The geomean column aggregates whatever suites each subject completed, so
those numbers cover different suite subsets. The pairwise column (from
the ratio table under the matrix) compares only suites both subjects
ran; quote it when the ratio is the claim. `json-logic-js` shares just
the 23 spec-only suites, which is why its pairwise ratio lands above its
geomean quotient.

Headline takeaways:

- **`dlrs:engine` is the fastest cell on every suite it runs** — single-digit
  ns/op on basic arithmetic, comparison, and control-flow; double-digit on
  heavier `try` / `chained` / `scopes` patterns.
- **`json-logic-engine:compiled` (~60 ns) is the strongest non-dlrs
  contender** — a real, modern competitor and the only JS library in the
  same order of magnitude. Still ~8× behind `dlrs:engine` but far ahead
  of the reference `json-logic-js`.
- **`dlrs:wasm:compiled` (98.4× pairwise)** — the cost is
  the V8↔WASM boundary on every call (data marshall + JSON parse + eval +
  result stringify + result marshall). Eval itself is fast; the ABI is
  what hurts. Per-binding boundary decomposition and the catalog of
  reduction options: [BINDINGS-OVERHEAD.md](./BINDINGS-OVERHEAD.md).
- **`ERR` cells reflect operator-set differences**, not raw failure:
  - `json-logic-js` ERRs on extension suites (it ships only the spec).
  - `json-logic-engine`'s compiled mode (`engine.build`) eagerly
    validates operator names and ERRs on unknown ones; the interpreted
    mode is more lenient and runs more suites.
  - `jsonlogic-rs` ERRs on a handful of arithmetic and val-compat
    cases — small subset of the spec it doesn't model the same way.
  - History: the 2026-05-10 capture showed 12 `dlrs:wasm:compiled` ERR
    cells. Eleven were a stale artifact of the curated feature set the
    WASM package shipped back then (every operator family is compiled in
    since 2026-05-11), and `datetime/now.json` was a real bug: `now`
    trapped on wasm32 without a JS clock, fixed 2026-07-03 by the core's
    opt-in `wasm-clock` feature. An interim re-run also showed two
    `dlrs:engine` flagd ERRs caused by `datalogic-bench` itself missing
    the `flagd` feature; when adding an operator-family feature to the
    core, add it to the bench crate's dependency list too.
- **`structured-objects.json`** needs templating mode, which the
  precompile subjects don't enable (`Engine::new()`); the Rust tiers skip
  the suite (`—`) while the WASM runner counts per-case compile failures
  (`ERR`). Building with `Engine::builder().with_templating(true).build()`
  would unblock those columns at the cost of slightly slower non-template
  paths. Out of scope for this matrix.

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

## Macro tier

The operator suites use payloads of at most a few hundred bytes, so the
matrix above measures operator dispatch, not data-volume behaviour. The
macro tier fills that gap with suites **synthesized in code**
(`tools/benchmark/src/macro_suites.rs`, nothing large checked in):

| Suite                 | Payload and rules                                                                          |
|-----------------------|--------------------------------------------------------------------------------------------|
| `macro/array-1k`      | 1,000-element numeric permutation + object rows; filter / map / reduce / sort / `in` scans |
| `macro/array-10k`     | Same rules over 10,000 elements                                                            |
| `macro/object-128key` | 128-key object; shallow + dotted-deep `var` lookups, `merge` of two 64-element arrays      |
| `macro/deep-48`       | 48 levels of nesting; one 49-segment dotted `var` path                                     |
| `macro/string-10kb`   | Two ~10 KB strings; `cat`, `substr` (middle and negative-start), substring `in`            |
| `macro/eligibility`   | Realistic eligibility rule: and/or/comparisons/`missing`/`reduce` over a medium object     |

Run it against datalogic-rs alone with:

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

### Cross-engine macro matrix

The same six suites also run across every subject of the matrix above:

```bash
cargo run --release -p datalogic-bench --bin compare \
  --features subject-jsonlogic-rs -- --macro
```

The synthesized cases reach subjects through the same in-memory protocol
as the file suites (in-process `SuiteCase` slices for Rust subjects,
stdin JSON for the Node runners), so nothing is written to disk. The
matrix cells, per-column means, and pairwise ratios land in
`output/report-compare-macro-<timestamp>.json`. A full run finishes in
well under a minute (about 7 s on the capture host), so no reduced
per-cell budget or suite subset is needed.

Captured 2026-07-03, Apple M2 Pro (arm64), Rust 1.96.0, Node v22.22.2,
release build from the repo root (no `target-cpu=native`):

```
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 6 suites) ===

| Suite               | dlrs:engine | jsonlogic-rs | dlrs:wasm:compiled | json-logic-js | json-logic-engine | json-logic-engine:compiled |
|---------------------|------------:|-------------:|-------------------:|--------------:|------------------:|---------------------------:|
| macro/array-1k      |     10135.2 |     373449.1 |           231045.3 |     156192.3* |          31076.1* |                    7165.4* |
| macro/array-10k     |    136237.7 |    4325078.4 |          2334026.8 |    1494373.5* |         281915.2* |                   65485.5* |
| macro/object-128key |        65.6 |       5141.5 |             9485.5 |         300.8 |             287.0 |                      113.2 |
| macro/deep-48       |       121.7 |      61007.9 |             4192.6 |        1319.0 |            1308.1 |                      139.7 |
| macro/string-10kb   |      2803.0 |       2069.3 |            54255.0 |         710.5 |             369.6 |                       55.5 |
| macro/eligibility   |       252.3 |      29876.1 |             9141.5 |        6455.7 |            4551.5 |                     1025.6 |
| arithmetic mean     |     24935.9 |     799437.0 |           440357.8 |      276558.6 |           53251.2 |                    12330.8 |
| geometric mean      |      1408.2 |      56144.5 |            46895.8 |        8670.1 |            4205.5 |                      866.3 |

* partial coverage — subject errored on some cases in this suite.

=== Pairwise shared-suite ratios ===

Geomean of per-suite ns/op ratios, computed only over suites where both
subjects have finite cells. The per-column mean rows above cover different
suite subsets when a subject errors; these ratios never mix subsets.

  jsonlogic-rs                   39.9x slower than dlrs:engine                over  6 shared suites
  dlrs:wasm:compiled             33.3x slower than dlrs:engine                over  6 shared suites
  json-logic-js                   6.2x slower than dlrs:engine                over  6 shared suites
  json-logic-engine               3.0x slower than dlrs:engine                over  6 shared suites
  dlrs:engine                     1.6x slower than json-logic-engine:compiled over  6 shared suites
  jsonlogic-rs                    1.2x slower than dlrs:wasm:compiled         over  6 shared suites
  jsonlogic-rs                    6.5x slower than json-logic-js              over  6 shared suites
  jsonlogic-rs                   13.4x slower than json-logic-engine          over  6 shared suites
  jsonlogic-rs                   64.8x slower than json-logic-engine:compiled over  6 shared suites
  dlrs:wasm:compiled              5.4x slower than json-logic-js              over  6 shared suites
  dlrs:wasm:compiled             11.2x slower than json-logic-engine          over  6 shared suites
  dlrs:wasm:compiled             54.1x slower than json-logic-engine:compiled over  6 shared suites
  json-logic-js                   2.1x slower than json-logic-engine          over  6 shared suites
  json-logic-js                  10.0x slower than json-logic-engine:compiled over  6 shared suites
  json-logic-engine               4.9x slower than json-logic-engine:compiled over  6 shared suites
```

Reading the macro matrix honestly:

- `dlrs:engine` matches its self-benchmark macro numbers within a few
  percent (e.g. `macro/array-10k` at about 136 microseconds per op,
  roughly 14 ns per element), so the two tiers cross-validate.
- **Partial cells skip real work.** None of the JS subjects implement
  the non-spec `sort` operator, so their `*` cells on the two array
  suites replace the most expensive case with a cheap throw. That
  deflates their array averages relative to the subjects that actually
  sort (`dlrs:engine`, `dlrs:wasm:compiled`), and it flatters
  `json-logic-engine:compiled` in the ratio table above.
- `jsonlogic-rs` shows full coverage on the array suites but does no
  sorting either: it treats an object whose key is not a known
  operation as a raw literal and returns it unchanged, so the `sort`
  case "succeeds" without touching the array.
- `macro/string-10kb` is the one suite the JS engines win outright. V8
  represents concatenation and slicing as rope/sliced strings (O(1))
  where `dlrs:engine` materialises a 20 KB `cat` result per eval;
  `json-logic-engine:compiled` at ~55 ns/op is measuring V8's lazy
  string machinery, not byte copies.
- `dlrs:wasm:compiled` pays the V8-to-WASM string marshalling per call,
  and that cost scales with payload size: ~54 microseconds per op on
  `string-10kb` and ~2.3 ms per op on `array-10k` are boundary cost,
  not engine cost.

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
  above says nothing about 1k+-element arrays, 100+-key objects, or
  deep nesting; those are covered by the [macro tier](#macro-tier),
  both self-only (`self --macro`) and cross-engine (`compare --macro`).
  The honest micro headline: single-digit nanoseconds for folded/scalar
  rules, 10-120 ns for context-dependent rules.
- Local-only by design — never run in CI.

## Optimization notes: completed passes and deferred designs

All nine candidates from the 2026-07 performance backlog are implemented.
First wave: zero-copy `substr`/string-`slice` (-45% slice suite),
arena-backed sort scratch (-25%), `itoa`/`ryu` number rendering (parity-
gated), split context stack (map -17%, scopes -16%). Second wave:
borrowed thrown-value channel + interned NaN (try.json -32%,
try.extra.json -49%), ISO-datetime byte-compare fast path (var-driven
datetime compares -37%, naive forms -92%), compile-time literal
pre-conversion via `self_cell` (50-element literal `in` lists -51%,
which also fixed a latent switch-case-table bug), optimistic ordered
probe for wide objects (object-128key -48%), and `Error` shrunk
80 -> 40 bytes (every operator `Result` slot halved). Guardrails for
future work: the conformance suite, the optimized-vs-traced differential
property test, `tests/layout_test.rs`, and the folded / non-folded split
above (quote the non-folded geomean).

Deferred designs, recorded for future owners:

- **Escaping-throw deferral**: throws that escape to the API boundary
  still build the owned payload eagerly; materializing from the context
  slot at the two boundaries (`engine/mod.rs`, `trace.rs`) would recover
  that cost (~3 lines per boundary plus a `pub(crate)` materialize
  helper). Would also claw back most of the meta-box cost below on
  throw-heavy profiles.
- **The `Error` meta box (tried, measured, reverted)**: boxing the
  operator/path metadata shrank `Error` to 40 bytes but cost one 48-byte
  box (~14 ns) per boundary-escaping error, regressing error-dense
  suites 15-45%; corpus geomean was +3.9% with the box vs -6.3% without,
  so the inline layout shipped. Notable datapoint for future work: the
  box variant was 25-30% FASTER on deep Ok-path suites (scopes, val,
  string), meaning those paths are sensitive to the `Result` slot width;
  a targeted fix (e.g. `Option`-returning hot helpers with cold `Err`
  construction at the edges) could capture that win without taxing the
  error path.
- **Open integration regression**: relative to the pre-second-wave
  baseline, three suites regressed with the combined second-wave changes
  (scopes +28%, string/string +33%, val.extra +13%) through an
  interaction not attributable to a single candidate (each change
  measured clean in isolation); the corpus still nets -6.3%. Worth a
  dedicated bisect: suspects are the context-stack catch/thrown fields,
  the literal-dispatch tag branch, and codegen layout shifts.
- **`ErrorKind` below 32 bytes** requires boxing the public
  `Thrown(OwnedDataValue)` payload: breaking (next major); would take
  `ErrorKind` to 16 and `Error` to 24.
- **Comparison/datetime NaN sites** still build owned payloads eagerly
  when caught; switching them to `Error::nan_at` needs `ctx` threaded
  through the `compare_equals`/`compare_ordered` fan-out (invasive, low
  value until a profile shows it).
- **datavalue object equality** (sibling crate, read-only from here):
  the `PartialEq` object arm is O(n^2) (two 131-pair objects cost
  ~21.5 us via `===`) and is asymmetric under duplicate keys, which
  violates the `PartialEq` contract. Recommended: same-order zip fast
  path now; key canonicalization or a sorted-flag variant as the
  long-term fix (both breaking for the sibling crate).
