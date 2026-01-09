# Error Handling Operators

Operators for throwing and catching errors, providing exception-like error handling in JSONLogic.

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
When an error is caught, the catch expression can access error details via `var`:
- `{ "var": "message" }` - Error message
- `{ "var": "code" }` - Error code (if thrown with one)
- `{ "var": "" }` - Entire error object

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

// Catch with error access
{ "try": [
    { "throw": { "code": "NOT_FOUND", "message": "User not found" } },
    { "cat": ["Error: ", { "var": "message" }] }
]}
// Result: "Error: User not found"

// Access error code
{ "try": [
    { "throw": { "code": 404 } },
    { "var": "code" }
]}
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
    { "cat": ["Operation failed: ", { "var": "message" }] }
]}
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
{ "throw": { "code": code, "message": message, ...} }
```

**Arguments:**
- `message` - Error message string, or
- Error object with `code`, `message`, and additional properties

**Returns:** Never returns normally; throws an error that must be caught by `try`.

**Examples:**

```json
// Simple string error
{ "throw": "Something went wrong" }
// Throws error with message "Something went wrong"

// Error with code
{ "throw": { "code": "INVALID_INPUT", "message": "Age must be positive" } }
// Throws error with code and message

// Error with additional data
{ "throw": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid email format",
    "field": "email",
    "value": { "var": "email" }
}}
// Throws detailed error with context

// Conditional throw
{ "if": [
    { "<": [{ "var": "age" }, 0] },
    { "throw": { "code": "INVALID_AGE", "message": "Age cannot be negative" } },
    { "var": "age" }
]}
// Data: { "age": -5 }
// Throws error

// Data: { "age": 25 }
// Result: 25
```

### Common Patterns

**Validation with throw:**
```json
{ "if": [
    { "missing": ["name", "email"] },
    { "throw": {
        "code": "MISSING_FIELDS",
        "message": "Required fields missing",
        "fields": { "missing": ["name", "email"] }
    }},
    "valid"
]}
```

**Business rule enforcement:**
```json
{ "if": [
    { ">": [{ "var": "amount" }, { "var": "balance" }] },
    { "throw": {
        "code": "INSUFFICIENT_FUNDS",
        "message": "Amount exceeds balance",
        "requested": { "var": "amount" },
        "available": { "var": "balance" }
    }},
    { "-": [{ "var": "balance" }, { "var": "amount" }] }
]}
```

**Type validation:**
```json
{ "if": [
    { "!==": [{ "type": { "var": "value" } }, "number"] },
    { "throw": { "code": "TYPE_ERROR", "message": "Expected number" } },
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
        { "throw": { "code": "EMPTY", "message": "Input required" } },
        { "if": [
            { "<": [{ "length": { "var": "input" } }, 3] },
            { "throw": { "code": "TOO_SHORT", "message": "Minimum 3 characters" } },
            { "var": "input" }
        ]}
    ]},
    { "cat": ["Validation error: ", { "var": "message" }] }
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
