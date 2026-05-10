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

# A specific JSON suite, with output. Path is relative to packages/core
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

`suites/index.json` lists every file the harness should run; new
suites must be added there.
