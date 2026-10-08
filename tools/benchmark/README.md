# datalogic-bench

Dev-only benchmark harness for `datalogic-rs`. **For the latest captured
matrix and headline numbers, see [`BENCHMARK.md`](./BENCHMARK.md)**. Link
to that file from other docs rather than re-quoting cells inline.

The crate has five binaries over one library (`src/lib.rs`). `self` and
`compare` share its suite loader and reporter, `boundary_core` and
`projection` share its timer (`measure`: warmup, pilot, median of 5), and
`self`, `compare` and `profile_macro` share the synthesized macro suites
(`src/macro_suites.rs`). The suite loader is the core tests' own
(`crates/datalogic-rs/tests/common/suite.rs`, included by path): a case
that sets `templating` or `template_key_escape` compiles on an engine
built that way, and a suite that fails to parse stops the run instead of
dropping out of the geomean.

| Binary          | Purpose                                                                                |
|-----------------|----------------------------------------------------------------------------------------|
| `self`          | Times datalogic-rs alone on the fast arena path (compile once, persistent input arena, eval-arena reset). Use it to track regressions in the engine. |
| `compare`       | Cross-library **matrix**: runs every suite against every available subject (native datalogic-rs, gated Rust crates, the WASM build and JS libraries via Node) and prints a markdown table of avg ns/op. |
| `boundary_core` | The rust-core runner for the per-binding boundary benchmark under [`boundary/`](./boundary); emits the same JSON-lines schema as the other runtimes' runners. |
| `profile_macro` | Sampling-profiler feeder (samply / Instruments): hammers one macro suite in a hot loop so the profile shows only that suite's evaluation path. |
| `projection`    | Read projection (5.8.0): what one evaluation costs when a rule reads a few fields of a large owned or `serde_json` context, with and without projection. |

A separate area, [`boundary/`](./boundary), measures the opposite of the
matrix: **per-binding boundary cost**, what a real caller pays per
evaluation through each language binding (C ABI, Node, Python, WASM,
Go, JVM, .NET, PHP) on the three workloads from
[`BINDINGS-OVERHEAD.md`](./BINDINGS-OVERHEAD.md). One runner per
runtime, one shared discipline (warmup, ~250 ms samples, median of 5),
JSON-lines output, and a renderer for that document's tables:

```bash
cd tools/benchmark/boundary && ./run.sh && python3 render.py
```

`./run.sh` runs the five runtimes that need no extra toolchain
(rust-core, c-abi, node, python, wasm); `./run.sh all` adds go, dotnet,
jvm and php. The `boundary_core` binary in this crate is the rust-core
runner. See [`boundary/README.md`](./boundary/README.md) for each
runner's prerequisites.

`self` and `compare` read JSON suites from
`crates/datalogic-rs/tests/suites/`, and both accept `--macro` to swap
those for the synthesized macro suites. They write JSON reports to
`tools/benchmark/output/` (gitignored): `report-self-*.json` for `--all`
runs of `self`, `report-compare-*.json` for `compare`,
`report-compare-macro-*.json` for `compare --macro`.

## `self`: regression baseline

```bash
# Single suite (compatible.json by default)
cargo run --release -p datalogic-bench --bin self

# All suites
cargo run --release -p datalogic-bench --bin self -- --all

# Specific suite
cargo run --release -p datalogic-bench --bin self -- arithmetic/plus.json

# Macro tier: synthesized large-payload suites (1k/10k arrays, 128-key
# objects, 48-level nesting, 10 KB strings, one eligibility rule)
cargo run --release -p datalogic-bench --bin self -- --macro
```

