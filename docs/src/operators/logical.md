# Logical Operators

Boolean logic operators. `and` and `or` short-circuit: they stop at the first
operand that decides the result and leave the rest unevaluated.

> **Feature flags (Rust crate).** All logical operators are baseline: every build has them. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## ! (Not)

Logical NOT: negates the truthiness of a value.

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
// Result: true (no argument, treated as null)

// An array argument is the argument list: only the first element is negated
{ "!": [0, 2] }
// Result: true (negates 0; the 2 is ignored)

// To negate a literal array, wrap it so the array is the single argument
{ "!": [[1, 2]] }
// Result: false (non-empty array is truthy)
```

**Try it:**

<div class="playground-widget" data-logic='{"!": 0}' data-data='{}'>
</div>

**Notes:**
- Uses the engine's truthiness rules (default: JavaScript-style; see [Truthiness Reference](#truthiness-reference))
- An array argument is read as the argument list, and `!` uses only its first element. To negate a literal array, wrap it: `{ "!": [[1, 2]] }`

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

// A literal array must be wrapped so it is the single argument
{ "!!": [[1, 2, 3]] }
// Result: true

{ "!!": [] }
// Result: false

// Unwrapped, the array is the argument list and only the first element counts
{ "!!": [0, 2, 3] }
// Result: false

{ "!!": null }
// Result: false
```

**Try it:**

<div class="playground-widget" data-logic='{"!!": "hello"}' data-data='{}'>
</div>

**Notes:**
- Equivalent to `{ "!": { "!": value } }`; use it to turn any value into a boolean
- Like `!`, an array argument is the argument list and only its first element is inspected; wrap a literal array (`{ "!!": [[]] }` is `false`, `{ "!!": [{}] }` is `false`)

---

## and

Logical AND with short-circuit evaluation.

**Syntax:**
```json
{ "and": [a, b, ...] }
```

**Arguments:**
- `a`, `b`, ... - One or more values to AND together, always passed as an array. A single-element array returns that element; an empty array returns `null`; a bare scalar (`{ "and": 5 }`) is an Invalid Arguments error

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
- Short-circuits: stops at the first falsy value
- Returns the operand itself, which need not be a boolean

---

## or

Logical OR with short-circuit evaluation.

**Syntax:**
```json
{ "or": [a, b, ...] }
```

**Arguments:**
- `a`, `b`, ... - One or more values to OR together, always passed as an array. A single-element array returns that element; an empty array returns `null`; a bare scalar (`{ "or": "x" }`) is an Invalid Arguments error

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
- Short-circuits: stops at the first truthy value
- Returns the operand itself, which need not be a boolean, so `or` can supply a default value

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
| `{}` | No |
| `null` | No |

`EvaluationConfig::truthy_evaluator` replaces these rules wherever an operator tests truthiness (`!`, `!!`, `and`, `or`, `if`, and the conditions of `filter`, `all`, `some` and `none`); see [Truthiness Evaluation](../advanced/configuration.md#truthiness-evaluation). To apply the engine's rules to a value in host code, such as an evaluated result, call [`Engine::truthy_of`](../rust/api-reference.md#truthy_of).
