# JSONLogic test suites

Each `*.json` file in this tree is a list of test cases consumed by
`tests/test_jsonlogic.rs`. The runner reads the files listed in
`index.json`, in that order; `JSONLOGIC_TEST_FILE=…` scopes a run to one
suite. The bindings' conformance runners, the benchmark and several
generated tests read the same files (see [`../README.md`](../README.md)).

## Categories

| Naming | What it covers |
|---|---|
| `compatible.json` | The shared JSONLogic baseline: every conforming engine should pass these. The reference cases come from <https://jsonlogic.com/tests.json>. |
| `*.extra.json` (e.g. `try.extra.json`, `val.extra.json`, `iterators.extra.json`) | Extensions to a baseline operator (extra error cases, extra argument shapes). Other JSONLogic engines won't run these. |
| `structured-objects.json` | Cases for templating mode (object templating); each case carries `templating: true` so the runner switches the engine into that mode for it. |
| `template-key-escape.json` | The template-key escape prefix: strip-one semantics, nesting, a configurable escape character, and that the escape is inert outside templating mode. |
| `unknown-operators.json` | Behaviour when a rule uses an operator name the engine doesn't know. |
| `additional.json` / `chained.json` / `coalesce.json` / `truthiness.json` / `empty-objects.json` / `type.json` | Catch-alls for cross-cutting behaviour that doesn't belong to one operator. |
| `literal-arguments.json` | Literal versus computed arguments (`+` / `*` over a single array, `var` / `val` paths, `sort` sources, `switch` case tables, `slice` bounds, `missing_some` minimums), pinning that constant folding cannot change what a rule means. |
| `val.json` / `val-compat.json` / `val.extra.json` / `exists.json` / `var-computed-path.json` | The `val` / `var` / `exists` family: path-resolution semantics, computed paths and their defaults, reduce shortcuts. |
| `scopes.json` / `scopes-nested.json` | Level markers and scope walking; `scopes-nested.json` is a grid of every level at every iterator depth, cross-checked against json-logic-engine. |
| `length.json` / `slice.json` / `sort.json` | Array helpers (`length`, `slice`, `sort`). |
| `throw.json` / `try.json` / `try.extra.json` | The `throw` / `try` error-handling pair (`error-handling` feature). |
| `group_by.json` / `distinct.json` | The `ext-array` collection operators: keyed grouping, value and keyed dedup, iteration-scope isolation. |
| `object-ops.json` | The `keys` / `values` / `entries` family (`ext-object` feature). |
| `cse.json` | Rules that repeat pure subtrees; pins that common-subexpression elimination is invisible (same values, error flow, and context isolation with the pass on or off). |
| Subdirectories (`arithmetic/`, `array/`, `comparison/`, `control/`, `datetime/`, `flagd/`, `string/`, `tensor/`) | Grouped by category, most files covering one operator. They exercise edge cases (NaN, division by zero, type coercion) the baseline doesn't cover. `datetime/timezone.json` covers the IANA-zone arguments on `format_date` / `parse_date`; `flagd/` mirrors the upstream OpenFeature flagd test files; `tensor/` covers the `tensor` feature's constructors, shape operators and readers. |

## Feature gating

`tests/test_jsonlogic.rs` declares `required-features = ["templating",
"serde_json"]`, so cargo skips it without both. On a build that leaves an
operator family out, the runner skips each case that calls one of its
operators (or names a feature in its `requires` field) and reports the
skip count; under `--all-features` nothing is skipped. See
[Reduced-feature builds](../README.md#reduced-feature-builds).

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
    "description": "Error case: NaN from string",
    "rule":   { "+": [ "text", 1 ] },
    "data":   null,
    "error":  { "type": "NaN" }
  }
]
```

Required fields:

- `description`: test name surfaced in the runner output.
- `rule`: the JSONLogic expression to evaluate.
- One of `result` (expected output) or `error` (expected error object); they are mutually exclusive.

Optional fields:

- `data`: input data; `{}` when absent.
- `templating`: set to `true` for cases that need templating mode (the runner enables it on the engine for that case only).
- `template_key_escape`: one character, the template-key escape prefix for that case.
- `requires`: cargo features the case needs beyond its operators.

The full field reference is in [`../README.md`](../README.md#suite-format).

## Running

```bash
# Whole suite (driven by index.json):
cargo test -p datalogic-rs --all-features --test test_jsonlogic

# One suite; path is relative to crates/datalogic-rs/ (the test binary's cwd):
JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
    cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
```

## Adding a new suite

1. Create `tests/suites/<path>.json` with the test cases.
2. Add it to `tests/suites/index.json`, which fixes run order:
   `UPDATE_SUITE_INDEX=1 cargo test -p datalogic-rs --all-features --test test_jsonlogic suite_index`
   appends every new file on disk.
3. Run the suite locally with `JSONLOGIC_TEST_FILE=…` to confirm pass/fail counts before committing.
