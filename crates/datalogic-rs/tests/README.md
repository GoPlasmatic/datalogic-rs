# Tests

Two layers:

- **Rust unit tests** (`*.rs` in this directory) — exercise specific
  modules (compile, evaluate, arena, custom operators, threading, etc.).
- **JSONLogic compatibility suite** (`suites/`) — large data-driven
  battery driven by `test_jsonlogic.rs`.

## Running

All commands assume you're at the repo root. Most integration tests are
gated behind `feature = "serde_json"` (they use `serde_json::json!`);
the JSONLogic suite runner additionally needs `feature = "templating"`.
Use `--all-features` to run everything.

```bash
# Everything (recommended)
cargo test -p datalogic-rs --all-features

# Single Rust file (most files require at least --features serde_json)
cargo test -p datalogic-rs --all-features --test basic_test

# Just the JSONLogic suite (reads suites/index.json — needs templating + serde_json)
cargo test -p datalogic-rs --all-features --test test_jsonlogic

# A specific JSON suite, with output. Path is relative to crates/datalogic-rs
# because that's the test binary's cwd.
JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
  cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
```

## Suite format

Each file in `suites/` is a JSON array of test-case objects. Strings
inside the array are skipped — used as section headers in the test
output:

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
    "description": "Error case — NaN from string",
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

The runner builds one engine per distinct `(templating, template_key_escape)`
pair on first use, so a suite can mix flavours freely — including setting
`template_key_escape` with `templating` absent, to pin that the escape is
inert outside templating mode.

`suites/index.json` lists every file the harness should run; new
suites must be added there.

## Reduced-feature builds

The index is feature-agnostic — it lists every suite — but a build without,
say, `ext-control` cannot evaluate `switch`. The runner therefore skips a case
when it invokes an operator this build did not compile in, reporting the count
so a run stays honest about its coverage:

```
TOTAL RESULTS: 1142 passed, 0 failed, 572 skipped
```

Detection walks the rule for single-key objects — how an operator call is
spelled — and matches them against `GATED_OPERATORS` in the runner. That table
is checked against `Engine::builtin_operator_names()` by
`gated_operator_table_matches_engine`, so it cannot silently drift. A
misspelled operator is deliberately *not* in the table, so unknown-operator
cases still assert rather than being skipped.

`requires` covers the residual case where a feature changes value semantics
rather than adding an operator. Under `--all-features` nothing is skipped, so
CI's coverage is unchanged.
