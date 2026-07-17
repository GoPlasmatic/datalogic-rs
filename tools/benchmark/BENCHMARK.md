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

> **Captured:** 2026-07-17  •  **Apple M2 Pro (arm64)** macOS 26.5 (Tahoe)
> •  Rust 1.97.0  •  Node v22.22.2  •  release build, no `target-cpu=native`
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
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 51 suites) ===

| Suite                             | dlrs:engine | jsonlogic-rs | dlrs:wasm:compiled | json-logic-js | json-logic-engine | json-logic-engine:compiled |
|-----------------------------------|------------:|-------------:|-------------------:|--------------:|------------------:|---------------------------:|
| compatible.json                   |         9.5 |        450.5 |              587.3 |         233.2 |             117.6 |                       61.1 |
| arithmetic/plus.json              |         3.4 |       234.8* |              509.0 |        373.0* |             105.7 |                       30.2 |
| arithmetic/multiply.json          |         3.4 |       218.7* |              453.0 |        502.3* |              96.3 |                       31.2 |
| arithmetic/minus.json             |         3.7 |       144.5* |              703.0 |        369.2* |             149.1 |                       31.4 |
| arithmetic/divide.json            |         3.9 |          ERR |              626.7 |        345.0* |             109.5 |                       31.6 |
| arithmetic/modulo.json            |         3.9 |       189.5* |              654.1 |        444.9* |             155.2 |                       31.6 |
| arithmetic/min.json               |        16.2 |       370.3* |             1040.8 |       1463.0* |             205.6 |                       38.6 |
| arithmetic/max.json               |        15.4 |       369.0* |              995.2 |       1485.2* |             208.7 |                       36.5 |
| arithmetic/chain.json             |        28.4 |          ERR |             2288.5 |           ERR |             427.9 |                      108.8 |
| comparison/softEquals.json        |         3.1 |       145.9* |              433.4 |        293.5* |              75.0 |                       29.3 |
| comparison/strictEquals.json      |         3.1 |       146.9* |              415.3 |        275.7* |              56.5 |                       23.7 |
| comparison/softNotEquals.json     |         3.1 |       148.9* |              428.7 |        304.3* |              79.8 |                       32.4 |
| comparison/strictNotEquals.json   |         3.0 |       150.6* |              420.1 |        295.2* |              76.9 |                       25.2 |
| comparison/greaterThan.json       |         3.0 |        204.9 |              428.4 |        290.7* |              77.8 |                       29.2 |
| comparison/greaterThanEquals.json |         3.2 |        210.7 |              484.1 |        353.8* |             109.7 |                       29.7 |
| comparison/lessThan.json          |         3.0 |        201.3 |              399.5 |        237.6* |              44.8 |                       23.1 |
| comparison/lessThanEquals.json    |         3.2 |        203.3 |              638.1 |        468.5* |             146.2 |                       31.0 |
| control/if.json                   |         3.6 |        262.4 |              497.7 |        273.4* |              69.9 |                       31.9 |
| control/and.json                  |         2.9 |       158.0* |              433.8 |         199.9 |             101.1 |                       32.2 |
| control/or.json                   |         3.2 |       157.5* |              425.1 |        356.3* |             117.7 |                       32.1 |
| control/switch.json               |        25.8 |       429.5* |              776.8 |           ERR |               ERR |                          — |
| truthiness.json                   |         4.6 |        164.7 |              616.6 |       1460.1* |             191.1 |                       35.9 |
| additional.json                   |        14.9 |       400.2* |             2055.0 |           ERR |             677.8 |                       83.2 |
| coalesce.json                     |         6.2 |        174.3 |              744.3 |           ERR |             190.0 |                       29.8 |
| chained.json                      |        39.0 |       528.9* |             2247.7 |           ERR |             676.8 |                      104.8 |
| exists.json                       |         7.7 |        128.4 |              867.7 |           ERR |             324.9 |                       64.8 |
| val.json                          |         8.7 |       178.3* |             1039.0 |           ERR |             195.3 |                       31.4 |
| val-compat.json                   |        16.2 |          ERR |             1202.8 |           ERR |             341.9 |                      133.5 |
| val.extra.json                    |        51.2 |      1033.3* |             1973.3 |           ERR |             555.6 |                      207.6 |
| scopes.json                       |        72.4 |          ERR |             3136.4 |           ERR |            1814.0 |                      402.1 |
| empty-objects.json                |         3.6 |         23.1 |             1074.7 |         120.1 |             194.0 |                       48.5 |
| structured-objects.json           |           — |        375.7 |                ERR |       1347.5* |               ERR |                        ERR |
| try.json                          |        53.8 |        365.2 |             1801.7 |           ERR |             464.3 |                      119.3 |
| try.extra.json                    |        60.4 |        485.8 |             2103.7 |           ERR |            7572.6 |                      323.6 |
| datetime/datetime.json            |         9.8 |       387.0* |              735.6 |           ERR |               ERR |                          — |
| datetime/duration.json            |        13.6 |       346.5* |              759.1 |           ERR |               ERR |                        ERR |
| datetime/now.json                 |       120.3 |        370.4 |             2421.2 |           ERR |               ERR |                          — |
| length.json                       |        11.5 |        299.1 |             1377.3 |           ERR |             335.9 |                      235.7 |
| sort.json                         |        40.5 |       259.4* |             1787.9 |           ERR |               ERR |                          — |
| slice.json                        |        34.3 |       231.7* |             1198.1 |           ERR |               ERR |                          — |
| array/map.json                    |        76.3 |          ERR |             2511.5 |           ERR |           5071.2* |                    2949.2* |
| array/merge.json                  |        13.7 |        234.1 |              637.6 |         405.7 |             139.1 |                       19.3 |
| array/reduce.json                 |        39.3 |      3152.2* |             1335.3 |       1446.0* |            916.8* |                     429.7* |
| string/string.json                |        23.4 |        194.6 |             1066.0 |           ERR |               ERR |                          — |
| arithmetic/abs.json               |         3.8 |       161.7* |              838.2 |           ERR |               ERR |                          — |
| arithmetic/ceil.json              |         3.6 |       144.3* |              751.2 |           ERR |               ERR |                          — |
| arithmetic/floor.json             |         3.6 |       146.6* |              777.9 |           ERR |               ERR |                          — |
| flagd/fractional.json             |        36.8 |        486.0 |             1343.9 |           ERR |               ERR |                          — |
| flagd/sem_ver.json                |         7.1 |        289.7 |              578.7 |           ERR |               ERR |                          — |
| type.json                         |         7.6 |        171.1 |              920.4 |           ERR |               ERR |                          — |
| cse.json                          |        69.7 |       5332.0 |             2531.7 |       4044.1* |            1440.0 |                      633.0 |
| arithmetic mean                   |        20.0 |        444.8 |             1081.5 |         695.5 |             638.7 |                      177.6 |
| geometric mean                    |        10.3 |        264.2 |              900.5 |         465.1 |             234.8 |                       63.3 |
```

`*` partial coverage — subject errored on some cases in this suite.

### Pairwise shared-suite ratios

Quote these instead of dividing the per-column geomeans; each pair is
computed only over the suites both subjects completed.

```
  jsonlogic-rs                   28.1x slower than dlrs:engine                over 45 shared suites
  dlrs:wasm:compiled             87.5x slower than dlrs:engine                over 50 shared suites
  json-logic-js                  83.6x slower than dlrs:engine                over 24 shared suites
  json-logic-engine              25.8x slower than dlrs:engine                over 37 shared suites
  json-logic-engine:compiled      7.0x slower than dlrs:engine                over 37 shared suites
  dlrs:wasm:compiled              3.2x slower than jsonlogic-rs               over 45 shared suites
  json-logic-js                   1.9x slower than jsonlogic-rs               over 24 shared suites
  jsonlogic-rs                    1.3x slower than json-logic-engine          over 32 shared suites
  jsonlogic-rs                    5.1x slower than json-logic-engine:compiled over 32 shared suites
  dlrs:wasm:compiled              1.4x slower than json-logic-js              over 24 shared suites
  dlrs:wasm:compiled              3.7x slower than json-logic-engine          over 37 shared suites
  dlrs:wasm:compiled             13.8x slower than json-logic-engine:compiled over 37 shared suites
  json-logic-js                   3.3x slower than json-logic-engine          over 24 shared suites
  json-logic-js                  11.1x slower than json-logic-engine:compiled over 24 shared suites
  json-logic-engine               3.7x slower than json-logic-engine:compiled over 37 shared suites