Each suite is timed three ways with the same discipline (median of 3
reps, `black_box`, session reset per iteration, pre-sized arena): the
whole suite (the headline, comparable with older reports), only the
rules the compiler constant-folded to a literal (`Logic::is_constant`),
and the rest. The per-suite line shows the split
(`folded 23/32 @ 2.94 ns, rest @ 75.75 ns`) and the summary reports
overall / folded-only / non-folded-only geomeans, so constant-folded
rules can't flatter the data-dependent number. The macro tier scales
its per-suite iteration count from a pilot pass so one timed rep lands
near 250 ms; see [`BENCHMARK.md`](./BENCHMARK.md#macro-tier) for the
suite list.

## `compare`: cross-library matrix

The matrix has one row per suite and one column per subject. Cells are
the median ns/op of three timed samples, each sized to hit a ~200ms wall
budget. Two aggregation rows at the bottom show the arithmetic mean
(familiar) and geometric mean (the one to use for cross-library
comparison, since one slow suite doesn't dominate it).

### Subjects

The matrix shows one column per **library / API tier that takes a
precompile-once approach**, so cells compare like with like. The matrix
leaves out convenience-API tiers (`Engine::eval_str`,
`Session::eval_borrowed`, the WASM free function
`evaluate(ruleStr, dataStr, false)`, deprecated in 5.8.0) because their
numbers measure API-shape costs (parse cost, session reset cost, WASM
string marshalling) rather than engine cost. For datalogic-rs's own API
tiers, `self` times `Session::eval_borrowed`, and `boundary_core` times
parse-per-call, serialize and `serde_json::Value` tiers (section 1 of
[`BINDINGS-OVERHEAD.md`](./BINDINGS-OVERHEAD.md)).

Always compiled in:

| Column        | What it exercises                                                                                       |
|---------------|---------------------------------------------------------------------------------------------------------|
| `dlrs:engine` | Pre-compiled `Logic` + caller-owned `Bump`, batch-style reset between iterations. Each case compiles on an engine with the templating and key-escape settings its suite asks for. The native baseline. |

Behind a Cargo feature:

| Column         | Feature flag           | Crate                              |
|----------------|------------------------|------------------------------------|
| `jsonlogic-rs` | `subject-jsonlogic-rs` | [bestowinc/json-logic-rs] 0.5: `apply(&Value, &Value)`, no compile API |

[bestowinc/json-logic-rs]: https://crates.io/crates/jsonlogic-rs

Auto-detected at runtime (require Node + an `npm install` in `runners/`):

| Column                       | API exercised                                                              |
|------------------------------|----------------------------------------------------------------------------|
| `dlrs:wasm:compiled`         | `@goplasmatic/datalogic-wasm` `new CompiledRule(ruleStr, templating)` once per rule (the case's `templating` flag), then `.evaluate(dataStr)` per call. WASM analog of `dlrs:engine`; the remaining per-call cost is data marshall + parse + result stringify across the V8↔WASM boundary. `CompiledRule` is deprecated in 5.8.0 and removed in 6.0; its replacement, `new Engine(options).compile(ruleStr)`, returns a `Rule` with the same `.evaluate(dataStr)` call. |
| `json-logic-js`              | `json-logic-js` (jwadhams): `apply(rule, data)`, interpreted, no compile API. |
| `json-logic-engine`          | `json-logic-engine` (TotalTechGeek): interpreted (`engine.run(rule, data)`). |
| `json-logic-engine:compiled` | `json-logic-engine`: pre-compiled (`engine.build(rule)`, "12.5–20× hot path" per the library's README). |

`json-logic-engine` and `json-logic-engine:compiled` share their npm
package but exercise different APIs (interpreter vs build-then-call).

### One-time setup for Node subjects

From the repo root:

```bash
# Build the WASM package the dlrs:wasm column loads:
(cd bindings/wasm && ./build.sh)

# Install the runner deps (json-logic-js, json-logic-engine, and a
# file: link to the wasm pkg):
(cd tools/benchmark/runners && npm install)
```

If `node` isn't on PATH or a subject's package is missing from
`runners/node_modules/`, `compare` exits with an error listing the
unavailable subjects, because a matrix that looks complete with empty
columns misleads more than a failed run. Pass
`--allow-missing-subjects` to render the matrix without those columns.

### Run

```bash
# Single suite (compatible.json by default)
cargo run --release -p datalogic-bench --bin compare

# Specific suite
cargo run --release -p datalogic-bench --bin compare -- arithmetic/plus.json

# Every suite from tests/suites/index.json
cargo run --release -p datalogic-bench --bin compare -- --all

# Synthesized macro suites (large payloads) across all subjects; report
# lands in output/report-compare-macro-<timestamp>.json
cargo run --release -p datalogic-bench --bin compare -- --macro

# With the gated Rust competitor
cargo run --release -p datalogic-bench --bin compare \
  --features subject-jsonlogic-rs -- --all

# Allow rendering even when Node subjects aren't installed
cargo run --release -p datalogic-bench --bin compare -- --all --allow-missing-subjects
```

### Reading the output

```
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 50 suites) ===

| Suite                | dlrs:engine | jsonlogic-rs | dlrs:wasm:compiled | json-logic-js | json-logic-engine | json-logic-engine:compiled |
|----------------------|------------:|-------------:|-------------------:|--------------:|------------------:|---------------------------:|
| arithmetic/plus.json |         2.8 |       224.4* |              518.6 |        393.4* |              73.0 |                       22.6 |
...
| arithmetic mean      |         ... |          ... |                ... |           ... |               ... |                        ... |
| geometric mean       |         ... |          ... |                ... |           ... |               ... |                        ... |

* partial coverage — subject errored on some cases in this suite.
```

- Numbers are nanoseconds per evaluation (lower is better).
- `—` = subject unavailable for this run (feature off, runtime missing,
  or precompile failed for the suite).
- `ERR` = subject ran but errored on >50% of cases in the suite.
- A trailing `*` on a number = subject errored on some cases in the suite
  but completed enough that ns/op is still meaningful.
- Compare runs filter out negative-test cases (entries with
  `error: {...}` instead of `result`): engines disagree on what "errors"
  and how expensive their error path is, so including them would
  penalise verbose-error subjects.

After the matrix, the runner prints a pairwise ratio table:

```
=== Pairwise shared-suite ratios ===

  json-logic-engine:compiled      9.4x slower than dlrs:engine  over  3 shared suites
  ...
```

Each line is the geomean of per-suite ns/op ratios computed only over
suites where **both** subjects have finite cells. The per-column mean
rows in the matrix cover different suite subsets when subjects `ERR` on
different suites, so quotients of column geomeans mix incomparable
sets; the pairwise ratios never do. Matrix cells, per-column means, and
these ratios also go to `output/report-compare-<timestamp>.json`
(`output/report-compare-macro-<timestamp>.json` for `--macro` runs).

### Native-CPU build (optional, host-only numbers)

A `.cargo/config.toml` inside `tools/benchmark/` adds
`-C target-cpu=native`. Cargo only picks this up when the cwd is at or below
the benchmark crate, so it's opt-in by location:

```bash
cd tools/benchmark
cargo run --release --bin compare -- --all
```

Numbers from a native build are not portable across machines: keep them as
a relative baseline, not an absolute publishable figure. Builds invoked
from the repo root remain portable.

## `projection`: read projection

5.8.0 evaluates an owned, `serde_json` or `Roots` input by viewing only
the paths a rule reads (see the 5.8.0 Performance notes in
[`CHANGELOG.md`](../../CHANGELOG.md)). This binary measures that against
viewing the whole input:

```bash
cargo run --release -p datalogic-bench --bin projection
```

It builds contexts of about 1 KB, 100 KB and 8 MB, and times three rules
(one field, three fields, a `reduce` over a small array) on each context
as an `OwnedDataValue` and as a `serde_json::Value`. The `projected`
column evaluates on the engine that compiled the rule; `whole` evaluates
the same rule on a second, identically built engine, which views the
whole input because it did not compile the rule. The binary asserts both
give the same result, prints a table with the speedup, and writes no
report.

## `profile_macro`: profiler feeder

`profile_macro <suite-substring> [seconds]` runs the first macro suite
whose name contains the substring (default `checkout`) in a loop for the
given time (default 10 s) and prints a rough ns/op. Point a sampling
profiler at it:

```bash
cargo build --release -p datalogic-bench --bin profile_macro
samply record target/release/profile_macro checkout 10
```

The root release profile keeps line tables (`debug = "line-tables-only"`),
so frames resolve to source lines.

## Adding more subjects

### Native Rust crate

1. Add an optional dep + a Cargo feature in `tools/benchmark/Cargo.toml`:
   ```toml
   [dependencies]
   my-jsonlogic = { version = "X.Y", optional = true }

   [features]
   subject-my-jsonlogic = ["dep:my-jsonlogic"]
   ```
2. Add a `Subject` impl inside `bin/compare.rs`, gated by
   `#[cfg(feature = "subject-my-jsonlogic")]`. Mirror the pattern of
   `JsonLogicRs`: pre-parse rule and data once, time `apply()` only.
3. Push the subject into `build_subjects()` (also gated).
4. Run with `--features subject-my-jsonlogic`.

### JS / WASM library (via Node subprocess)

1. `cd tools/benchmark/runners && npm install <pkg>`.
2. Add a `LIBS` entry in `runners/node-runner.js`: one async `setup`
   that returns a callable `apply(case)`.
3. In `build_subjects()` inside `bin/compare.rs`, add a
   `("display-name", "<LIBS key>", "<npm-pkg>")` tuple to
   `node_subjects`. The column appears when `node` is on PATH and
   `runners/node_modules/<npm-pkg>` exists.

Neither recipe changes the shared harness in `src/lib.rs`.

## Platform support

Linux and macOS. The Node runner uses POSIX path conventions in
`file:../../../bindings/wasm/pkg` and the `runners/` setup is
shell-coded; the harness is untested on Windows.

## CI

Don't run `compare` in CI. WASM build + npm install + 3+ minutes of
matrix work makes for flaky CI runs. `self` is the regression-tracking
target: keep CI on `cargo test --workspace --all-features` plus a
single-suite `self` invocation if you want a perf signal.
