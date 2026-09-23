# Arithmetic Operators

Mathematical operations. `+`, `-`, `*`, `/`, `%`, `abs`, `ceil`, and `floor`
coerce numeric strings to numbers; `max` and `min` accept numbers only.

> **Feature flags (Rust crate).** `+`, `-`, `*`, `/`, `%`, `min`, and `max` are baseline; `abs`, `ceil`, and `floor` require the `ext-math` feature. Every language binding enables all operator features. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## + (Add)

Add numbers together (numeric strings are coerced).

**Syntax:**
```json
{ "+": [a, b, ...] }
{ "+": value }
```

**Arguments:**
- `a`, `b`, ... - Values to add (variadic)
- Single value is cast to number

**Returns:** Sum of all arguments.

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
- `+` converts numeric strings to numbers
- `+` never concatenates: a non-numeric string throws a `NaN` error under the default `EvaluationConfig::arithmetic_nan_handling`, so `{ "+": ["hello", " world"] }` is an error. Use `cat` to join strings
- Single argument converts value to number
- `{ "+": [] }` returns `0`

---

## - (Subtract)

Subtract numbers.

**Syntax:**
```json
{ "-": [a, b] }
{ "-": [a, b, c, ...] }
{ "-": value }
```

**Arguments:**
- `a` - Value to subtract from
- `b`, `c`, ... - Values to subtract, folded left to right (`a - b - c ...`)
- Single value negates it

**Returns:** Difference, or negated value. An empty argument list is an Invalid Arguments error.

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

// Variadic (left fold)
{ "-": [10, 3, 2] }
// Result: 5

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
{ "/": [a, b, c, ...] }
{ "/": value }
```

**Arguments:**
- `a` - Dividend
- `b`, `c`, ... - Divisors, folded left to right (`a / b / c ...`)
- Single value returns its reciprocal (`1 / value`)

**Returns:** Quotient, or the reciprocal for the single-value form.

**Examples:**

```json
// Basic division
{ "/": [10, 2] }
// Result: 5

// Decimal result
{ "/": [7, 2] }
// Result: 3.5

// Division by zero with two integral-valued operands throws an error (error type "NaN")
{ "/": [10, 0] }
// Result: error

// Variadic (left fold)
{ "/": [100, 2, 5] }
// Result: 10

// Unary form: reciprocal
{ "/": 5 }
// Result: 0.2

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
- If both operands are integral-valued numbers and the divisor is zero (`{ "/": [10, 0] }`, and also `{ "/": [10, 0.0] }`, since `0.0` is integral-valued), the engine always throws an error (error type "NaN"), regardless of config
- Otherwise a zero divisor follows `EvaluationConfig::division_by_zero`: a non-integral dividend (`{ "/": [10.5, 0] }`) or a divisor coerced from `null`, a boolean, or a string (`{ "/": [10, null] }`, `{ "/": [10, "0"] }`). The default is `DivisionByZeroHandling::ReturnSaturated`, which returns `f64::MAX` (or `f64::MIN` for a negative dividend), not `Infinity`
- You can select the other modes (`ReturnInfinity`, `ReturnNull`, `ThrowError`) through `EvaluationConfig`. Choose `ThrowError` if a `try` fallback should cover every zero-like divisor

---

## % (Modulo)

Calculate remainder of division.

**Syntax:**
```json
{ "%": [a, b] }
{ "%": [a, b, c, ...] }
```

**Arguments:**
- `a` - Dividend
- `b`, `c`, ... - Divisors, folded left to right (`(a % b) % c ...`)

**Returns:** Remainder after division. A single operand is an Invalid Arguments error.

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

**Notes:**
- A zero divisor follows the same rule as `/`: when both operands are integral-valued numbers (`{ "%": [10, 0] }`) the engine throws an error (error type "NaN"); otherwise (`{ "%": [10.5, 0] }`, `{ "%": [10, null] }`) the result follows `EvaluationConfig::division_by_zero`, default `ReturnSaturated`

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
- `array` - A single value (such as a `var`) that resolves to an array. It must be a resolved array, not a literal array written inline

**Returns:** The largest value.

**Examples:**

