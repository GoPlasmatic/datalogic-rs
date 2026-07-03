# Benchmark reference

Cross-library JSONLogic matrix produced by
`tools/benchmark/src/bin/compare.rs`. This file is the canonical
performance reference — link to it from other docs (README, blog posts,
changelog) rather than re-quoting numbers inline, so updates only need
one place.

This matrix measures **engine cost** (pre-parsed inputs, compile-once
subjects). For the per-call **boundary cost of each language binding**
(Node, Python, WASM, C, Go, JVM, .NET, PHP), decomposed per API tier
(string / data handle / batch), see
[BINDINGS-OVERHEAD.md](./BINDINGS-OVERHEAD.md).

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
| compatible.json                   |         8.5 |        438.9 |              590.3 |         241.7 |             110.6 |                       60.2 |
| arithmetic/plus.json              |         3.1 |       222.6* |              506.0 |        374.8* |             112.4 |                       30.7 |
| arithmetic/multiply.json          |         3.1 |       210.1* |              466.1 |        501.5* |             102.9 |                       31.4 |
| arithmetic/minus.json             |         3.4 |       142.1* |              699.8 |        366.3* |             153.5 |                       32.0 |
| arithmetic/divide.json            |         3.6 |          ERR |              540.3 |        344.5* |             104.9 |                       31.1 |
| arithmetic/modulo.json            |         3.5 |       183.0* |              651.1 |        425.6* |             155.6 |                       34.4 |
| arithmetic/min.json               |        13.7 |       358.6* |              976.8 |       1461.4* |             203.2 |                       36.7 |
| arithmetic/max.json               |        13.6 |       355.5* |              995.4 |       1491.1* |             213.0 |                       38.1 |
| arithmetic/chain.json             |        25.2 |          ERR |             2075.3 |           ERR |             433.3 |                      108.5 |
| comparison/softEquals.json        |         2.8 |       138.8* |              416.9 |        291.0* |              97.4 |                       31.8 |
| comparison/strictEquals.json      |         2.7 |       139.3* |              412.1 |        285.2* |              66.1 |                       26.8 |
| comparison/softNotEquals.json     |         2.8 |       141.6* |              440.7 |        299.4* |              88.9 |                       31.5 |
| comparison/strictNotEquals.json   |         2.7 |       141.7* |              419.1 |        281.3* |              68.2 |                       26.5 |
| comparison/greaterThan.json       |         2.7 |        195.6 |              423.4 |        291.2* |              84.2 |                       30.5 |
| comparison/greaterThanEquals.json |         2.7 |        202.0 |              498.7 |        365.8* |             107.3 |                       29.9 |
| comparison/lessThan.json          |         2.6 |        192.4 |              396.6 |        239.6* |              53.9 |                       22.9 |
| comparison/lessThanEquals.json    |         2.9 |        197.6 |              553.3 |        459.4* |              98.5 |                       30.2 |
| control/if.json                   |         2.7 |        246.3 |              469.9 |        275.2* |              69.6 |                       32.2 |
| control/and.json                  |         2.6 |       149.1* |              413.2 |         178.9 |             123.8 |                       32.7 |
| control/or.json                   |         2.8 |       148.9* |              404.7 |        358.9* |             122.7 |                       31.3 |
| control/switch.json               |        24.8 |       419.3* |              764.5 |           ERR |               ERR |                          — |
| truthiness.json                   |         4.3 |        156.9 |              583.4 |       1420.6* |             189.0 |                       36.5 |
| additional.json                   |        13.4 |       378.6* |             2015.9 |           ERR |             752.7 |                       78.0 |
| coalesce.json                     |         5.5 |        169.1 |              766.7 |           ERR |             207.4 |                       30.0 |
| chained.json                      |        36.8 |       516.6* |             2272.3 |           ERR |             615.6 |                      106.4 |
| exists.json                       |         7.1 |        122.7 |              995.4 |           ERR |             332.2 |                       66.7 |
| val.json                          |         7.8 |       173.2* |              938.9 |           ERR |             204.3 |                       31.8 |
| val-compat.json                   |        14.7 |          ERR |             1208.3 |           ERR |             333.4 |                      130.0 |
| val.extra.json                    |        61.8 |      1008.1* |             1988.6 |           ERR |             657.1 |                      219.7 |
| scopes.json                       |        98.0 |          ERR |             3899.1 |           ERR |            1902.1 |                      424.4 |
| empty-objects.json                |         3.2 |         22.9 |             1075.4 |         118.9 |             217.8 |                       47.1 |
| structured-objects.json           |           — |        367.2 |                ERR |       1355.7* |               ERR |                        ERR |
| try.json                          |        52.5 |        354.2 |             1789.5 |           ERR |             504.2 |                      129.9 |
| try.extra.json                    |        58.3 |        473.9 |             1919.7 |           ERR |            7562.8 |                      287.2 |
| datetime/datetime.json            |         9.4 |       379.9* |              771.4 |           ERR |               ERR |                          — |
| datetime/duration.json            |        12.6 |       341.7* |              782.7 |           ERR |               ERR |                        ERR |
| datetime/now.json                 |       117.3 |        357.9 |             2502.1 |           ERR |               ERR |                          — |
| length.json                       |        11.0 |        289.3 |             1451.9 |           ERR |             374.5 |                      244.7 |
| sort.json                         |        43.2 |       251.8* |             1888.4 |           ERR |               ERR |                          — |
| slice.json                        |        33.9 |       225.1* |             1172.5 |           ERR |               ERR |                          — |
| array/map.json                    |        75.9 |          ERR |             2787.1 |           ERR |           5120.3* |                    2939.1* |
| array/merge.json                  |        13.2 |        223.9 |              660.8 |         403.2 |             143.7 |                       19.9 |
| array/reduce.json                 |        41.4 |      3723.3* |             2101.9 |       1800.3* |             710.0 |                     286.8* |
| string/string.json                |        22.4 |        189.3 |             1098.7 |           ERR |               ERR |                          — |
| arithmetic/abs.json               |         3.4 |       154.4* |              886.4 |           ERR |               ERR |                          — |
| arithmetic/ceil.json              |         3.2 |       139.8* |              780.8 |           ERR |               ERR |                          — |
| arithmetic/floor.json             |         3.2 |       142.4* |              789.2 |           ERR |               ERR |                          — |
| flagd/fractional.json             |        36.9 |        483.0 |             1427.6 |           ERR |               ERR |                          — |
| flagd/sem_ver.json                |         6.9 |        279.8 |              605.6 |           ERR |               ERR |                          — |
| type.json                         |         6.9 |        165.5 |              879.7 |           ERR |               ERR |                          — |
| arithmetic mean                   |        19.1 |        340.3 |             1084.8 |         568.0 |             622.1 |                      161.3 |
| geometric mean                    |         9.2 |        239.5 |              890.9 |         426.1 |             229.6 |                       59.5 |
```

`*` partial coverage — subject errored on some cases in this suite.

### Pairwise shared-suite ratios

Quote these instead of dividing the per-column geomeans; each pair is
computed only over the suites both subjects completed.

```
  jsonlogic-rs                   28.9x slower than dlrs:engine                over 44 shared suites
  dlrs:wasm:compiled             96.8x slower than dlrs:engine                over 49 shared suites
  json-logic-js                  95.2x slower than dlrs:engine                over 23 shared suites
  json-logic-engine              28.9x slower than dlrs:engine                over 36 shared suites
  json-logic-engine:compiled      7.5x slower than dlrs:engine                over 36 shared suites
  dlrs:wasm:compiled              3.5x slower than jsonlogic-rs               over 44 shared suites
  json-logic-js                   2.1x slower than jsonlogic-rs               over 23 shared suites
  jsonlogic-rs                    1.2x slower than json-logic-engine          over 31 shared suites
  jsonlogic-rs                    4.8x slower than json-logic-engine:compiled over 31 shared suites
  dlrs:wasm:compiled              1.4x slower than json-logic-js              over 23 shared suites
  dlrs:wasm:compiled              3.7x slower than json-logic-engine          over 36 shared suites
  dlrs:wasm:compiled             14.3x slower than json-logic-engine:compiled over 36 shared suites
  json-logic-js                   3.3x slower than json-logic-engine          over 23 shared suites
  json-logic-js                  11.5x slower than json-logic-engine:compiled over 23 shared suites
  json-logic-engine               3.9x slower than json-logic-engine:compiled over 36 shared suites
