# Tests

Two layers:

- **Rust unit tests** (`*.rs` in this directory): exercise specific
  modules (compile, evaluate, arena, custom operators, threading, etc.).
- **JSONLogic compatibility suite** (`suites/`): a large data-driven
  battery that `test_jsonlogic.rs` runs.

## Running

All commands assume you're at the repo root. Most integration tests are
gated behind `feature = "serde_json"` (they use `serde_json::json!`);
the JSONLogic suite runner additionally needs `feature = "templating"`.
Use `--all-features` to run everything.

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
skips strings inside the array and uses them as section headers in the
test output:

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
| `data`               | yes      | Input data (object or `null`).                                       |
| `result`             | one of   | Expected output value. Mutually exclusive with `error`.              |
| `error`              | one of   | Expected error object, e.g. `{"type": "NaN"}`.                       |
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

## Reference oracle

`oracle/mod.rs` is a second, deliberately unoptimised reading of the rules:
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
TOTAL RESULTS: 1142 passed, 0 failed, 572 skipped
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
CI's coverage is unchanged.
