# Tests

Three kinds of test live here:

- **Rust integration tests** (`*.rs` in this directory): one file per
  area (compile, evaluate, sessions, custom operators, config, errors,
  tracing, threading, ...).
- **JSONLogic compatibility suite** (`suites/`): a data-driven battery
  that `test_jsonlogic.rs` runs on every evaluation path.
- **Generated checks** that run every suite case, or generated rules,
  through a second path and require the same outcome (see
  [Generated checks](#generated-checks)).

## Running

All commands assume you're at the repo root. Most integration tests need
`serde_json` (they use `serde_json::json!`), several need more, and the
JSONLogic suite runner needs `templating` too. Use `--all-features` to
run everything.

Each gated file declares its features twice: a `#![cfg(...)]` header and
a matching `[[test]] required-features` entry in `../Cargo.toml`. Without
the features, cargo skips the target rather than reporting an empty
binary as "0 passed", and `--test <name>` fails with the features to
pass. Adding a gated test file means adding its entry;
`scripts/check-test-features.sh` (run in CI) fails until the two agree.

```bash
# Everything (recommended)
cargo test -p datalogic-rs --all-features

# Single Rust file (most files require at least --features serde_json)
cargo test -p datalogic-rs --all-features --test basic_test

# Only the JSONLogic suite (reads suites/index.json; needs templating + serde_json)
cargo test -p datalogic-rs --all-features --test test_jsonlogic

# A specific JSON suite, with output. Path is relative to crates/datalogic-rs
# because that's the test binary's cwd.
JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
  cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
```

## Suite format

Each file in `suites/` is a JSON array of test-case objects. The runner
prints strings inside the array as section headers and does not run
them:

```json
[
  "# Addition",
  {
    "description": "Addition with variables",
    "rule": { "+": [{ "var": "x" }, { "var": "y" }] },
    "data": { "x": 1, "y": 2 },
    "result": 3
  },
  {
    "description": "Error case: NaN from string",
    "rule": { "+": ["text", 1] },
    "data": null,
    "error": { "type": "NaN" }
  }
]
```

Test case fields:

| Field                | Required | Notes                                                                |
|----------------------|----------|----------------------------------------------------------------------|
| `description`        | yes      | Human-readable test name.                                            |
| `rule`               | yes      | JSONLogic expression to evaluate.                                    |
| `data`               | no       | Input data. Defaults to `{}`.                                        |
| `result`             | one of   | Expected output value. Mutually exclusive with `error`.              |
| `error`              | one of   | Expected error, compared as JSON. A thrown value compares as itself (the engine raises `NaN` as the thrown value `{"type": "NaN"}`); `InvalidArguments` compares as `{"type": "<message>"}` and an unknown operator as `{"type": "Unknown Operator"}`. Other error kinds cannot be expected here. |
| `templating`         | no       | When `true`, evaluate in templating mode (unknown keys preserved).   |
| `template_key_escape`| no       | One character. Evaluate with that template-key escape prefix (see `with_template_key_escape`). Combines with `templating`. |
| `requires`           | no       | Array of cargo feature names the case needs beyond its operators, e.g. `["datetime"]` for a rule whose *data* only carries meaning under a feature. Skipped when absent. |
| `decimal`            | no       | Carried over from the upstream JSONLogic suite, which marks cases whose result depends on decimal parsing. The Rust runner ignores it. |

The runner builds one engine per distinct
`(templating, template_key_escape, constant folding)` key on first use,
so a suite can mix flavours freely, including setting
`template_key_escape` with `templating` absent, to pin that the escape is
inert outside templating mode. The folding part of the key is not a case
field: the evaluation modes below choose it.

### Evaluation modes

Every case runs through every evaluation mode, and all of them must
produce the same outcome before the `result` / `error` expectation is
checked against the first. A disagreement fails the case as a "path
split", whatever it expected: the same rule must mean one thing on every
path.

| Mode             | Path                                                                        |
|------------------|-----------------------------------------------------------------------------|
| `default`        | `Engine::compile`, then a session, with the data as a `serde_json::Value`.  |
| `no-fold`        | The same on an engine built with `with_constant_folding(false)`.            |
| `traced`         | `Engine::trace()`, which compiles from source with the optimizer off. Needs `trace`. |
| `owned-data`     | The default compile, with the data as an `&OwnedDataValue`.                 |
| `json-text`      | The default compile, with the data as JSON text.                            |
| `one-shot-owned` | `Engine::eval_into` with an `&OwnedDataValue` (the `OwnedInput` path).      |
| `traced-owned`   | `Engine::trace()` with an `&OwnedDataValue`. Needs `trace`.                 |

Seven modes under `--all-features`; a build without `trace` runs the
five others. Errors compare by the JSON shape `error` expectations use,
so a rule that fails at compile time on one path and at evaluation on
another still agrees.

`suites/index.json` lists every file the harness should run, in run order
(the bindings' conformance runners and the benchmark read it too). The
Rust readers of the suites (this runner, `oracle_test.rs` and the
benchmark in `tools/benchmark`) share one loader, `common/suite.rs`: it
reads the index, splits a suite into its entries and validates a case's
flavour fields, and panics with the file's path on a suite it cannot
read or parse.
`suite_index_lists_every_suite` fails when a file on disk is missing from
the index (or the index names a file that is gone); rerun it with
`UPDATE_SUITE_INDEX=1` to append new files and drop deleted ones, keeping
the existing order. `every_operator_has_suite_cases` fails when an operator
in the operator table is called by no rule in any suite, and
`every_foldable_row_is_folded_by_a_suite_case` (a unit test in
`operators/table_tests.rs`) when a foldable operator has no case with
literal arguments, or folding one changes its result.

## Generated checks

These tests generate their cases (from the suites, from proptest, or as
a cross product of rule shapes and datasets) and require a second path to
agree. A new suite case is covered by the suite-driven ones without an
edit:

| Test file | What must hold |
|---|---|
| `oracle_test.rs` | The engine agrees with the reference oracle (below). |
| `facts_test.rs` | Evaluating against data pruned to the paths `Logic::facts()` reports gives the same outcome as the full data. |
| `projection_test.rs` | Evaluating with input projection gives what evaluating the whole input gives, on default, unfolded and `MissingVar::Error` engines. |
| `roots_test.rs` | Splitting a case's data into `Roots` leaves the outcome unchanged. |
| `compile_mode_test.rs` | `compile_template` / `compile_strict` give the same compiled rule and outcome as an engine built in that mode. |
| `check_test.rs` | No suite rule that expects a value draws an error diagnostic; on generated rules, `compile` fails exactly when `check` reports a compile-level problem. |
| `trace_pointers_test.rs` | Every suite rule compiled for tracing places each node at a JSON Pointer that resolves in the rule as written. |
| `to_json_round_trip_test.rs` | On generated rules, `compile(to_json(compile(r)))` evaluates as `r` does. |
| `fast_predicate_test.rs`, `fast_arith_test.rs` | The `filter` / `all` / `some` / `none` predicate fast paths and the `map` / `reduce` arithmetic fast paths agree with a traced evaluation, which skips them. |
| `property_test.rs` | On bounded arbitrary rules and data, nothing panics and the optimized pipeline agrees with the traced one (`optimized_and_traced_agree`), plus CSE, fused `reduce(map(..))` and template-escape properties. |

The operator table has its own generated guardrails: `operator_names_test.rs`
(`builtin_operator_names()` against each family's feature;
`catalogue_gates_match_cargo_features` keeps the table's feature gates
and the `all-operators` list in step with `Cargo.toml`), `operators_json_test.rs` (the `docs/src/operators/operators.json`
snapshot and the overview's feature table; regenerate both with
`UPDATE_OPERATORS_JSON=1`), `budget_audit_test.rs` (every costed operator
charges more for a larger input) and `operator_table_rule_test.rs` (an
`OpCode::` reference outside the table needs a listed reason).

## Reference oracle

`oracle/mod.rs` is a second, unoptimised reading of the rules:
it walks the rule JSON with its own frame stack and implements every
operator that decides which expression runs or what data it sees (`var` /
`val` / `exists` / `missing`, the control-flow operators, every iterator,
`try` / `throw`, templates). Operators that only compute a value from their
arguments are handed to an engine built without constant folding, with
each argument expression replaced by a slot that calls back into the
oracle, so no argument the operator sees has been through the optimizer.

`oracle_test.rs` checks the engine against it:

- `every_suite_case_agrees_with_the_oracle`: every suite case, on the
  default engine and on one without constant folding.
- `every_suite_case_agrees_with_the_oracle_when_a_miss_is_an_error`: the
  same with `MissingVar::Error`.
- `generated_rules_agree_with_the_oracle` and
  `generated_templates_agree_with_the_oracle`: proptest rules biased toward
  the shapes the optimizer rewrites (field comparisons in `filter`, `map`
  and `reduce` arithmetic, `reduce` over `map`, `sort` on a field, repeated
  and literal subtrees), over data with missing, null and mistyped fields.
  512 cases each by default; `PROPTEST_CASES` raises it.
- `delegation_is_sound`: an operator the oracle does not implement must not
  push a frame, read the current data or catch errors.
- `explore_disagreements` (ignored): prints every disagreement in a large
  deterministic sample instead of stopping at the first.

```bash
cargo test -p datalogic-rs --all-features --test oracle_test
ORACLE_EXPLORE=100000 cargo test -p datalogic-rs --all-features \
  --test oracle_test explore -- --ignored --nocapture
```

A disagreement is a bug in the engine or in the oracle. Fix it, and pin
the rule as a suite case.

## Reduced-feature builds

The index is feature-agnostic (it lists every suite), but a build without,
say, `ext-control` cannot evaluate `switch`. The runner therefore skips a case
when it invokes an operator this build did not compile in, and reports the
skip count so a run shows its real coverage:

```
TOTAL RESULTS: <passed> passed, 0 failed, <skipped> skipped
(<skipped> cases need operators this build did not compile in)
```

Detection walks the rule for single-key objects (how an operator call is
spelled) and matches them against the operator table's catalogue
(`datalogic_rs::__private::CATALOGUE`), which lists every operator family in
the source with whether this build compiled it in. Because it comes from the
same table as dispatch, it cannot drift. A misspelled operator is in no
catalogue entry, so unknown-operator cases still assert rather than being
skipped.

`requires` covers the residual case where a feature changes value semantics
rather than adding an operator. Under `--all-features` nothing is skipped, so
CI's coverage is unchanged. CI's `feature-combos` job runs the suite on
reduced builds, pairing `serde_json`, `templating` and `trace` with one
family or a few at a time.