```

## Quick reading

Geomeans across the 51 timed suites (54 discovered; the 3 negative-only
suites skip). Lower is better:

| Subject                        | Geomean ns/op (own suite set) | Pairwise vs `dlrs:engine` (shared suites) |
|--------------------------------|------------------------------:|------------------------------------------:|
| `dlrs:engine`                  |                          10.3 | 1.0×                                       |
| `json-logic-engine:compiled`   |                          63.3 | 7.0× (37)                                  |
| `json-logic-engine` (interp.)  |                         234.8 | 25.8× (37)                                 |
| `jsonlogic-rs`                 |                         264.2 | 28.1× (45)                                 |
| `json-logic-js`                |                         465.1 | 83.6× (24)                                 |
| `dlrs:wasm:compiled`           |                         900.5 | 87.5× (50)                                 |

The geomean column aggregates whatever suites each subject completed, so
those numbers cover different suite subsets. The pairwise column (from
the ratio table under the matrix) compares only suites both subjects
ran; quote it when the ratio is the claim. `json-logic-js` shares just
the 24 spec-only suites, which is why its pairwise ratio lands above its
geomean quotient.

Headline takeaways:

- **`dlrs:engine` is the fastest cell on every suite it runs** — single-digit
  ns/op on basic arithmetic, comparison, and control-flow; double-digit on
  heavier `try` / `chained` / `scopes` patterns.
- **`json-logic-engine:compiled` (~63 ns) is the strongest non-dlrs
  contender** — a real, modern competitor and the only JS library in the
  same order of magnitude. Still ~7× behind `dlrs:engine` but far
  ahead of the reference `json-logic-js`.
- **`dlrs:wasm:compiled` (87.5× pairwise)** — the cost is
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

Captured 2026-07-17, Apple M2 Pro (arm64), Rust 1.97.0, Node v22.22.2,
release build from the repo root (no `target-cpu=native`):

```
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 7 suites) ===