```

## Quick reading

Geomeans across the 50 timed suites (53 discovered; the 3 negative-only
suites skip). Lower is better:

| Subject                        | Geomean ns/op (own suite set) | Pairwise vs `dlrs:engine` (shared suites) |
|--------------------------------|------------------------------:|------------------------------------------:|
| `dlrs:engine`                  |                           9.2 | 1.0×                                       |
| `json-logic-engine:compiled`   |                          59.5 | 7.5× (36)                                  |
| `json-logic-engine` (interp.)  |                         229.6 | 28.9× (36)                                 |
| `jsonlogic-rs`                 |                         239.5 | 28.9× (44)                                 |
| `json-logic-js`                |                         426.1 | 95.2× (23)                                 |
| `dlrs:wasm:compiled`           |                         890.9 | 96.8× (49)                                 |

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
  same order of magnitude. Still ~7.5× behind `dlrs:engine` but far
  ahead of the reference `json-logic-js`.
- **`dlrs:wasm:compiled` (96.8× pairwise)** — the cost is
  the V8↔WASM boundary on every call (data marshall + JSON parse + eval +
  result stringify + result marshall). Eval itself is fast; the
  per-call string contract is what hurts — which is why the binding
  also ships a parse-once `DataHandle` tier that removes the payload
  copy + parse per call (8.3x at 8 KB; measured per tier in
  [BINDINGS-OVERHEAD.md](./BINDINGS-OVERHEAD.md)). This matrix
  deliberately keeps the string tier as the wasm column: it is the
  comparable per-call contract the other subjects use.
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
| `macro/checkout-40`   | Realistic 40-item checkout decision (4.7 KB rule, ~26 distinct operators, 6 KB payload): completeness, risk screen, cart validation, promo pricing with cap, weight-based shipping, loyalty adjustment — **spec-compatible operators only**, so every subject runs 100% of it; the fair cross-engine row |

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
evaluation: `macro/array-10k` at ~130 µs/op means one filter/map/sort
pass over 10k elements costs ~130 µs, i.e. ~13 ns per element touched.

### Cross-engine macro matrix

The same suites also run across every subject of the matrix above:

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
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 7 suites) ===

| Suite               | dlrs:engine | jsonlogic-rs | dlrs:wasm:compiled | json-logic-js | json-logic-engine | json-logic-engine:compiled |
|---------------------|------------:|-------------:|-------------------:|--------------:|------------------:|---------------------------:|
| macro/array-1k      |     10004.8 |     373201.5 |           223387.7 |     154172.1* |          30812.3* |                    7176.8* |
| macro/array-10k     |    130701.7 |    4361015.9 |          2327601.2 |    1465646.6* |         287489.3* |                   67216.9* |
| macro/object-128key |        66.3 |       5247.7 |             9359.9 |         286.6 |             251.5 |                      106.6 |
| macro/deep-48       |       121.2 |      60965.4 |             4301.9 |        1406.6 |            1324.5 |                      141.4 |
| macro/string-10kb   |      2809.0 |       2193.3 |            54813.4 |         740.1 |             377.9 |                       51.4 |
| macro/eligibility   |       252.6 |      28655.8 |             9110.0 |        6775.2 |            5151.1 |                     1105.2 |
| macro/checkout-40   |     28785.9 |    2270752.0 |           306637.4 |      556300.6 |           92825.9 |                    27722.1 |
| arithmetic mean     |     24677.4 |    1014575.9 |           419315.9 |      312189.7 |           59747.5 |                    14788.6 |
| geometric mean      |      2153.5 |      95847.7 |            61173.2 |       15875.0 |            6578.6 |                     1416.6 |

* partial coverage — subject errored on some cases in this suite.

=== Pairwise shared-suite ratios ===

Geomean of per-suite ns/op ratios, computed only over suites where both
subjects have finite cells. The per-column mean rows above cover different
suite subsets when a subject errors; these ratios never mix subsets.

  jsonlogic-rs                   44.5x slower than dlrs:engine                over  7 shared suites
  dlrs:wasm:compiled             28.4x slower than dlrs:engine                over  7 shared suites
  json-logic-js                   7.4x slower than dlrs:engine                over  7 shared suites
  json-logic-engine               3.1x slower than dlrs:engine                over  7 shared suites
  dlrs:engine                     1.5x slower than json-logic-engine:compiled over  7 shared suites
  jsonlogic-rs                    1.6x slower than dlrs:wasm:compiled         over  7 shared suites
  jsonlogic-rs                    6.0x slower than json-logic-js              over  7 shared suites
  jsonlogic-rs                   14.6x slower than json-logic-engine          over  7 shared suites
  jsonlogic-rs                   67.7x slower than json-logic-engine:compiled over  7 shared suites
  dlrs:wasm:compiled              3.9x slower than json-logic-js              over  7 shared suites
  dlrs:wasm:compiled              9.3x slower than json-logic-engine          over  7 shared suites
  dlrs:wasm:compiled             43.2x slower than json-logic-engine:compiled over  7 shared suites
  json-logic-js                   2.4x slower than json-logic-engine          over  7 shared suites
  json-logic-js                  11.2x slower than json-logic-engine:compiled over  7 shared suites
  json-logic-engine               4.6x slower than json-logic-engine:compiled over  7 shared suites
```

