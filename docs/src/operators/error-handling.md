# Error Handling Operators

Operators for throwing and catching errors, providing exception-like error handling in JSONLogic.

> **Feature flag (Rust crate).** `try` and `throw` require the `error-handling` feature. Every language binding enables it. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## try

Catch errors and provide fallback values.

**Syntax:**
```json
{ "try": [expression, fallback] }
{ "try": [expression, catch_expression] }
```

**Arguments:**
- `expression` - Expression that might throw an error
- `fallback` - Value or expression to use if an error occurs

**Returns:** Result of expression if successful, or fallback value/expression result if an error occurs.

**Context in Catch:**
When an error is caught, the catch expression evaluates with the thrown error
object as its context, so its fields are read via `var` / `val`:
- A string `throw` produces the error object `{ "type": <string> }`, so the
  message is read with `{ "var": "type" }`.
- An object `throw` (sourced from data) preserves its own keys, so fields such
  as `{ "var": "code" }` or `{ "var": "message" }` read those keys directly.
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

// Nested try for multiple error sources
{ "try": [
    { "try": [
        { "var": "data.nested.value" },
        { "throw": "nested access failed" }
    ]},
    "default"
]}
```

### Common Patterns

**Safe division:**
```json
{ "try": [
    { "/": [{ "var": "numerator" }, { "var": "denominator" }] },
    0
]}
```

**Safe property access:**
```json
{ "try": [
    { "var": "user.profile.settings.theme" },
    "default-theme"
]}
```

**Error logging pattern:**
```json
{ "try": [
    { "risky_operation": [] },
    { "cat": ["Operation failed: ", { "var": "type" }] }
]}
// For a string throw, the thrown text is in the "type" field. If the operation
// throws a structured object instead, read the relevant key (e.g. "message").
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
- `error_object` - An error object value (sourced from data, or built in templating mode) with arbitrary keys such as `code` and `message`. A multi-key object written inline as a literal does NOT compile in the default engine, because it is parsed as an operator map.

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

```json
{ "try": [
    { "var": "user.preferences.language" },
    { "try": [
        { "var": "defaults.language" },
        "en"
    ]}
]}
// Try user preference, then defaults, then hardcoded "en"
```

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
```

### Error Recovery with Retry Logic

```json
{ "try": [
    { "primary_operation": [] },
    { "try": [
        { "fallback_operation": [] },
        "all operations failed"
    ]}
]}
```

### Collecting All Errors

While JSONLogic doesn't natively support collecting multiple errors, you can structure validations to report all issues:

```json
{
    "errors": { "filter": [
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
}
```

This returns an array of error messages for all validation failures.
