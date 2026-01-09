# Comparison Operators

Operators for comparing values. All comparison operators support lazy evaluation.

## == (Equals)

Loose equality comparison with type coercion.

**Syntax:**
```json
{ "==": [a, b] }
```

**Arguments:**
- `a` - First value
- `b` - Second value

**Returns:** `true` if values are equal (after type coercion), `false` otherwise.

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
// Result: true

// Null comparison
{ "==": [null, null] }
// Result: true

// Arrays
{ "==": [[1, 2], [1, 2]] }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"==": [1, "1"]}' data-data='{}'>
</div>

**Notes:**
- Performs type coercion similar to JavaScript's `==`
- For strict comparison without coercion, use `===`

---

## === (Strict Equals)

Strict equality comparison without type coercion.

**Syntax:**
```json
{ "===": [a, b] }
```

**Arguments:**
- `a` - First value
- `b` - Second value

**Returns:** `true` if values are equal and same type, `false` otherwise.

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
```

**Try it:**

<div class="playground-widget" data-logic='{"===": [1, "1"]}' data-data='{}'>
</div>

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

**Returns:** `true` if values are not equal (after type coercion), `false` otherwise.

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

**Returns:** `true` if values are not equal or different types, `false` otherwise.

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
{ ">": [a, b, c] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c` - Optional third value for chained comparison

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
{ ">=": [a, b, c] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c` - Optional third value for chained comparison

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
{ "<": [a, b, c] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c` - Optional third value for chained comparison

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
{ "<=": [a, b, c] }
```

**Arguments:**
- `a`, `b` - Values to compare
- `c` - Optional third value for chained comparison

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

**Notes:**
- Chained comparisons are useful for range checks
- `{ "<": [a, x, b] }` is equivalent to `a < x AND x < b`
