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
| compatible.json                   |         7.5 |        465.7 |              818.2 |         240.9 |             110.0 |                       62.5 |
| arithmetic/plus.json              |         2.9 |       230.9* |              504.1 |        394.6* |             110.4 |                       30.6 |
| arithmetic/multiply.json          |         2.9 |       214.4* |              484.2 |        519.0* |             106.1 |                       31.1 |
| arithmetic/minus.json             |         3.2 |       144.5* |              716.0 |        364.7* |             155.0 |                       32.1 |
| arithmetic/divide.json            |         3.5 |          ERR |              533.1 |        351.2* |             111.2 |                       32.3 |
| arithmetic/modulo.json            |         3.3 |       186.8* |              675.5 |        439.1* |             177.2 |                       30.7 |
| arithmetic/min.json               |        13.8 |       364.4* |              983.3 |       1505.8* |             303.8 |                       38.6 |
| arithmetic/max.json               |        13.7 |       365.1* |              978.2 |       1500.8* |             207.1 |                       38.1 |
| arithmetic/chain.json             |        25.6 |          ERR |             2024.8 |           ERR |             465.5 |                      109.6 |
| comparison/softEquals.json        |         2.6 |       142.2* |              428.8 |        302.6* |              76.8 |                       29.6 |
| comparison/strictEquals.json      |         2.5 |       140.7* |              418.6 |        279.3* |              65.8 |                       25.4 |
| comparison/softNotEquals.json     |         2.6 |       144.8* |              426.5 |        302.8* |              91.8 |                       31.3 |
| comparison/strictNotEquals.json   |         2.5 |       144.6* |              428.7 |        288.3* |              93.7 |                       28.5 |
| comparison/greaterThan.json       |         2.5 |        200.1 |              440.8 |        301.4* |              77.5 |                       30.2 |
| comparison/greaterThanEquals.json |         2.5 |        207.6 |              482.0 |        359.3* |             108.6 |                       29.1 |
| comparison/lessThan.json          |         2.4 |        195.8 |              406.7 |        241.7* |              64.1 |                       23.4 |
| comparison/lessThanEquals.json    |         2.6 |        199.2 |              554.5 |        470.9* |             149.3 |                       32.2 |
| control/if.json                   |         2.3 |        249.8 |              471.5 |        277.1* |              73.7 |                       32.2 |
| control/and.json                  |         2.3 |       152.4* |              423.8 |         202.8 |             127.7 |                       32.7 |
| control/or.json                   |         2.5 |       150.9* |              415.8 |        364.1* |             121.8 |                       33.2 |
| control/switch.json               |        24.8 |       432.7* |              786.2 |           ERR |               ERR |                          — |
| truthiness.json                   |         4.0 |        161.9 |              588.8 |       1445.1* |             191.7 |                       37.6 |
| additional.json                   |        13.8 |       385.8* |             1829.4 |           ERR |             699.2 |                       80.2 |
| coalesce.json                     |         5.4 |        172.8 |              756.9 |           ERR |             194.0 |                       31.3 |
| chained.json                      |        37.2 |       526.5* |             2377.9 |           ERR |             626.2 |                      111.7 |
| exists.json                       |         7.1 |        125.4 |              882.3 |           ERR |             347.8 |                       70.4 |
| val.json                          |         8.0 |       174.5* |              967.2 |           ERR |             207.1 |                       30.3 |
| val-compat.json                   |        15.5 |          ERR |             1199.1 |           ERR |             344.5 |                      130.1 |
| val.extra.json                    |        64.3 |      1029.3* |             2026.3 |           ERR |             564.4 |                      220.3 |
| scopes.json                       |       102.6 |          ERR |             3565.7 |           ERR |            1914.9 |                      409.8 |
| empty-objects.json                |         3.0 |         23.8 |             1155.7 |         122.1 |             222.2 |                       47.2 |
| structured-objects.json           |           — |        378.5 |                ERR |       1357.9* |               ERR |                        ERR |
| try.json                          |        52.7 |        364.1 |             1817.9 |           ERR |             536.9 |                      129.7 |
| try.extra.json                    |        60.0 |        488.5 |             1937.1 |           ERR |            7612.5 |                      305.3 |
| datetime/datetime.json            |         9.5 |       389.4* |              759.0 |           ERR |               ERR |                          — |
| datetime/duration.json            |        12.5 |       349.8* |              806.8 |           ERR |               ERR |                        ERR |
| datetime/now.json                 |       121.0 |        371.3 |             2620.6 |           ERR |               ERR |                          — |
| length.json                       |        11.2 |        294.9 |             1475.0 |           ERR |             334.8 |                      256.3 |
| sort.json                         |        43.0 |       257.5* |             1825.3 |           ERR |               ERR |                          — |
| slice.json                        |        33.9 |       230.3* |             1169.5 |           ERR |               ERR |                          — |
| array/map.json                    |        78.5 |          ERR |             2642.4 |           ERR |           5287.3* |                    3023.0* |
| array/merge.json                  |        13.2 |        226.9 |              759.6 |         413.7 |              96.9 |                       22.6 |
| array/reduce.json                 |        41.1 |      3810.4* |             2103.4 |       1802.5* |             745.7 |                     297.3* |
| string/string.json                |        23.1 |        194.0 |             1057.2 |           ERR |               ERR |                          — |
| arithmetic/abs.json               |         3.2 |       157.4* |              902.2 |           ERR |               ERR |                          — |
| arithmetic/ceil.json              |         3.0 |       142.6* |              791.4 |           ERR |               ERR |                          — |
| arithmetic/floor.json             |         3.0 |       145.2* |              838.4 |           ERR |               ERR |                          — |
| flagd/fractional.json             |        37.2 |        487.7 |             1472.0 |           ERR |               ERR |                          — |
| flagd/sem_ver.json                |         6.5 |        285.9 |              582.1 |           ERR |               ERR |                          — |
| type.json                         |         6.8 |        166.6 |              874.0 |           ERR |               ERR |                          — |
| arithmetic mean                   |        19.4 |        348.3 |             1085.4 |         577.0 |             631.2 |                      164.9 |
| geometric mean                    |         8.9 |        244.9 |              901.1 |         434.8 |             235.2 |                       60.3 |
```

`*` partial coverage — subject errored on some cases in this suite.

### Pairwise shared-suite ratios

Quote these instead of dividing the per-column geomeans; each pair is
computed only over the suites both subjects completed.

```
  jsonlogic-rs                   30.6x slower than dlrs:engine                over 44 shared suites
  dlrs:wasm:compiled            100.9x slower than dlrs:engine                over 49 shared suites
  json-logic-js                 104.2x slower than dlrs:engine                over 23 shared suites
  json-logic-engine              30.7x slower than dlrs:engine                over 36 shared suites
  json-logic-engine:compiled      7.9x slower than dlrs:engine                over 36 shared suites
  dlrs:wasm:compiled              3.5x slower than jsonlogic-rs               over 44 shared suites
  json-logic-js                   2.0x slower than jsonlogic-rs               over 23 shared suites
  jsonlogic-rs                    1.2x slower than json-logic-engine          over 31 shared suites
  jsonlogic-rs                    4.9x slower than json-logic-engine:compiled over 31 shared suites
  dlrs:wasm:compiled              1.4x slower than json-logic-js              over 23 shared suites
  dlrs:wasm:compiled              3.7x slower than json-logic-engine          over 36 shared suites
  dlrs:wasm:compiled             14.3x slower than json-logic-engine:compiled over 36 shared suites
  json-logic-js                   3.2x slower than json-logic-engine          over 23 shared suites
  json-logic-js                  11.6x slower than json-logic-engine:compiled over 23 shared suites
  json-logic-engine               3.9x slower than json-logic-engine:compiled over 36 shared suites
