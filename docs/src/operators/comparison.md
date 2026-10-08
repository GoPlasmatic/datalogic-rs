# Comparison Operators

Operators for comparing values. `==`, `===` and the ordering operators take
two or more operands and stop at the first comparison that fails, leaving
later operands unevaluated. `!=` and `!==` take exactly two.

Numbers compare by value, and two integers compare exactly, also above 2^53
(`{ "===": [9007199254740993, 9007199254740992] }` is `false`). With the
`datetime` feature, two datetimes compare as instants and two durations by
length, whether written as strings or as `{ "datetime": ... }` /
`{ "timestamp": ... }` values: `{ "==": ["2024-01-01T00:00:00Z", "2024-01-01T00:00:00+00:00"] }`
is `true`. Only a single-key `{ "datetime": ... }` or `{ "timestamp": ... }`
object counts as one; a record that also has other fields is ordinary data.

> **Feature flags (Rust crate).** All comparison operators are baseline: every build has them. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## == (Equals)

Loose equality comparison with type coercion.

**Syntax:**
```json
{ "==": [a, b] }
{ "==": [a, b, c, ...] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c`, ... - Optional further values; each must equal `a`

**Returns:** `true` if every value equals the first (after type coercion), `false` otherwise.

**Examples:**

```json
// Same type
{ "==": [1, 1] }
// Result: true

// Type coercion
{ "==": [1, "1"] }
// Result: true

{ "==": [0, false] }
// Result: true

{ "==": ["", false] }
// Result: false (a String and a Bool are compared as strings, with no JS numeric coercion)

// Null comparison
{ "==": [null, null] }
// Result: true

// Arrays
{ "==": [[1, 2], [1, 2]] }
// Result: true

// More than two operands
{ "==": [1, "1", 1.0] }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"==": [1, "1"]}' data-data='{}'>
</div>

**Coercion rules:**

| Operands | Rule |
|----------|------|
| number and string | The string is read as a number: `{ "==": [1, "1"] }` is `true`. An integer string matches an integer exactly, also above 2^53 |
| number and boolean | `true` is `1`, `false` is `0` |
| string and boolean | The string must be `"true"` or `"false"`: `{ "==": ["false", false] }` is `true`, `{ "==": ["", false] }` is `false` |
| `null` and number, boolean or string | `null` equals `0`, `false` and `""` |
| two arrays | Equal when their elements are equal under `===`: `{ "==": [[1], [1.0]] }` is `true`, `{ "==": [[1], ["1"]] }` is an error (see below) |
| two objects | `false`; use `===` to compare objects by content |

These rules differ from JavaScript's `==`: `{ "==": [null, 0] }` is `true` here.

Operands that cannot be compared are an error by default (error type `NaN`): a number against a string that is not a finite number (`{ "==": [1, "abc"] }`, `{ "==": [1, "NaN"] }`), an array or object against a scalar (`{ "==": [1, [1]] }`), and two arrays that differ. Set `EvaluationConfig::loose_equality_errors` to `false` to get `false` instead (see [Loose Equality Errors](../advanced/configuration.md#loose-equality-errors)). For comparison without coercion, use `===`.

---

## === (Strict Equals)

Strict equality comparison without type coercion.

**Syntax:**
```json
{ "===": [a, b] }
{ "===": [a, b, c, ...] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c`, ... - Optional further values; each must equal `a`

**Returns:** `true` if every value has the same type and value as the first, `false` otherwise.

**Examples:**

```json
// Same type and value
{ "===": [1, 1] }
// Result: true

// Different types
{ "===": [1, "1"] }
// Result: false

{ "===": [0, false] }
// Result: false

// Null
{ "===": [null, null] }
// Result: true

// An integer and a float with the same value
{ "===": [1, 1.0] }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"===": [1, "1"]}' data-data='{}'>
</div>

**Notes:**
- Arrays and objects compare by content: two objects with the same keys and values are equal
- `===` never raises an error for mismatched types; it returns `false`

---

## != (Not Equals)

Loose inequality comparison with type coercion.

**Syntax:**
```json
{ "!=": [a, b] }
```

**Arguments:**
- `a` - First value
- `b` - Second value

**Returns:** `true` if values are not equal (after type coercion), `false` otherwise. The negation of `==`, with the same coercion rules and errors.

**Examples:**