Reading the macro matrix honestly:

- `dlrs:engine` matches its self-benchmark macro numbers within a few
  percent (e.g. `macro/array-10k` at about 131 microseconds per op,
  roughly 13 ns per element), so the two tiers cross-validate.
- **`macro/checkout-40` is the fair real-world row**: a large rule
  built only from spec-compatible operators, byte-identical results
  verified across engines, and full coverage in every column. On it,
  `dlrs:engine` (28.8 µs) and `json-logic-engine:compiled` (27.7 µs)
  are a statistical tie (the gap sits inside the ±5% run noise), with
  every other subject 3.1x to 79x slower. The rule is
  iterator-dominated — map/filter/reduce over 40-row object arrays,
  with aggregates recomputed where referenced because pure-spec
  JSONLogic has no local bindings (identical work for every engine) —
  and per-row closure calls are exactly where a warmed JIT closes the
  gap on native dispatch. dlrs's 7.5x micro-matrix lead lives on
  dispatch-heavy rules; on per-row iteration the honest label is
  "tied with the best JS engine, far ahead of everything else".
- **Partial cells skip real work.** None of the JS subjects implement
  the non-spec `sort` operator, so their `*` cells on the two array
  suites replace the most expensive case with a cheap throw (~1.6 µs)
  while `dlrs:engine` actually sorts (437 µs of its per-case budget at
  10k). Reconstructing a fair six-case comparison from per-case
  measurements puts dlrs and `json-logic-engine:compiled` at parity on
  both array suites (75.9 vs 76.4 µs per case at 10k) — the apparent
  jle array wins in the ratio table are entirely the skipped sort.
