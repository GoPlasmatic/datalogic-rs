# Control Flow Operators

Conditional branching and value selection operators.

> **Feature flags (Rust crate).** `if` and `?:` are baseline; `??`, `switch`/`match`, and `type` require the `ext-control` feature. Every language binding enables all operator features. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## if

Conditional branching with if/then/else chains.

**Syntax:**
```json
{ "if": [condition, then_value] }
{ "if": [condition, then_value, else_value] }
{ "if": [cond1, value1, cond2, value2, ..., else_value] }
```

**Arguments:**
- `condition` - Condition to evaluate
- `then_value` - Value if condition is truthy
- `else_value` - Value if condition is falsy (optional)
- Additional condition/value pairs for else-if chains

**Returns:** The value corresponding to the first truthy condition, or the else value.

**Examples:**

```json
// Simple if/then
{ "if": [true, "yes"] }
// Result: "yes"

{ "if": [false, "yes"] }
// Result: null

// If/then/else
{ "if": [true, "yes", "no"] }
// Result: "yes"

{ "if": [false, "yes", "no"] }
// Result: "no"

// If/else-if/else chain
{ "if": [
    { ">=": [{ "var": "score" }, 90] }, "A",
    { ">=": [{ "var": "score" }, 80] }, "B",
    { ">=": [{ "var": "score" }, 70] }, "C",
    { ">=": [{ "var": "score" }, 60] }, "D",
    "F"
]}
// Data: { "score": 85 }
// Result: "B"

// Nested if
{ "if": [
    { "var": "premium" },
    { "if": [
        { ">": [{ "var": "amount" }, 100] },
        "free_shipping",
        "standard_shipping"
    ]},
    "no_shipping"
]}
// Data: { "premium": true, "amount": 150 }
// Result: "free_shipping"
```

**Try it:**

<div class="playground-widget" data-logic='{"if": [{">=": [{"var":"score"}, 90]}, "A", {">=": [{"var":"score"}, 80]}, "B", {">=": [{"var":"score"}, 70]}, "C", "F"]}' data-data='{"score": 85}'>
</div>

**Notes:**
- Only evaluates the matching branch (lazy evaluation)
- Empty condition list returns `null`
- Odd number of arguments uses last as else value

---

## ?: (Ternary)

Ternary conditional operator (shorthand if/then/else).

**Syntax:**
```json
{ "?:": [condition, then_value, else_value] }
```

**Arguments:**
- `condition` - Condition to evaluate
- `then_value` - Value if condition is truthy
- `else_value` - Value if condition is falsy

**Returns:** `then_value` if condition is truthy, `else_value` otherwise.

**Examples:**

```json
// Basic ternary
{ "?:": [true, "yes", "no"] }
// Result: "yes"

{ "?:": [false, "yes", "no"] }
// Result: "no"

// With comparison
{ "?:": [
    { ">": [{ "var": "age" }, 18] },
    "adult",
    "minor"
]}
// Data: { "age": 21 }
// Result: "adult"

// Nested ternary
{ "?:": [
    { "var": "vip" },
    0,
    { "?:": [
        { ">": [{ "var": "total" }, 50] },
        5,
        10
    ]}
]}
// Data: { "vip": false, "total": 75 }
// Result: 5 (shipping cost)
```

**Try it:**

<div class="playground-widget" data-logic='{"?:": [{">": [{"var":"age"}, 18]}, "adult", "minor"]}' data-data='{"age": 21}'>
</div>

**Notes:**
- Equivalent to `{ "if": [condition, then_value, else_value] }`
- More concise for simple conditions
- Only evaluates the matching branch

---

## ?? (Null Coalesce)

Return the first non-null value.

**Syntax:**
```json
{ "??": [a, b] }
{ "??": [a, b, c, ...] }
```

**Arguments:**
- `a`, `b`, ... - Values to check (variadic)

**Returns:** The first non-null value, or `null` if all are null.

**Examples:**