```json
// Multiple arguments
{ "max": [1, 5, 3] }
// Result: 5

// A single argument that resolves to an array (data-driven)
{ "max": { "var": "scores" } }
// Data: { "scores": [85, 92, 78] }
// Result: 92

// Note: a literal nested array passed positionally, e.g. { "max": [[1, 5, 3]] }
// or { "max": [[]] }, is an invalid operand and throws Invalid Arguments.
// Pass scalars directly, or a value that resolves to an array.
```

**Try it:**

<div class="playground-widget" data-logic='{"max": [{"var":"scores"}]}' data-data='{"scores": [85, 92, 78]}'>
</div>

**Notes:**
- Operands must be numbers. `max` does not coerce strings (even numeric ones such as `"1"`), booleans, or `null`; they throw Invalid Arguments. This differs from json-logic-js, which coerces `{ "max": ["1", 5, "3"] }` to `5`
- An empty argument list, or a single value that resolves to an empty array, throws Invalid Arguments

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
- `array` - A single value (such as a `var`) that resolves to an array. It must be a resolved array, not a literal array written inline

**Returns:** The smallest value.

**Examples:**

```json
// Multiple arguments
{ "min": [5, 1, 3] }
// Result: 1

// A single argument that resolves to an array (data-driven)
{ "min": { "var": "prices" } }
// Data: { "prices": [29.99, 19.99, 39.99] }
// Result: 19.99

// Note: a literal nested array passed positionally, e.g. { "min": [[5, 1, 3]] }
// or { "min": [[]] }, is an invalid operand and throws Invalid Arguments.
// Pass scalars directly, or a value that resolves to an array.
```

**Try it:**

<div class="playground-widget" data-logic='{"min": [{"var":"prices"}]}' data-data='{"prices": [29.99, 19.99, 39.99]}'>
</div>

**Notes:**
- Operands must be numbers. `min` does not coerce strings (even numeric ones), booleans, or `null`; they throw Invalid Arguments
- An empty argument list, or a single value that resolves to an empty array, throws Invalid Arguments

---

## abs

Get the absolute value.

**Syntax:**
```json
{ "abs": value }
{ "abs": [a, b, ...] }
```

**Arguments:**
- `value` - Number to get absolute value of (a numeric string is coerced)
- `a`, `b`, ... - Two or more numbers; the result is an array of per-element absolute values

**Returns:** Absolute (positive) value, or an array of them for the multi-argument form.

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

// Numeric strings are coerced
{ "abs": "-5" }
// Result: 5

// Multiple arguments: element-wise
{ "abs": [-1, 2] }
// Result: [1, 2]
```

**Try it:**

<div class="playground-widget" data-logic='{"abs": {"-": [{"var":"a"}, {"var":"b"}]}}' data-data='{"a": 3, "b": 10}'>
</div>

**Notes (abs, ceil, floor):**
- Numeric strings are coerced; `null`, booleans, and non-numeric strings throw Invalid Arguments
- Two or more arguments return an array of per-element results
- Unlike `max`/`min`, a single value that resolves to an array throws Invalid Arguments: `{ "abs": { "var": "a" } }` with `{ "a": [-1] }` is an error. Use `map` to apply these to a data-driven array

---

## ceil

Round up to the nearest integer.

**Syntax:**
```json
{ "ceil": value }
{ "ceil": [a, b, ...] }
```

**Arguments:**
- `value` - Number to round up (a numeric string is coerced)
- `a`, `b`, ... - Two or more numbers; the result is an array of per-element results

**Returns:** Smallest integer greater than or equal to value, or an array of them for the multi-argument form. See the notes under `abs` for argument rules.

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

// Multiple arguments: element-wise
{ "ceil": [1.2, 2] }
// Result: [2, 2]
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
{ "floor": [a, b, ...] }
```

**Arguments:**
- `value` - Number to round down (a numeric string is coerced)
- `a`, `b`, ... - Two or more numbers; the result is an array of per-element results

**Returns:** Largest integer less than or equal to value, or an array of them for the multi-argument form. See the notes under `abs` for argument rules.

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

// Multiple arguments: element-wise
{ "floor": [1.2, 2] }
// Result: [1, 2]
```

**Try it:**

<div class="playground-widget" data-logic='{"floor": {"var":"amount"}}' data-data='{"amount": 99.99}'>
</div>
