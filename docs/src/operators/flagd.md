# flagd-Compat Operators

Two operators specified by the [OpenFeature flagd in-process provider](https://flagd.dev/reference/custom-operations/) for feature-flag targeting. Implemented to match the canonical [Go evaluator](https://github.com/open-feature/flagd/tree/main/core/pkg/evaluator) byte-for-byte, so a flag definition that works under any flagd provider will produce identical variants here.

**Cargo feature:** `flagd`. Off by default — opt in via:

```toml
datalogic-rs = { version = "5", features = ["flagd"] }
```

Both operators return `null` on malformed input (wrong arg count, unparseable version, missing targeting context, etc.) rather than raising. flagd's evaluator observes the `null` and falls back to the flag's default variant; non-flagd callers can compose with `??` or `if` for the same effect.

## fractional

Deterministic percentage bucketing for A/B tests and gradual rollouts. Buckets are sticky per bucketing key — the same input always lands in the same variant across runs.

**Reference:** [flagd Fractional spec](https://flagd.dev/reference/custom-operations/fractional-operation/)

**Algorithm.** MurmurHash3 x86-32 of the bucketing key, then `bucket = (hash * total_weight) >> 32` and walk cumulative integer weight bands. Identical to the Go evaluator's `core/pkg/evaluator/fractional.go`. The hash is vendored inline (~30 LOC) for portability across every target.

**Two argument shapes:**

### 1. Explicit bucketing key

The first argument evaluates to a string; the remaining args are `[variant, weight]` pairs.

```json
{
  "fractional": [
    { "cat": [{ "var": "$flagd.flagKey" }, { "var": "email" }] },
    ["red",   50],
    ["blue",  20],
    ["green", 30]
  ]
}
```

The canonical pattern concatenates `$flagd.flagKey + email` so the same email gets different variants on different flags — users aren't always in the same cohort across your whole product.

### 2. Implicit bucketing key

Omit the first argument (or pass anything that doesn't evaluate to a string). The bucketing key is built from the root context as `flagKey + targetingKey` (the order the flagd Go evaluator uses):

```json
{
  "fractional": [
    ["new-ui", 50],
    ["old-ui", 50]
  ]
}
```

The evaluation data needs to carry both pieces. flagd in-process providers stamp them onto the context as:

```json
{
  "targetingKey": "alice@example.com",
  "$flagd": { "flagKey": "header-color" }
}
```

**Missing or empty `targetingKey`** in implicit form returns `null` — there's no key to hash and flagd's contract is to fall back to the default variant.

### Weights

Weights are **relative**, not percentages: `[50, 50]` and `[1, 1]` produce identical splits because the operator divides by the total. This lets you grow a rollout from `[1, 99]` → `[50, 50]` → `[99, 1]` without renormalizing.

Omitted weights default to `1`, so `["red"], ["blue"]` is equivalent to `["red", 1], ["blue", 1]`. Negative weights clamp to `0`.

### Composing with `if`

Real-world usage typically gates `fractional` behind a precondition rather than running it unconditionally:

```json
{
  "if": [
    { "in": ["@example.com", { "var": "email" }] },
    {
      "fractional": [
        { "cat": [{ "var": "$flagd.flagKey" }, { "var": "email" }] },
        ["new-ui", 50],
        ["old-ui", 50]
      ]
    },
    "old-ui"
  ]
}
```

## sem_ver

Semantic-version comparison with the four normalizations the flagd spec calls for.

**Reference:** [flagd SemVer spec](https://flagd.dev/reference/custom-operations/semver-operation/)

**Syntax:**

```json
{ "sem_ver": [version1, operator, version2] }
```

**Operators:**

| Operator | Meaning |
|----------|---------|
| `=`  | Exact match |
| `!=` | Not equal |
| `<`  | Less than |
| `<=` | Less or equal |
| `>`  | Greater than |
| `>=` | Greater or equal |
| `^`  | Same major version (caret-style "compatible") |
| `~`  | Same major + minor (tilde-style "approximate") |

Comparison follows SemVer 2.0 precedence, including pre-release ordering: `1.0.0-alpha < 1.0.0-beta < 1.0.0`.

### Input normalizations

The operator applies four normalizations to both version arguments before parsing — matching what the flagd evaluator and most other flagd providers do:

1. **Strip leading `v` / `V`** — `"v1.2.3"`, `"V1.2.3"`, and `"1.2.3"` are all equivalent.
2. **Pad partial versions** — `"1"` becomes `"1.0.0"`, `"1.2"` becomes `"1.2.0"`.
3. **Coerce numeric input** — `1` (a JSON number) is treated as the string `"1"`, then padded.
4. **Drop build metadata** — `"1.2.3+build.7"` is treated as `"1.2.3"`. (SemVer 2.0 specifies build metadata is ignored when determining precedence.)

### Examples

```json
// Simple comparison
{ "sem_ver": [{ "var": "app_version" }, ">=", "1.2.0"] }

// Caret: same major
{ "sem_ver": [{ "var": "app_version" }, "^", "1.0.0"] }
// matches 1.0.0, 1.5.3, 1.99.99 — but not 2.0.0

// Tilde: same major + minor
{ "sem_ver": [{ "var": "app_version" }, "~", "1.2.0"] }
// matches 1.2.0, 1.2.5, 1.2.99 — but not 1.3.0

// v-prefixed input is handled transparently
{ "sem_ver": ["v1.2.3", "=", "1.2.3"] }            // true
{ "sem_ver": ["1.2", "<", "1.2.1"] }                // true (1.2 padded to 1.2.0)
{ "sem_ver": [1, "=", "v1.0.0"] }                   // true (int 1 coerced)
```

### Gated rollout pattern

The common shape for shipping a feature only to clients on a recent version:

```json
{
  "if": [
    { "sem_ver": [{ "var": "app_version" }, ">=", "2.0.0"] },
    { "fractional": [
        { "cat": [{ "var": "$flagd.flagKey" }, { "var": "user_id" }] },
        ["new-checkout", 10],
        ["old-checkout", 90]
    ]},
    "old-checkout"
  ]
}
```

## Conformance

The conformance test suites live under [`crates/datalogic-rs/tests/suites/flagd/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs/tests/suites/flagd) and mirror the upstream Go test fixtures:

- [`fractional_test.go`](https://github.com/open-feature/flagd/blob/main/core/pkg/evaluator/fractional_test.go)
- [`semver_test.go`](https://github.com/open-feature/flagd/blob/main/core/pkg/evaluator/semver_test.go)

Every release runs the full suite, so any flagd-spec drift gets caught before publish.
