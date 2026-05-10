# datalogic-bench

Dev-only benchmark harness for `datalogic-rs`. **For the latest captured
matrix and headline numbers, see [`BENCHMARK.md`](./BENCHMARK.md)** — link
to that file from other docs rather than re-quoting cells inline.

Two binaries share a common suite loader and reporter (`src/lib.rs`):

| Binary    | Purpose                                                                                |
|-----------|----------------------------------------------------------------------------------------|
| `self`    | Times datalogic-rs alone using the fast arena path (compile once, persistent input arena, eval-arena reset). Use this to track regressions in our own engine. |
| `compare` | Cross-library **matrix** — runs every suite against every available subject (datalogic-rs API tiers, gated Rust crates, JS/WASM via Node) and prints a markdown table of avg ns/op. |

Both read JSON suites from `packages/core/tests/suites/`. `self` writes a
JSON report to `packages/benchmark/output/` (gitignored); `compare` only
prints to stdout.

## `self` — regression baseline

```bash
# Single suite (compatible.json by default)
cargo run --release -p datalogic-bench --bin self

# All suites
cargo run --release -p datalogic-bench --bin self -- --all

# Specific suite
cargo run --release -p datalogic-bench --bin self -- arithmetic/plus.json
```

## `compare` — cross-library matrix

The matrix has one row per suite and one column per subject. Cells are
the median ns/op of three timed samples, each sized to hit a ~200ms wall
budget. Two aggregation rows at the bottom show the arithmetic mean
(familiar) and geometric mean (the right average for cross-library
comparison — one slow suite doesn't dominate).

### Subjects

The matrix shows one column per **library / API tier that takes a
precompile-once approach** — apples-to-apples cells. Convenience-API
tiers (`Engine::eval_str`, `Session::eval_borrowed`, raw
`evaluate(ruleStr, dataStr, false)` on the WASM) are intentionally not
in the matrix because their numbers measure API-shape costs (parse cost,
session reset cost, WASM string marshalling) rather than engine cost.
For per-API-tier numbers on datalogic-rs alone, see `bin/self.rs`.

Always compiled in:

| Column        | What it exercises                                                                                       |
|---------------|---------------------------------------------------------------------------------------------------------|
| `dlrs:engine` | Pre-compiled `Logic` + caller-owned `Bump`, batch-style reset between iterations. The native baseline.  |

Behind a Cargo feature:

| Column         | Feature flag           | Crate                              |
|----------------|------------------------|------------------------------------|
| `jsonlogic-rs` | `subject-jsonlogic-rs` | [bestowinc/json-logic-rs] 0.5 — `apply(&Value, &Value)`, no compile API |

[bestowinc/json-logic-rs]: https://crates.io/crates/jsonlogic-rs

Auto-detected at runtime (require Node + an `npm install` in `runners/`):

| Column                       | API exercised                                                              |
|------------------------------|----------------------------------------------------------------------------|
| `dlrs:wasm:compiled`         | `@goplasmatic/datalogic` `new CompiledRule(ruleStr, false)` once per rule, then `.evaluate(dataStr)` per call. WASM analog of `dlrs:engine`; remaining per-call cost is data marshall + parse + result stringify across the V8↔WASM boundary. |
| `json-logic-js`              | `json-logic-js` (jwadhams) — `apply(rule, data)`, interpreted, no compile API. |
| `json-logic-engine`          | `json-logic-engine` (TotalTechGeek) — interpreted (`engine.run(rule, data)`). |
| `json-logic-engine:compiled` | `json-logic-engine` — pre-compiled (`engine.build(rule)`, "12.5–20× hot path" per the library's README). |

`json-logic-engine` and `json-logic-engine:compiled` share their npm
package but exercise different APIs (interpreter vs build-then-call).

### One-time setup for Node subjects

```bash
# Build the WASM that the dlrs:wasm column points at:
cd packages/wasm && ./build.sh

# Install the runner deps (json-logic-js + a file: link to the wasm pkg):
cd packages/benchmark/runners && npm install
```

If `node` isn't on PATH or `runners/node_modules/` is missing, the
matrix runner hard-fails by default (the surprise of "complete-looking
matrix with silently-empty columns" is worse than an explicit error).
Pass `--allow-missing-subjects` to render the matrix without the
unavailable columns.

### Run

```bash
# Single suite (compatible.json by default)
cargo run --release -p datalogic-bench --bin compare

# Specific suite
cargo run --release -p datalogic-bench --bin compare -- arithmetic/plus.json

# Every suite from tests/suites/index.json
cargo run --release -p datalogic-bench --bin compare -- --all

# With the gated Rust competitor
cargo run --release -p datalogic-bench --bin compare \
  --features subject-jsonlogic-rs -- --all

# Allow rendering even when Node subjects aren't installed
cargo run --release -p datalogic-bench --bin compare -- --all --allow-missing-subjects
```

### Reading the output

```
=== Cross-Library Matrix — avg ns/op (median of 3, ~200ms target/cell, 44 suites) ===

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
- Negative-test cases (entries with `error: {...}` instead of `result`) are
  filtered out of compare runs — engines disagree on what "errors"
  and how expensive their error path is, so including them would
  unfairly penalise verbose-error subjects.

### Native-CPU build (optional, host-only numbers)

A `.cargo/config.toml` inside `packages/benchmark/` adds
`-C target-cpu=native`. Cargo only picks this up when the cwd is at or below
the benchmark crate, so it's opt-in by location:

```bash
cd packages/benchmark
cargo run --release --bin compare -- --all
```

Numbers from a native build are not portable across machines — keep them as
a relative baseline, not an absolute publishable figure. Builds invoked
from the repo root remain portable.

## Adding more subjects

### Native Rust crate

1. Add an optional dep + a Cargo feature in `packages/benchmark/Cargo.toml`:
   ```toml
   [dependencies]
   my-jsonlogic = { version = "X.Y", optional = true }

   [features]
   subject-my-jsonlogic = ["dep:my-jsonlogic"]
   ```
2. Add a `Subject` impl inside `bin/compare.rs`, gated by
   `#[cfg(feature = "subject-my-jsonlogic")]`. Mirror the pattern of
   `JsonLogicRs` — pre-parse rule and data once, time `apply()` only.
3. Push the subject into `build_subjects()` (also gated).
4. Run with `--features subject-my-jsonlogic`.

### JS / WASM library (via Node subprocess)

1. `cd packages/benchmark/runners && npm install <pkg>`.
2. Add a `LIBS` entry in `runners/node-runner.js` — one async `setup`
   that returns a callable `apply(case)`.
3. In `build_subjects()` inside `bin/compare.rs`, push a new
   `NodeSubject::new("display-name", "<npm-pkg>")` (gated on
   `node_dep_installed("<npm-pkg>")`).

That's the entire recipe — three files each, no harness changes.

## Platform support

Linux and macOS. The Node runner uses POSIX path conventions in
`file:../../wasm/pkg` and the `runners/` setup is shell-coded; Windows
isn't tested.

## CI

Don't run `compare` in CI. WASM build + npm install + 3+ minutes of
matrix work makes for flaky CI runs. `self` is the regression-tracking
target — keep CI on `cargo test --workspace --all-features` plus a
single-suite `self` invocation if you want a perf signal.
