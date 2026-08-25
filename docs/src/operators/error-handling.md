# Error Handling Operators

Operators for throwing and catching errors, providing exception-like error handling in JSONLogic.

> **Feature flag (Rust crate).** `try` and `throw` require the `error-handling` feature. Every language binding enables it. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## try

Catch errors and provide fallback values.

**Syntax:**
```json
{ "try": [expression, fallback] }
{ "try": [expression, catch_expression] }
{ "try": [expression, fallback1, fallback2, ...] }
```

**Arguments:**
- `expression` - Expression that might throw an error
- `fallback` - Value or expression to use if an error occurs
- Further arms are tried in order: each arm runs only if every arm before it raised an error. Only the last arm is evaluated with the error object as its context; intermediate arms see the ordinary data context

**Returns:** Result of expression if successful, or fallback value/expression result if an error occurs.

**What counts as an error:**
- Anything raised by `throw`.
- Errors raised by the engine itself: an unknown operator, invalid arguments
  (for example `{ "max": ["a", 1] }`), an integer division by zero, a datetime
  that fails to parse, an unknown timezone, and so on.
- A missing variable is **not** an error: `var` and `val` return `null` for an
  absent path, so `try` never falls back on missing data. Use `??` or `var`'s
  default argument for that (see [Control Flow](control-flow.md)).

**Context in Catch:**
When an error is caught, the catch expression evaluates with the error object
as its context, so its fields are read via `var` / `val`:
- A string `throw` produces the error object `{ "type": <string> }`, so the
  message is read with `{ "var": "type" }`.
- An object `throw` (sourced from data) preserves its own keys, so fields such
  as `{ "var": "code" }` or `{ "var": "message" }` read those keys directly.
- Engine-raised errors arrive as `{ "type": <message> }`: an unknown operator
  gives `{ "type": "Unknown Operator" }`, bad operands give
  `{ "type": "Invalid Arguments" }`, an integer division by zero gives
  `{ "type": "NaN" }`, and other kinds carry their message text (for example
  `{ "type": "Invalid datetime format" }` or
  `{ "type": "Unknown timezone: Mars/Olympus" }`).
- `{ "var": "" }` returns the entire error object.

**Examples:**

```json
// Simple fallback value
{ "try": [
    { "/": [10, 0] },
    0
]}
// Result: 0 (division by zero caught)

// Expression that succeeds
{ "try": [
    { "+": [1, 2] },
    0
]}
// Result: 3 (no error, normal result)

// Catch a string error: the string becomes the error object's "type" field
{ "try": [
    { "throw": "User not found" },
    { "cat": ["Error: ", { "var": "type" }] }
]}
// Result: "Error: User not found"

// Canonical pattern: read a thrown string back via "type"
{ "try": [
    { "throw": "Some error" },
    { "val": "type" }
]}
// Result: "Some error"

// Throw an object sourced from data, then read its fields by key
{ "try": [
    { "throw": { "var": "err" } },
    { "var": "code" }
]}
// Data: { "err": { "code": 404, "message": "User not found" } }
// Result: 404

// Engine errors are caught too: read the kind via "type"
{ "try": [{ "not_an_operator": [1] }, { "var": "type" }] }
// Result: "Unknown Operator"

{ "try": [{ "max": ["a", 1] }, { "var": "type" }] }
// Result: "Invalid Arguments"

// Multiple fallback arms: each runs only if the previous one raised
{ "try": [{ "throw": "a" }, { "throw": "b" }, "c"] }
// Result: "c"

// A missing variable is NOT an error, so try does not fall back
{ "try": [{ "var": "missing" }, "fallback"] }
// Data: {}
// Result: null

// Nested try: the inner arm re-throws a clearer message, the outer arm reads it
{ "try": [
    { "try": [
        { "/": [1, { "var": "d" }] },
        { "throw": "division failed" }
    ]},
    { "var": "type" }
]}
// Data: { "d": 0 }
// Result: "division failed"

// Data: { "d": 4 }
// Result: 0.25
```

### Common Patterns

**Safe division:**
```json
{ "if": [
    { "!": { "var": "denominator" } },
    0,
    { "/": [{ "var": "numerator" }, { "var": "denominator" }] }
]}
// Data: { "numerator": 1 }
// Result: 0

// Data: { "numerator": 1, "denominator": 4 }
// Result: 0.25
```

`try` alone is not enough here. Only an integral-valued zero divisor (or a
non-numeric string) raises the `NaN` error that `try` can catch. A `null`,
`false`, `""`, or `"0"` divisor coerces to zero and follows
`EvaluationConfig::division_by_zero`, whose default (`ReturnSaturated`)
returns `f64::MAX` without raising, so
`{ "try": [{ "/": [{ "var": "numerator" }, { "var": "denominator" }] }, 0] }`
yields `1.7976931348623157e308` when `denominator` is missing. Configure
`DivisionByZeroHandling::ThrowError` if you want every zero divisor to reach
the `try` fallback; see [Arithmetic](arithmetic.md) for the full rule.

**Default for a missing value (not a `try` job):**
```json
{ "??": [{ "var": "user.profile.settings.theme" }, "default-theme"] }
// Data: {}
// Result: "default-theme"
```

`var` on a missing path returns `null` rather than raising, so wrapping it in
`try` never produces the fallback. Use `??` or the `var` default form,
`{ "var": ["user.profile.settings.theme", "default-theme"] }`.

