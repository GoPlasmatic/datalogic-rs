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

Captured 2026-07-02, Apple M2 Pro (arm64), Rust 1.96.0, Node v22.22.2,
release build from the repo root (no `target-cpu=native`):

```
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 6 suites) ===

| Suite               | dlrs:engine | jsonlogic-rs | dlrs:wasm:compiled | json-logic-js | json-logic-engine | json-logic-engine:compiled |
|---------------------|------------:|-------------:|-------------------:|--------------:|------------------:|---------------------------:|
| macro/array-1k      |      9986.2 |     368156.0 |           222517.3 |     152706.2* |          31717.5* |                    7072.3* |
| macro/array-10k     |    129948.1 |    4225527.8 |          2315503.4 |    1495594.5* |         275151.8* |                   67019.5* |
| macro/object-128key |       126.0 |       5112.4 |             9528.3 |         304.6 |             284.3 |                      110.9 |
| macro/deep-48       |       129.6 |      61067.9 |             4216.8 |        1342.3 |            1324.4 |                      139.1 |
| macro/string-10kb   |      2789.1 |       2160.6 |            42352.6 |         740.1 |             500.1 |                       51.0 |
| macro/eligibility   |       296.2 |      29657.4 |             8728.2 |        6795.5 |            5015.1 |                     1166.5 |
| arithmetic mean     |     23879.2 |     781947.0 |           433807.8 |      276247.2 |           52332.2 |                    12593.2 |
| geometric mean      |      1611.5 |      56084.5 |            44391.8 |        8816.5 |            4494.4 |                      870.3 |

* partial coverage — subject errored on some cases in this suite.

=== Pairwise shared-suite ratios ===

  jsonlogic-rs                   34.8x slower than dlrs:engine                over  6 shared suites
  dlrs:wasm:compiled             27.5x slower than dlrs:engine                over  6 shared suites
  json-logic-js                   5.5x slower than dlrs:engine                over  6 shared suites
  json-logic-engine               2.8x slower than dlrs:engine                over  6 shared suites
  dlrs:engine                     1.9x slower than json-logic-engine:compiled over  6 shared suites
  jsonlogic-rs                    1.3x slower than dlrs:wasm:compiled         over  6 shared suites
  jsonlogic-rs                    6.4x slower than json-logic-js              over  6 shared suites
  jsonlogic-rs                   12.5x slower than json-logic-engine          over  6 shared suites
  jsonlogic-rs                   64.4x slower than json-logic-engine:compiled over  6 shared suites
  dlrs:wasm:compiled              5.0x slower than json-logic-js              over  6 shared suites
  dlrs:wasm:compiled              9.9x slower than json-logic-engine          over  6 shared suites
  dlrs:wasm:compiled             51.0x slower than json-logic-engine:compiled over  6 shared suites
  json-logic-js                   2.0x slower than json-logic-engine          over  6 shared suites
  json-logic-js                  10.1x slower than json-logic-engine:compiled over  6 shared suites
  json-logic-engine               5.2x slower than json-logic-engine:compiled over  6 shared suites
```

Reading the macro matrix honestly:

- `dlrs:engine` matches its self-benchmark macro numbers within a few
  percent (e.g. `macro/array-10k` at about 130 microseconds per op,
  roughly 13 ns per element), so the two tiers cross-validate.
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
  `json-logic-engine:compiled` at ~51 ns/op is measuring V8's lazy
  string machinery, not byte copies.
- `dlrs:wasm:compiled` pays the V8-to-WASM string marshalling per call,
  and that cost scales with payload size: ~42 microseconds per op on
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