```json
{ "!=": [1, 2] }
// Result: true

{ "!=": [1, "1"] }
// Result: false (type coercion makes them equal)

{ "!=": ["hello", "world"] }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"!=": [1, 2]}' data-data='{}'>
</div>

**Notes (!=, !==):**
- Exactly two operands are compared. Further operands are ignored and never evaluated: `{ "!=": [1, 1, 2] }` is `false`
- Fewer than two operands is an Invalid Arguments error

---

## !== (Strict Not Equals)

Strict inequality comparison without type coercion.

**Syntax:**
```json
{ "!==": [a, b] }
```

**Arguments:**
- `a` - First value
- `b` - Second value

**Returns:** `true` if values are not equal or different types, `false` otherwise. The negation of `===`.

**Examples:**

```json
{ "!==": [1, "1"] }
// Result: true (different types)

{ "!==": [1, 1] }
// Result: false

{ "!==": [1, 2] }
// Result: true
```

---

## > (Greater Than)

Check if the first value is greater than the second.

**Syntax:**
```json
{ ">": [a, b] }
{ ">": [a, b, c, ...] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c`, ... - Optional further values for chained comparison (any number)

**Returns:** `true` if a > b (and b > c if provided), `false` otherwise.

**Examples:**

```json
// Simple comparison
{ ">": [5, 3] }
// Result: true

{ ">": [3, 5] }
// Result: false

// Chained comparison (a > b > c)
{ ">": [5, 3, 1] }
// Result: true (5 > 3 AND 3 > 1)

{ ">": [5, 3, 4] }
// Result: false (3 is not > 4)

// String comparison
{ ">": ["b", "a"] }
// Result: true (lexicographic)

// With variables
{ ">": [{ "var": "age" }, 18] }
// Data: { "age": 21 }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{">":[{"var":"age"}, 18]}' data-data='{"age": 21}'>
</div>

---

## >= (Greater Than or Equal)

Check if the first value is greater than or equal to the second.

**Syntax:**
```json
{ ">=": [a, b] }
{ ">=": [a, b, c, ...] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c`, ... - Optional further values for chained comparison (any number)

**Returns:** `true` if a >= b (and b >= c if provided), `false` otherwise.

**Examples:**

```json
{ ">=": [5, 5] }
// Result: true

{ ">=": [5, 3] }
// Result: true

{ ">=": [3, 5] }
// Result: false

// Chained
{ ">=": [5, 3, 3] }
// Result: true (5 >= 3 AND 3 >= 3)
```

---

## < (Less Than)

Check if the first value is less than the second.

**Syntax:**
```json
{ "<": [a, b] }
{ "<": [a, b, c, ...] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c`, ... - Optional further values for chained comparison (any number)

**Returns:** `true` if a < b (and b < c if provided), `false` otherwise.

**Examples:**

```json
{ "<": [3, 5] }
// Result: true

{ "<": [5, 3] }
// Result: false

// Chained (useful for range checks)
{ "<": [1, 5, 10] }
// Result: true (1 < 5 AND 5 < 10)

// Range check: is x between 1 and 10?
{ "<": [1, { "var": "x" }, 10] }
// Data: { "x": 5 }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"<": [1, {"var":"x"}, 10]}' data-data='{"x": 5}'>
</div>

---

## <= (Less Than or Equal)

Check if the first value is less than or equal to the second.

**Syntax:**
```json
{ "<=": [a, b] }
{ "<=": [a, b, c, ...] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c`, ... - Optional further values for chained comparison (any number)

**Returns:** `true` if a <= b (and b <= c if provided), `false` otherwise.

**Examples:**

```json
{ "<=": [3, 5] }
// Result: true

{ "<=": [5, 5] }
// Result: true

{ "<=": [5, 3] }
// Result: false

// Range check (inclusive)
{ "<=": [1, { "var": "x" }, 10] }
// Data: { "x": 10 }
// Result: true (1 <= 10 AND 10 <= 10)
```

**Notes (all ordering operators):**
- `{ "<": [a, x, b] }` is equivalent to `a < x AND x < b`, which suits range checks
- The operators accept any number of operands: `{ "<": [1, 2, 3, 4] }` is `true` and `{ "<": [1, 2, 5, 4] }` is `false`. Evaluation short-circuits at the first failing pair and never evaluates later operands (`{ ">": [5, 3, 4, { "throw": "boom" }] }` is `false`, nothing is thrown)
- Fewer than two operands (`{ "<": [1] }`, `{ "<": [] }`) is an Invalid Arguments error
- Two strings compare lexicographically (`{ ">": ["10", "9"] }` is `false`); a string against a number is coerced numerically (`{ ">": ["10", 9] }` is `true`). A string that is not a finite number against a number throws a `NaN` error (`{ "<": ["abc", 1] }`, `{ "<": [1, "inf"] }`)
- `null` and booleans compare as numbers (`{ "<": [null, 1] }` and `{ "<": [false, true] }` are `true`)
- An array or object operand (other than a datetime or duration value) throws a `NaN` error