| Suite               | dlrs:engine | jsonlogic-rs | dlrs:wasm:compiled | json-logic-js | json-logic-engine | json-logic-engine:compiled |
|---------------------|------------:|-------------:|-------------------:|--------------:|------------------:|---------------------------:|
| macro/array-1k      |      5316.7 |     373435.0 |           216483.0 |     152894.6* |          30750.4* |                    7027.5* |
| macro/array-10k     |     67196.0 |    4244439.5 |          2256062.5 |    1451931.6* |         277194.7* |                   64991.8* |
| macro/object-128key |        66.4 |       5309.1 |            10464.5 |         296.4 |             269.3 |                      108.0 |
| macro/deep-48       |       120.6 |      60721.7 |             4508.8 |        1301.5 |            1253.3 |                      139.7 |
| macro/string-10kb   |       304.9 |       2249.7 |            36606.5 |         712.1 |             471.2 |                       51.6 |
| macro/eligibility   |       257.0 |      28870.8 |             8718.3 |        6120.1 |            5096.6 |                     1050.3 |
| macro/checkout-40   |      3340.2 |    2238050.3 |            45367.0 |      542880.0 |           92567.1 |                    27836.4 |
| arithmetic mean     |     10943.1 |     993296.6 |           368315.8 |      308019.5 |           58228.9 |                    14457.9 |
| geometric mean      |       959.5 |      95841.1 |            44277.5 |       15370.0 |            6751.5 |                     1397.2 |

* partial coverage — subject errored on some cases in this suite.

=== Pairwise shared-suite ratios ===

