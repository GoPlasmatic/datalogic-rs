# Control Flow Operators

Conditional branching and value selection operators.

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