```

## Quick reading

Geomeans across the 50 timed suites (53 discovered; the 3 negative-only
suites skip). Lower is better:

| Subject                        | Geomean ns/op (own suite set) | Pairwise vs `dlrs:engine` (shared suites) |
|--------------------------------|------------------------------:|------------------------------------------:|
| `dlrs:engine`                  |                           8.9 | 1.0×                                       |
| `json-logic-engine:compiled`   |                          60.3 | 7.9× (36)                                  |
| `json-logic-engine` (interp.)  |                         235.2 | 30.7× (36)                                 |
| `jsonlogic-rs`                 |                         244.9 | 30.6× (44)                                 |
| `json-logic-js`                |                         434.8 | 104.2× (23)                                |
| `dlrs:wasm:compiled`           |                         901.1 | 100.9× (49)                                |

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
| macro/array-1k      |      5364.6 |     370736.7 |           224809.5 |     156076.5* |          31560.5* |                    7354.1* |
| macro/array-10k     |     69232.2 |    4322416.7 |          2342331.6 |    1476459.8* |         281423.2* |                   66324.0* |
| macro/object-128key |        66.1 |       5197.9 |             9474.7 |         278.5 |             251.2 |                      118.3 |
| macro/deep-48       |       121.8 |      61208.2 |             4759.8 |        1449.5 |            1315.7 |                      153.4 |
| macro/string-10kb   |       321.2 |       2380.8 |            54832.0 |         844.5 |             505.9 |                       53.0 |
| macro/eligibility   |       260.5 |      29847.8 |             9760.5 |        6763.2 |            4892.8 |                     1024.3 |
| macro/checkout-40   |      9456.1 |    2253488.1 |           309804.5 |      555129.2 |          100102.1 |                    41343.0 |
| arithmetic mean     |     12117.5 |    1006468.0 |           422253.2 |      313857.3 |           60007.4 |                    16624.3 |
| geometric mean      |      1131.0 |      97144.3 |            62996.5 |       16216.4 |            6877.1 |                     1532.7 |

* partial coverage — subject errored on some cases in this suite.

=== Pairwise shared-suite ratios ===

Geomean of per-suite ns/op ratios, computed only over suites where both
subjects have finite cells. The per-column mean rows above cover different
suite subsets when a subject errors; these ratios never mix subsets.

  jsonlogic-rs                   85.9x slower than dlrs:engine                over  7 shared suites
  dlrs:wasm:compiled             55.7x slower than dlrs:engine                over  7 shared suites
  json-logic-js                  14.3x slower than dlrs:engine                over  7 shared suites
  json-logic-engine               6.1x slower than dlrs:engine                over  7 shared suites
  json-logic-engine:compiled      1.4x slower than dlrs:engine                over  7 shared suites
  jsonlogic-rs                    1.5x slower than dlrs:wasm:compiled         over  7 shared suites
  jsonlogic-rs                    6.0x slower than json-logic-js              over  7 shared suites
  jsonlogic-rs                   14.1x slower than json-logic-engine          over  7 shared suites
  jsonlogic-rs                   63.4x slower than json-logic-engine:compiled over  7 shared suites
  dlrs:wasm:compiled              3.9x slower than json-logic-js              over  7 shared suites
  dlrs:wasm:compiled              9.2x slower than json-logic-engine          over  7 shared suites
  dlrs:wasm:compiled             41.1x slower than json-logic-engine:compiled over  7 shared suites
  json-logic-js                   2.4x slower than json-logic-engine          over  7 shared suites
  json-logic-js                  10.6x slower than json-logic-engine:compiled over  7 shared suites
  json-logic-engine               4.5x slower than json-logic-engine:compiled over  7 shared suites
```