```json
// First is not null
{ "??": ["hello", "default"] }
// Result: "hello"

// First is null
{ "??": [null, "default"] }
// Result: "default"

// Multiple values
{ "??": [null, null, "found"] }
// Result: "found"

// All null
{ "??": [null, null] }
// Result: null

// With variables (default value pattern)
{ "??": [{ "var": "nickname" }, { "var": "name" }, "Anonymous"] }
// Data: { "name": "Alice" }
// Result: "Alice"

// Note: 0, "", and false are NOT null
{ "??": [0, "default"] }
// Result: 0

{ "??": ["", "default"] }
// Result: ""

{ "??": [false, "default"] }
// Result: false
```

**Try it:**

<div class="playground-widget" data-logic='{"??": [{"var":"nickname"}, {"var":"name"}, "Anonymous"]}' data-data='{"name": "Alice"}'>
</div>

**Notes:**
- Only checks for `null`, not other falsy values
- Use `or` if you want to skip all falsy values
- Short-circuits: stops at first non-null value

---

## switch / match

Match a value against a list of cases, returning the result of the first case
whose key strictly equals the value, or a default. `match` is an alias of
`switch`.

> **Experimental / known issue:** in the current build this operator does not
> match cases correctly; every input falls through to the default. The syntax
> and behavior below describe the intended design. See the note at the end of
> this section.

**Syntax:**
```json
{ "switch": [value, [[case, result], ...]] }
{ "switch": [value, [[case, result], ...], default] }
```

**Arguments:**
- `value` - The discriminant, evaluated once
- `[[case, result], ...]` - Array of `[case, result]` pairs; the first case that strictly equals `value` selects its result
- `default` - Optional value used when no case matches (omitted: returns `null`)

**Returns:** The matched case's result, the default, or `null`.

**Examples (intended behavior):**

```json
{ "switch": [
    { "var": "color" },
    [["red", "stop"], ["green", "go"]],
    "unknown"
]}
// Data: { "color": "green" }
// Intended result: "go"

// Alias `match`
{ "match": [
    { "var": "status" },
    [[200, "OK"], [404, "Not Found"]],
    "Unknown"
]}
// Data: { "status": 404 }
// Intended result: "Not Found"
```

**Notes:**
- Case comparison is strict (no type coercion): the number `1` does not match the string `"1"`.
- The discriminant is evaluated once and compared against each case in order.
- Only the matching case's result (or the default) is evaluated.
- **Known issue:** this operator is currently broken in this build, falling through to the default for every input. Avoid relying on it until it is fixed.

---

## type

Return the runtime type of a value as a string.

**Syntax:**
```json
{ "type": value }
```

**Arguments:**
- `value` - Any value to inspect

**Returns:** One of `"null"`, `"boolean"`, `"number"`, `"string"`, `"array"`, `"object"`, `"datetime"`, or `"duration"`.

**Examples:**

```json
{ "type": 42 }
// Result: "number"

{ "type": "hello" }
// Result: "string"

// A value that resolves to an array
{ "type": { "var": "items" } }
// Data: { "items": [1, 2, 3] }
// Result: "array"

{ "type": { "now": [] } }
// Result: "datetime"
```

**Notes:**
- `type` reads exactly one argument. A literal array such as `{ "type": [1, 2, 3] }` is parsed as a multi-argument call, so it inspects the first element (here, `"number"`). Pass a single value that resolves to an array, e.g. `{ "type": { "var": "items" } }`.
- Datetime and duration values (from `now`, `datetime`, `timestamp`) report `"datetime"` / `"duration"`, even though they render as strings in JSON output.

---

## Comparison: if vs ?: vs ?? vs or

| Operator | Use Case | Falsy Handling |
|----------|----------|----------------|
| `if` | Complex branching, multiple conditions | Evaluates truthiness |
| `?:` | Simple if/else | Evaluates truthiness |
| `??` | Default for null only | Only skips `null` |
| `or` | Default for any falsy | Skips all falsy values |

**Examples:**

```json
// Value is 0 (falsy but not null)
// Data: { "count": 0 }

{ "if": [{ "var": "count" }, { "var": "count" }, 10] }
// Result: 10 (0 is falsy)

{ "?:": [{ "var": "count" }, { "var": "count" }, 10] }
// Result: 10 (0 is falsy)

{ "??": [{ "var": "count" }, 10] }
// Result: 0 (0 is not null)

{ "or": [{ "var": "count" }, 10] }
// Result: 10 (0 is falsy)
```

Choose the operator based on whether you want to treat `0`, `""`, and `false` as valid values.
