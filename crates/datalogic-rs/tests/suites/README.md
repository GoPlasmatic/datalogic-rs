# JSONLogic test suites

Each `*.json` file in this tree is a list of test cases consumed by
`tests/test_jsonlogic.rs`. The test runner discovers files via
`index.json` (run `JSONLOGIC_TEST_FILE=…` to scope to a single suite).

## Categories

| Naming | What it covers |
|---|---|
| `compatible.json` | The shared JSONLogic baseline — every conforming engine should pass these. The reference cases come from <https://jsonlogic.com/tests.json>. |
| `*.extra.json` (e.g. `try.extra.json`, `val.extra.json`, `iterators.extra.json`) | v5-only extensions to the baseline operator (extra error cases, extra arg shapes, etc.). Other JSONLogic engines won't run these. |
| `structured-objects.json` | Cases for templating mode (object templating). Gated on `feature = "templating"` in the test runner. |
| `unknown-operators.json` | Behaviour when a rule uses an operator name the engine doesn't know. |
| `additional.json` / `chained.json` / `coalesce.json` / `truthiness.json` / `scopes.json` / `empty-objects.json` / `type.json` | Catch-alls for cross-cutting behaviour that doesn't belong to one operator. |
| `val.json` / `val-compat.json` / `val.extra.json` / `exists.json` | The `val` / `var` / `exists` family — path-resolution semantics, scope walking, reduce shortcuts. |
| `length.json` / `slice.json` / `sort.json` | Array helpers (`length`, `slice`, `sort`). |
| `throw.json` / `try.json` / `try.extra.json` | The `throw` / `try` error-handling pair (gated on `feature = "error-handling"`). |
| Subdirectories (`arithmetic/`, `array/`, `comparison/`, `control/`, `datetime/`, `string/`, `custom/`) | One file per operator within the category. Per-operator suites exercise edge cases (NaN, divbyzero, type coercion) that the baseline doesn't cover. |

## Test case shape

```json
[
  "# Optional section header (strings get skipped)",
  {
    "description": "Addition with variables",
    "rule":   { "+": [ { "var": "x" }, { "var": "y" } ] },
    "data":   { "x": 1, "y": 2 },
    "result": 3
  },
  {
    "description": "Error case — NaN from string",
    "rule":   { "+": [ "text", 1 ] },
    "data":   null,
    "error":  { "type": "NaN" }
  }
]
```

Required fields:

- `description` — test name surfaced in the runner output.
- `rule` — the JSONLogic expression to evaluate.
- `data` — input data (`null` or object).
- One of `result` (expected output) or `error` (expected error object) — they are mutually exclusive.

Optional fields:

- `templating` — set to `true` for cases that need templating mode (the runner enables it on the engine for that case only).

## Running

```bash
# Whole suite (driven by index.json):
cargo test -p datalogic-rs --all-features --test test_jsonlogic

# One suite — path is relative to crates/datalogic-rs/ (the test binary's cwd):
JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
    cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
```

## Adding a new suite

1. Create `tests/suites/<path>.json` with the test cases.
2. Add the relative path to `tests/suites/index.json` so the discovery loop picks it up.
3. Run the suite locally with `JSONLOGIC_TEST_FILE=…` to confirm pass/fail counts before committing.
