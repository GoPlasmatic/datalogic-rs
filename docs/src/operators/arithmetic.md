# Arithmetic Operators

Mathematical operations with type coercion support.

## + (Add)

Add numbers together, or concatenate strings.

**Syntax:**
```json
{ "+": [a, b, ...] }
{ "+": value }
```

**Arguments:**
- `a`, `b`, ... - Values to add (variadic)
- Single value is cast to number

**Returns:** Sum of all arguments, or concatenated string.

**Examples:**

```json
// Basic addition
{ "+": [1, 2] }
// Result: 3

// Multiple values
{ "+": [1, 2, 3, 4] }
// Result: 10

// Type coercion
{ "+": ["5", 3] }
// Result: 8 (string "5" converted to number)

// Unary plus (convert to number)
{ "+": "42" }
// Result: 42

{ "+": "-3.14" }
// Result: -3.14

// With variables
{ "+": [{ "var": "price" }, { "var": "tax" }] }
// Data: { "price": 100, "tax": 8.5 }
// Result: 108.5
```

**Try it:**

<div class="playground-widget" data-logic='{"+": [{"var":"price"}, {"var":"tax"}]}' data-data='{"price": 100, "tax": 8.5}'>
</div>

**Notes:**
- Strings are converted to numbers when possible
- Non-numeric strings may result in NaN or error (configurable)
- Single argument converts value to number

---

## - (Subtract)

Subtract numbers.

**Syntax:**
```json
{ "-": [a, b] }
{ "-": value }
```

**Arguments:**
- `a` - Value to subtract from
- `b` - Value to subtract
- Single value negates it

**Returns:** Difference, or negated value.

**Examples:**

```json
// Subtraction
{ "-": [10, 3] }
// Result: 7

// Unary minus (negate)
{ "-": 5 }
// Result: -5

{ "-": -3 }
// Result: 3

// With coercion
{ "-": ["10", "3"] }
// Result: 7

// Calculate discount
{ "-": [{ "var": "price" }, { "var": "discount" }] }
// Data: { "price": 100, "discount": 15 }
// Result: 85
```

**Try it:**

<div class="playground-widget" data-logic='{"-": [{"var":"price"}, {"var":"discount"}]}' data-data='{"price": 100, "discount": 15}'>
</div>

---

## * (Multiply)

Multiply numbers.

**Syntax:**
```json
{ "*": [a, b, ...] }
```

**Arguments:**
- `a`, `b`, ... - Values to multiply (variadic)

**Returns:** Product of all arguments.

**Examples:**

```json
// Basic multiplication
{ "*": [3, 4] }
// Result: 12

// Multiple values
{ "*": [2, 3, 4] }
// Result: 24

// With coercion
{ "*": ["5", 2] }
// Result: 10

// Calculate total
{ "*": [{ "var": "quantity" }, { "var": "price" }] }
// Data: { "quantity": 3, "price": 25 }
// Result: 75

// Apply percentage
{ "*": [{ "var": "amount" }, 0.1] }
// Data: { "amount": 200 }
// Result: 20
```

**Try it:**

<div class="playground-widget" data-logic='{"*": [{"var":"quantity"}, {"var":"price"}]}' data-data='{"quantity": 3, "price": 25}'>
</div>

---

## / (Divide)

Divide numbers.

**Syntax:**
```json
{ "/": [a, b] }
```

**Arguments:**
- `a` - Dividend
- `b` - Divisor

**Returns:** Quotient.

**Examples:**

```json
// Basic division
{ "/": [10, 2] }
// Result: 5

// Decimal result
{ "/": [7, 2] }
// Result: 3.5

// Division by zero (configurable behavior)
{ "/": [10, 0] }
// Result: Infinity (default) or error

// With coercion
{ "/": ["100", "4"] }
// Result: 25

// Calculate average
{ "/": [{ "+": [10, 20, 30] }, 3] }
// Result: 20
```

**Try it:**

<div class="playground-widget" data-logic='{"/": [{"+": [10, 20, 30]}, 3]}' data-data='{}'>
</div>

**Notes:**
- Division by zero behavior is configurable via `EvaluationConfig`
- Default returns `Infinity` or `-Infinity`

---

## % (Modulo)

Calculate remainder of division.

**Syntax:**
```json
{ "%": [a, b] }
```

**Arguments:**
- `a` - Dividend
- `b` - Divisor

