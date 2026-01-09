# Logical Operators

Boolean logic operators with short-circuit evaluation.

## ! (Not)

Logical NOT - negates a boolean value.

**Syntax:**
```json
{ "!": value }
{ "!": [value] }
```

**Arguments:**
- `value` - Value to negate

**Returns:** `true` if value is falsy, `false` if value is truthy.

**Examples:**

```json
{ "!": true }
// Result: false

{ "!": false }
// Result: true

{ "!": 0 }
// Result: true (0 is falsy)

{ "!": 1 }
// Result: false (1 is truthy)

{ "!": "" }
// Result: true (empty string is falsy)

{ "!": "hello" }
// Result: false (non-empty string is truthy)

{ "!": null }
// Result: true (null is falsy)

{ "!": [] }
// Result: true (empty array is falsy)

{ "!": [1, 2] }
// Note: This negates the array [1, 2], not [value]
// Result: false (non-empty array is truthy)
```

**Try it:**

<div class="playground-widget" data-logic='{"!": 0}' data-data='{}'>
</div>

**Notes:**
- Uses configurable truthiness rules (default: JavaScript-style)
- Falsy values: `false`, `0`, `""`, `null`, `[]`
- Truthy values: everything else

---

## !! (Double Not / Boolean Cast)

Convert a value to its boolean equivalent.

**Syntax:**
```json
{ "!!": value }
{ "!!": [value] }
```

**Arguments:**
- `value` - Value to convert to boolean

**Returns:** `true` if value is truthy, `false` if value is falsy.

**Examples:**

```json
{ "!!": true }
// Result: true

{ "!!": false }
// Result: false

{ "!!": 1 }
// Result: true

{ "!!": 0 }
// Result: false

{ "!!": "hello" }
// Result: true

{ "!!": "" }
// Result: false

{ "!!": [1, 2, 3] }
// Result: true

{ "!!": [] }
// Result: false

{ "!!": null }
// Result: false
```

**Try it:**

<div class="playground-widget" data-logic='{"!!": "hello"}' data-data='{}'>
</div>

**Notes:**
- Equivalent to `{ "!": { "!": value } }`
- Useful for ensuring a boolean result from any value

---

## and

Logical AND with short-circuit evaluation.

**Syntax:**
```json
{ "and": [a, b, ...] }
```

**Arguments:**
- `a`, `b`, ... - Two or more values to AND together

**Returns:** The first falsy value encountered, or the last value if all are truthy.

**Examples:**

```json
// All truthy
{ "and": [true, true] }
// Result: true

// One falsy
{ "and": [true, false] }
// Result: false

// Short-circuit: returns first falsy
{ "and": [true, 0, "never evaluated"] }
// Result: 0

// All truthy returns last value
{ "and": [1, 2, 3] }
// Result: 3

// Multiple conditions
{ "and": [
    { ">": [{ "var": "age" }, 18] },
    { "==": [{ "var": "verified" }, true] },
    { "!=": [{ "var": "banned" }, true] }
]}
// Data: { "age": 21, "verified": true, "banned": false }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"and": [{">":[{"var":"age"}, 18]}, {"==":[{"var":"verified"}, true]}]}' data-data='{"age": 21, "verified": true}'>
</div>

**Notes:**
- Short-circuits: stops at first falsy value
- Returns the actual value, not necessarily a boolean
- Empty `and` returns `true` (vacuous truth)

---

## or

Logical OR with short-circuit evaluation.

**Syntax:**
```json
{ "or": [a, b, ...] }
```

**Arguments:**
- `a`, `b`, ... - Two or more values to OR together

**Returns:** The first truthy value encountered, or the last value if all are falsy.

**Examples:**

```json
// One truthy
{ "or": [false, true] }
// Result: true

// All falsy
{ "or": [false, false] }
// Result: false

// Short-circuit: returns first truthy
{ "or": [0, "", "found it", "not evaluated"] }
// Result: "found it"

// All falsy returns last value
{ "or": [false, 0, ""] }
// Result: ""

// Default value pattern
{ "or": [{ "var": "nickname" }, { "var": "name" }, "Anonymous"] }
// Data: { "name": "Alice" }
// Result: "Alice" (nickname is null/missing, so returns name)

// Role check
{ "or": [
    { "==": [{ "var": "role" }, "admin"] },
    { "==": [{ "var": "role" }, "moderator"] }
]}
// Data: { "role": "admin" }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"or": [{"var":"nickname"}, {"var":"name"}, "Anonymous"]}' data-data='{"name": "Alice"}'>
</div>

**Notes:**
- Short-circuits: stops at first truthy value
- Returns the actual value, not necessarily a boolean
- Useful for default value patterns
- Empty `or` returns `false`

---

## Truthiness Reference

The default JavaScript-style truthiness:

| Value | Truthy? |
|-------|---------|
| `true` | Yes |
| `false` | No |
| `1`, `2`, `-1`, `3.14` | Yes |
| `0`, `0.0` | No |
| `"hello"`, `"0"`, `"false"` | Yes |
| `""` | No |
| `[1, 2]`, `{"a": 1}` | Yes |
| `[]` | No |
| `null` | No |

This can be customized via `EvaluationConfig`. See [Configuration](../advanced/configuration.md).