Geomean of per-suite ns/op ratios, computed only over suites where both
subjects have finite cells. The per-column mean rows above cover different
suite subsets when a subject errors; these ratios never mix subsets.

  jsonlogic-rs                   99.9x slower than dlrs:engine                over  7 shared suites
  dlrs:wasm:compiled             46.1x slower than dlrs:engine                over  7 shared suites
  json-logic-js                  16.0x slower than dlrs:engine                over  7 shared suites
  json-logic-engine               7.0x slower than dlrs:engine                over  7 shared suites
  json-logic-engine:compiled      1.5x slower than dlrs:engine                over  7 shared suites
  jsonlogic-rs                    2.2x slower than dlrs:wasm:compiled         over  7 shared suites
  jsonlogic-rs                    6.2x slower than json-logic-js              over  7 shared suites
  jsonlogic-rs                   14.2x slower than json-logic-engine          over  7 shared suites
  jsonlogic-rs                   68.6x slower than json-logic-engine:compiled over  7 shared suites
  dlrs:wasm:compiled              2.9x slower than json-logic-js              over  7 shared suites
  dlrs:wasm:compiled              6.6x slower than json-logic-engine          over  7 shared suites
  dlrs:wasm:compiled             31.7x slower than json-logic-engine:compiled over  7 shared suites
  json-logic-js                   2.3x slower than json-logic-engine          over  7 shared suites
  json-logic-js                  11.0x slower than json-logic-engine:compiled over  7 shared suites
  json-logic-engine               4.8x slower than json-logic-engine:compiled over  7 shared suites
```

Reading the macro matrix honestly:

- `dlrs:engine` matches its self-benchmark macro numbers within a few
  percent (e.g. `macro/array-10k` at about 67 microseconds per op,
  roughly 7 ns per element), so the two tiers cross-validate.
- **`macro/checkout-40` is the fair real-world row**: a large rule
  built only from spec-compatible operators, byte-identical results
  verified across engines, and full coverage in every column. On it,
  `dlrs:engine` (3.3 µs) leads `json-logic-engine:compiled` (27.8 to
  41.3 µs across captures — its macro cells jitter with V8
  inline-cache state) by 8.3x or better, with every other subject an
  order of magnitude behind. Earlier captures had this row as a
  statistical tie (28.8 vs 27.7 µs); the gap opened in two waves. First
  the per-row iteration overhead was attacked directly: a var⊗var
  arithmetic map fast path (the `{"*": [unit_price, qty]}` line-total
  shape), compile-time-detected predicate trees for
  `and`/`or`/`!`/`in`/truthy-var filter and quantifier bodies, and a
  remembered-index field lookup for homogeneous rows (the arena analog
  of V8's monomorphic inline caches) — taking the row to 9.5 µs. Then
  the 5.1.0 compile passes attacked the rule's *shape*: the checkout
  rule recomputes its subtotal map+reduce in 8 places, so
  common-subexpression elimination memoizes the repeated pure aggregate
  once per evaluation, and reduce(map(...)) fusion folds the surviving
  pipeline without materializing the intermediate array — another 2.8x,
  to 3.3 µs.
- **Partial cells skip real work.** None of the JS subjects implement
  the non-spec `sort` operator, so their `*` cells on the two array
  suites replace the most expensive case with a cheap throw (~1.6 µs)
  while `dlrs:engine` actually sorts (a numeric-key
  `sort_unstable` fast path keeps that case cheap now). Even with the
  sort case *included*, dlrs's array-10k cell (67.2 µs) sits at
  parity with jle:compiled's sort-free cell (65.0 µs), and its
  array-1k cell (5.3 µs) beats the corresponding sort-free 7.0 µs
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
  1.5x slower than `dlrs:engine`. The honest label for per-row
  iteration went from "tied with the best JS engine" to "ahead of the
  best JS engine on every row shape except large-string slicing".
- `dlrs:wasm:compiled` pays the V8-to-WASM string marshalling per call,
  and that cost scales with payload size: ~37 microseconds per op on
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

The current numbers include the 2026-07 optimization passes:
whole-tree common-subexpression elimination for repeated pure
aggregates (structurally identical pure subtrees share one memoized
evaluation per rule execution), reduce(map(...)) fusion (the fold runs
directly over the map's input with no intermediate array), hinted
`FieldCursor` single-key lookups in the reduce fold and strict-eq
filter, datavalue 0.2.3's buffered heap-free number emit, zero-copy
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