**Returns:** Remainder after division.

**Examples:**

```json
// Basic modulo
{ "%": [10, 3] }
// Result: 1

{ "%": [10, 5] }
// Result: 0

// Negative numbers
{ "%": [-10, 3] }
// Result: -1

// Check if even
{ "==": [{ "%": [{ "var": "n" }, 2] }, 0] }
// Data: { "n": 4 }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"==": [{"%": [{"var":"n"}, 2]}, 0]}' data-data='{"n": 4}'>
</div>

---

## max

Find the maximum value.

**Syntax:**
```json
{ "max": [a, b, ...] }
{ "max": array }
```

**Arguments:**
- `a`, `b`, ... - Values to compare, or
- `array` - Single array of values

**Returns:** The largest value.

**Examples:**

```json
// Multiple arguments
{ "max": [1, 5, 3] }
// Result: 5

// Single array
{ "max": [[1, 5, 3]] }
// Result: 5

// With variables
{ "max": [{ "var": "scores" }] }
// Data: { "scores": [85, 92, 78] }
// Result: 92

// Empty array
{ "max": [[]] }
// Result: null
```

**Try it:**

<div class="playground-widget" data-logic='{"max": [{"var":"scores"}]}' data-data='{"scores": [85, 92, 78]}'>
</div>

---

## min

Find the minimum value.

**Syntax:**
```json
{ "min": [a, b, ...] }
{ "min": array }
```

**Arguments:**
- `a`, `b`, ... - Values to compare, or
- `array` - Single array of values

**Returns:** The smallest value.

**Examples:**

```json
// Multiple arguments
{ "min": [5, 1, 3] }
// Result: 1

// Single array
{ "min": [[5, 1, 3]] }
// Result: 1

// With variables
{ "min": [{ "var": "prices" }] }
// Data: { "prices": [29.99, 19.99, 39.99] }
// Result: 19.99

// Empty array
{ "min": [[]] }
// Result: null
```

**Try it:**

<div class="playground-widget" data-logic='{"min": [{"var":"prices"}]}' data-data='{"prices": [29.99, 19.99, 39.99]}'>
</div>

---

## abs

Get the absolute value.

**Syntax:**
```json
{ "abs": value }
```

**Arguments:**
- `value` - Number to get absolute value of

**Returns:** Absolute (positive) value.

**Examples:**

```json
{ "abs": -5 }
// Result: 5

{ "abs": 5 }
// Result: 5

{ "abs": -3.14 }
// Result: 3.14

{ "abs": 0 }
// Result: 0

// Distance between two points
{ "abs": { "-": [{ "var": "a" }, { "var": "b" }] } }
// Data: { "a": 3, "b": 10 }
// Result: 7
```

**Try it:**

<div class="playground-widget" data-logic='{"abs": {"-": [{"var":"a"}, {"var":"b"}]}}' data-data='{"a": 3, "b": 10}'>
</div>

---

## ceil

Round up to the nearest integer.

**Syntax:**
```json
{ "ceil": value }
```

**Arguments:**
- `value` - Number to round up

**Returns:** Smallest integer greater than or equal to value.

**Examples:**

```json
{ "ceil": 4.1 }
// Result: 5

{ "ceil": 4.9 }
// Result: 5

{ "ceil": 4.0 }
// Result: 4

{ "ceil": -4.1 }
// Result: -4

// Round up to whole units
{ "ceil": { "/": [{ "var": "items" }, 10] } }
// Data: { "items": 25 }
// Result: 3 (need 3 boxes of 10)
```

**Try it:**

<div class="playground-widget" data-logic='{"ceil": {"/": [{"var":"items"}, 10]}}' data-data='{"items": 25}'>
</div>

---

## floor

Round down to the nearest integer.

**Syntax:**
```json
{ "floor": value }
```

**Arguments:**
- `value` - Number to round down

**Returns:** Largest integer less than or equal to value.

**Examples:**

```json
{ "floor": 4.9 }
// Result: 4

{ "floor": 4.1 }
// Result: 4

{ "floor": 4.0 }
// Result: 4

{ "floor": -4.1 }
// Result: -5

// Truncate decimal
{ "floor": { "var": "amount" } }
// Data: { "amount": 99.99 }
// Result: 99
```

**Try it:**

<div class="playground-widget" data-logic='{"floor": {"var":"amount"}}' data-data='{"amount": 99.99}'>
</div>