- `jsonlogic-rs` shows full coverage on the array suites but does no
  sorting either: it treats an object whose key is not a known
  operation as a raw literal and returns it unchanged, so the `sort`
  case "succeeds" without touching the array.
- `macro/string-10kb` is the one suite the JS engines win outright,
  and per-case measurement shows why: 96% of dlrs's cost is the two
  `substr` cases (4.0 and 6.8 µs each), which pay O(n) UTF-8
  char-boundary scans to honour character indexing, where V8's UTF-16
  sliced strings slice in O(1) (~15 ns). `cat` is most of the
  remainder: dlrs materialises the 20 KB result (408 ns) while V8
  builds a lazy rope (10 ns) that the benchmark sink never forces
  flat. The `in` case is trivial for everyone (the needle occurs
  27 bytes into the haystack). An ASCII fast path for `substr` offset
  math would collapse most of this row.
- The pairwise line "`dlrs:engine` 1.5x slower than
  `json-logic-engine:compiled`" is shape-dominated, not a speed
  verdict: it averages two sort-skewed array cells and the
  string-10kb outlier against four rows dlrs wins or ties. Remove the
  asymmetric cells and the same computation favors dlrs on every
  remaining suite except the checkout tie.
- `dlrs:wasm:compiled` pays the V8-to-WASM string marshalling per call,
  and that cost scales with payload size: ~55 microseconds per op on
  `string-10kb` and ~2.3 ms per op on `array-10k` are boundary cost,
  not engine cost (the binding's parse-once `DataHandle` tier exists
  for exactly this; see
  [BINDINGS-OVERHEAD.md](./BINDINGS-OVERHEAD.md)).

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

## Optimization provenance

The current numbers include the 2026-07 optimization passes: zero-copy
`substr`/string-`slice`, arena-backed sort scratch, `itoa`/`ryu` number
rendering, a split context stack, a borrowed thrown-value channel with
interned NaN, an ISO-datetime byte-compare fast path, compile-time
literal pre-conversion via `self_cell`, an optimistic ordered probe for
wide objects, and an `Error` layout shrunk 80 -> 40 bytes. Guardrails
that keep future optimization honest: the conformance suite, the
optimized-vs-traced differential property test,
`tests/layout_test.rs`, and the folded / non-folded split above (quote
the non-folded geomean when the claim is about data-dependent rules).
Engineering notes on candidates that were tried-and-reverted or
deferred were removed from this file during the 5.0.1 docs cleanup;
they live in this file's git history.