Reading the macro matrix honestly:

- `dlrs:engine` matches its self-benchmark macro numbers within a few
  percent (e.g. `macro/array-10k` at about 69 microseconds per op,
  roughly 7 ns per element), so the two tiers cross-validate.
- **`macro/checkout-40` is the fair real-world row**: a large rule
  built only from spec-compatible operators, byte-identical results
  verified across engines, and full coverage in every column. On it,
  `dlrs:engine` (9.5 µs) leads `json-logic-engine:compiled` (27.7 to
  41.3 µs across captures — its macro cells jitter with V8
  inline-cache state) by 2.9x or better, with every other subject an
  order of magnitude behind. Earlier captures had this row as a
  statistical tie (28.8 vs 27.7 µs); the gap opened when the
  per-row iteration overhead was attacked directly: a var⊗var
  arithmetic map fast path (the `{"*": [unit_price, qty]}` line-total
  shape), compile-time-detected predicate trees for
  `and`/`or`/`!`/`in`/truthy-var filter and quantifier bodies, and a
  remembered-index field lookup for homogeneous rows (the arena analog
  of V8's monomorphic inline caches).
- **Partial cells skip real work.** None of the JS subjects implement
  the non-spec `sort` operator, so their `*` cells on the two array
  suites replace the most expensive case with a cheap throw (~1.6 µs)
  while `dlrs:engine` actually sorts (a numeric-key
  `sort_unstable` fast path keeps that case cheap now). Even with the
  sort case *included*, dlrs's array-10k cell (69.2 µs) sits at
  parity with jle:compiled's sort-free cell (66.3 µs), and its
  array-1k cell (5.4 µs) beats the corresponding sort-free 7.4 µs
  outright — the asymmetry now biases *against* dlrs and it wins
  anyway.
- `jsonlogic-rs` shows full coverage on the array suites but does no
  sorting either: it treats an object whose key is not a known
  operation as a raw literal and returns it unchanged, so the `sort`
  case "succeeds" without touching the array.
- `macro/string-10kb` is the one suite the JS engines still win, but
  the margin collapsed from 55x to 6x: `substr` now takes an ASCII
  fast path (byte-offset math plus a word-at-a-time `is_ascii` scan)
  instead of O(n) UTF-8 char-boundary walks, and the non-ASCII path
  only pays the full char count for negative offsets. What remains is
  structural: `cat` materialises the 20 KB result where V8 builds a
  lazy rope the benchmark sink never forces flat, and each `substr`
  re-scans for ASCII-ness where V8 keeps a one-byte-representation
  flag on the string object. Closing that would need a
  representation-level change (a cached ASCII bit or rope strings in
  `DataValue`).
- With all seven suites included — sort asymmetry, string-10kb outlier
  and all — the pairwise geomean now has `json-logic-engine:compiled`
  1.4x slower than `dlrs:engine`. The honest label for per-row
  iteration went from "tied with the best JS engine" to "ahead of the
  best JS engine on every row shape except large-string slicing".
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
`substr`/string-`slice` with an ASCII byte-offset fast path (and lazy
char counting on the non-ASCII path), arena-backed sort scratch plus a
numeric-key `sort_unstable` fast path with index tiebreaker,
`itoa`/`ryu` number rendering, a split context stack, a borrowed
thrown-value channel with interned NaN, an ISO-datetime byte-compare
fast path, compile-time literal pre-conversion via `self_cell`, an
optimistic ordered probe for wide objects, a var⊗var arithmetic map
fast path with remembered-index (inline-cache-style) field lookups,
compile-time-detected compound predicate trees
(`and`/`or`/`!`/`in`/truthy-var over comparison leaves) for
filter/quantifier bodies with an indeterminate-shape fallback that
also fixed the fast-path/general-path coercion divergences
(`"9" >= 2`, `"5" == 5`, `true == 1` inside `filter` previously
evaluated uncoerced), and an `Error` layout shrunk 80 -> 40 bytes. Guardrails
that keep future optimization honest: the conformance suite, the
optimized-vs-traced differential property test,
`tests/layout_test.rs`, and the folded / non-folded split above (quote
the non-folded geomean when the claim is about data-dependent rules).
Engineering notes on candidates that were tried-and-reverted or
deferred were removed from this file during the 5.0.1 docs cleanup;
they live in this file's git history.