**Error logging pattern:**
```json
{ "try": [
    { "risky_operation": [] },
    { "cat": ["Operation failed: ", { "var": "type" }] }
]}
// Result: "Operation failed: Unknown Operator"
// risky_operation stands in for a custom operator. In the default engine the
// name is not registered, so the call itself raises "Unknown Operator" and is
// caught. For a string throw, the thrown text is in the "type" field; if the
// operation throws a structured object instead, read the relevant key (e.g.
// "message").
```

**Try it:**

<div class="playground-widget" data-logic='{"try": [{"/": [10, 0]}, "Division by zero handled"]}' data-data='{}'>
</div>

---

## throw

Throw an error with optional details.

**Syntax:**
```json
{ "throw": message }
{ "throw": error_object }
```

**Arguments:**
- `message` - Error message string. The string becomes the error object's `type` field, or
- `error_object` - An error object value (sourced from data, or built in templating mode) with arbitrary keys such as `code` and `message`. A multi-key object written inline as a literal does NOT compile in the default engine, because it is parsed as an operator map. A single-key literal such as `{ "throw": { "type": "X" } }` does not work either, even in templating mode: `type` is an operator name, so it runs the `type` operator on `"X"` and throws `{ "type": "string" }`. An error object carrying a `type` key must come from data (`{ "throw": { "var": "err" } }`).

**Returns:** Never returns normally; throws an error that must be caught by `try`.

**Examples:**

```json
// Simple string error (the string lands in the error object's "type" field)
{ "throw": "Something went wrong" }
// Throws the error object { "type": "Something went wrong" }

// Error object sourced from data. A literal multi-key object written inline
// would be parsed as an operator map and fail to compile in the default engine.
{ "throw": { "var": "err" } }
// Data: { "err": { "code": "INVALID_INPUT", "message": "Age must be positive" } }
// Throws an error carrying the object's fields

// Richer error: build the object in your data (or enable templating mode) and
// throw it by reference.
{ "throw": { "var": "validationError" } }
// Data: {
//   "validationError": {
//     "code": "VALIDATION_ERROR",
//     "message": "Invalid email format",
//     "field": "email"
//   }
// }

// Conditional throw (string form)
{ "if": [
    { "<": [{ "var": "age" }, 0] },
    { "throw": "Age cannot be negative" },
    { "var": "age" }
]}
// Data: { "age": -5 }
// Throws the error object { "type": "Age cannot be negative" }

// Data: { "age": 25 }
// Result: 25
```

### Common Patterns

**Validation with throw:**
```json
{ "if": [
    { "missing": ["name", "email"] },
    { "throw": "Required fields missing" },
    "valid"
]}
```

**Business rule enforcement:**
```json
{ "if": [
    { ">": [{ "var": "amount" }, { "var": "balance" }] },
    { "throw": "Amount exceeds balance" },
    { "-": [{ "var": "balance" }, { "var": "amount" }] }
]}
```

**Type validation:**
```json
{ "if": [
    { "!==": [{ "type": { "var": "value" } }, "number"] },
    { "throw": "Expected number" },
    { "*": [{ "var": "value" }, 2] }
]}
```

**Try it:**

<div class="playground-widget" data-logic='{"if": [{"<": [{"var":"age"}, 0]}, {"throw": "Age cannot be negative"}, {"var":"age"}]}' data-data='{"age": 25}'>
</div>

---

## Error Handling Patterns

### Graceful Degradation

Fallback chains over possibly-missing data belong to `??`, not `try`, because
a missing `var` is `null` rather than an error:

```json
{ "??": [
    { "var": "user.preferences.language" },
    { "var": "defaults.language" },
    "en"
]}
// Data: { "defaults": { "language": "fr" } }
// Result: "fr"

// Data: {}
// Result: "en"
```

Reserve `try` for expressions that can actually raise: `throw`, an integer
division by zero, invalid arguments, an unknown operator, a datetime parse
failure.

### Validation Pipeline

```json
{ "try": [
    { "if": [
        { "!": { "var": "input" } },
        { "throw": "Input required" },
        { "if": [
            { "<": [{ "length": { "var": "input" } }, 3] },
            { "throw": "Minimum 3 characters" },
            { "var": "input" }
        ]}
    ]},
    { "cat": ["Validation error: ", { "var": "type" }] }
]}
// Data: {}
// Result: "Validation error: Input required"

// Data: { "input": "ab" }
// Result: "Validation error: Minimum 3 characters"

// Data: { "input": "abc" }
// Result: "abc"
```

### Error Recovery with Fallback Operations

The variadic form tries each arm in turn:

```json
{ "try": [
    { "primary_operation": [] },
    { "fallback_operation": [] },
    "all operations failed"
]}
// Result: "all operations failed"
// primary_operation and fallback_operation stand in for custom operators. Each
// arm runs only if the previous one raised; in the default engine neither name
// is registered, so both raise "Unknown Operator" and the last arm is returned.
```

### Collecting All Errors

While JSONLogic doesn't natively support collecting multiple errors, you can structure validations to report all issues:

```json
{ "filter": [
    [
        { "if": [{ "missing": ["name"] }, "name is required", null] },
        { "if": [{ "missing": ["email"] }, "email is required", null] },
        { "if": [
            { "and": [
                { "!": { "missing": ["email"] } },
                { "!": { "in": ["@", { "var": "email" }] } }
            ]},
            "invalid email format",
            null
        ]}
    ],
    { "!==": [{ "var": "" }, null] }
]}
// Data: { "email": "foo" }
// Result: ["name is required", "invalid email format"]
```

This returns an array of error messages for all validation failures. To wrap
it in an object such as `{ "errors": [...] }`, enable templating mode
(`Engine::builder().with_templating(true)`, Cargo feature `templating`); in the
default engine a top-level `errors` key is parsed as an operator and fails with
`InvalidOperator`.
