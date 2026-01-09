# Array Operators

Operations for working with arrays, including iteration and transformation.

## merge

Merge multiple arrays into one.

**Syntax:**
```json
{ "merge": [array1, array2, ...] }
```

**Arguments:**
- `array1`, `array2`, ... - Arrays to merge

**Returns:** Single flattened array.

**Examples:**

```json
// Merge two arrays
{ "merge": [[1, 2], [3, 4]] }
// Result: [1, 2, 3, 4]

// Merge multiple
{ "merge": [[1], [2], [3]] }
// Result: [1, 2, 3]

// Non-arrays are wrapped
{ "merge": [[1, 2], 3, [4, 5]] }
// Result: [1, 2, 3, 4, 5]

// With variables
{ "merge": [{ "var": "arr1" }, { "var": "arr2" }] }
// Data: { "arr1": [1, 2], "arr2": [3, 4] }
// Result: [1, 2, 3, 4]
```

**Try it:**

<div class="playground-widget" data-logic='{"merge": [{"var":"arr1"}, {"var":"arr2"}]}' data-data='{"arr1": [1, 2], "arr2": [3, 4]}'>
</div>

---

## filter

Filter array elements based on a condition.

**Syntax:**
```json
{ "filter": [array, condition] }
```

**Arguments:**
- `array` - Array to filter
- `condition` - Condition applied to each element (use `{"var": ""}` for current element)

**Returns:** Array of elements where condition is truthy.

**Examples:**

```json
// Filter numbers greater than 2
{ "filter": [
    [1, 2, 3, 4, 5],
    { ">": [{ "var": "" }, 2] }
]}
// Result: [3, 4, 5]

// Filter even numbers
{ "filter": [
    [1, 2, 3, 4, 5, 6],
    { "==": [{ "%": [{ "var": "" }, 2] }, 0] }
]}
// Result: [2, 4, 6]

// Filter objects by property
{ "filter": [
    { "var": "users" },
    { "==": [{ "var": "active" }, true] }
]}
// Data: {
//   "users": [
//     { "name": "Alice", "active": true },
//     { "name": "Bob", "active": false },
//     { "name": "Carol", "active": true }
//   ]
// }
// Result: [{ "name": "Alice", "active": true }, { "name": "Carol", "active": true }]

// Filter with multiple conditions
{ "filter": [
    { "var": "products" },
    { "and": [
        { ">": [{ "var": "price" }, 10] },
        { "var": "inStock" }
    ]}
]}
```

**Try it:**

<div class="playground-widget" data-logic='{"filter": [[1, 2, 3, 4, 5], {">": [{"var": ""}, 2]}]}' data-data='{}'>
</div>

**Notes:**
- Inside the condition, `{"var": ""}` refers to the current element
- The original array is not modified

---

## map

Transform each element of an array.

**Syntax:**
```json
{ "map": [array, transformation] }
```

**Arguments:**
- `array` - Array to transform
- `transformation` - Operation applied to each element

**Returns:** Array of transformed elements.

**Examples:**

```json
// Double each number
{ "map": [
    [1, 2, 3],
    { "*": [{ "var": "" }, 2] }
]}
// Result: [2, 4, 6]

// Extract property from objects
{ "map": [
    { "var": "users" },
    { "var": "name" }
]}
// Data: {
//   "users": [
//     { "name": "Alice", "age": 30 },
//     { "name": "Bob", "age": 25 }
//   ]
// }
// Result: ["Alice", "Bob"]

// Create new objects
{ "map": [
    { "var": "items" },
    { "cat": ["Item: ", { "var": "name" }] }
]}
// Data: { "items": [{ "name": "A" }, { "name": "B" }] }
// Result: ["Item: A", "Item: B"]

// Square numbers
{ "map": [
    [1, 2, 3, 4],
    { "*": [{ "var": "" }, { "var": "" }] }
]}
// Result: [1, 4, 9, 16]
```

**Try it:**

<div class="playground-widget" data-logic='{"map": [[1, 2, 3], {"*": [{"var": ""}, 2]}]}' data-data='{}'>
</div>

---

## reduce

Reduce an array to a single value.

**Syntax:**
```json
{ "reduce": [array, reducer, initial] }
```

**Arguments:**
- `array` - Array to reduce
- `reducer` - Operation combining accumulator and current element
- `initial` - Initial value for accumulator

**Returns:** Final accumulated value.

**Context Variables:**
- `{"var": "current"}` - Current element
- `{"var": "accumulator"}` - Current accumulated value

**Examples:**

```json
// Sum all numbers
{ "reduce": [
    [1, 2, 3, 4, 5],
    { "+": [{ "var": "accumulator" }, { "var": "current" }] },
    0
]}
// Result: 15

// Product of all numbers
{ "reduce": [
    [1, 2, 3, 4],
    { "*": [{ "var": "accumulator" }, { "var": "current" }] },
    1
]}
// Result: 24

// Concatenate strings
{ "reduce": [
    ["a", "b", "c"],
    { "cat": [{ "var": "accumulator" }, { "var": "current" }] },
    ""
]}
// Result: "abc"

// Find maximum
{ "reduce": [
    [3, 1, 4, 1, 5, 9],
    { "if": [
        { ">": [{ "var": "current" }, { "var": "accumulator" }] },
        { "var": "current" },
        { "var": "accumulator" }
    ]},
    0
]}
// Result: 9

// Count elements matching condition
{ "reduce": [
    [1, 2, 3, 4, 5, 6],
    { "+": [
        { "var": "accumulator" },
        { "if": [{ ">": [{ "var": "current" }, 3] }, 1, 0] }
    ]},
    0
]}
// Result: 3 (count of numbers > 3)
```

**Try it:**

<div class="playground-widget" data-logic='{"reduce": [[1, 2, 3, 4, 5], {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}' data-data='{}'>
</div>

---

## all

Check if all elements satisfy a condition.

**Syntax:**
```json
{ "all": [array, condition] }
```

**Arguments:**
- `array` - Array to check
- `condition` - Condition applied to each element

**Returns:** `true` if all elements satisfy condition, `false` otherwise.

**Examples:**

```json
// All positive
{ "all": [
    [1, 2, 3],
    { ">": [{ "var": "" }, 0] }
]}
// Result: true

// All greater than 5
{ "all": [
    [1, 2, 3],
    { ">": [{ "var": "" }, 5] }
]}
// Result: false

// All users active
{ "all": [
    { "var": "users" },
    { "var": "active" }
]}
// Data: { "users": [{ "active": true }, { "active": true }] }
// Result: true

// Empty array returns true (vacuous truth)
{ "all": [[], { ">": [{ "var": "" }, 0] }] }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"all": [[1, 2, 3], {">": [{"var": ""}, 0]}]}' data-data='{}'>
</div>

---

## some

Check if any element satisfies a condition.

**Syntax:**
```json
{ "some": [array, condition] }
```

**Arguments:**
- `array` - Array to check
- `condition` - Condition applied to each element

**Returns:** `true` if at least one element satisfies condition, `false` otherwise.

**Examples:**

```json
// Any negative
{ "some": [
    [1, -2, 3],
    { "<": [{ "var": "" }, 0] }
]}
// Result: true

// Any greater than 10
{ "some": [
    [1, 2, 3],
    { ">": [{ "var": "" }, 10] }
]}
// Result: false

// Any admin user
{ "some": [
    { "var": "users" },
    { "==": [{ "var": "role" }, "admin"] }
]}
// Data: {
//   "users": [
//     { "role": "user" },
//     { "role": "admin" }
//   ]
// }
// Result: true

// Empty array returns false
{ "some": [[], { ">": [{ "var": "" }, 0] }] }
// Result: false
```

**Try it:**

<div class="playground-widget" data-logic='{"some": [[1, -2, 3], {"<": [{"var": ""}, 0]}]}' data-data='{}'>
</div>

---

## none

Check if no elements satisfy a condition.

**Syntax:**
```json
{ "none": [array, condition] }
```

**Arguments:**
- `array` - Array to check
- `condition` - Condition applied to each element

**Returns:** `true` if no elements satisfy condition, `false` otherwise.

**Examples:**

```json
// None negative
{ "none": [
    [1, 2, 3],
    { "<": [{ "var": "" }, 0] }
]}
// Result: true

// None greater than 0
{ "none": [
    [1, 2, 3],
    { ">": [{ "var": "" }, 0] }
]}
// Result: false

// No banned users
{ "none": [
    { "var": "users" },
    { "var": "banned" }
]}
// Data: { "users": [{ "banned": false }, { "banned": false }] }
// Result: true

// Empty array returns true
{ "none": [[], { ">": [{ "var": "" }, 0] }] }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"none": [[1, 2, 3], {"<": [{"var": ""}, 0]}]}' data-data='{}'>
</div>

---

## sort

Sort an array.

**Syntax:**
```json
{ "sort": array }
{ "sort": [array] }
{ "sort": [array, comparator] }
```

**Arguments:**
- `array` - Array to sort
- `comparator` - Optional comparison logic

**Returns:** Sorted array.

**Examples:**

```json
// Sort numbers
{ "sort": [[3, 1, 4, 1, 5, 9]] }
// Result: [1, 1, 3, 4, 5, 9]

// Sort strings
{ "sort": [["banana", "apple", "cherry"]] }
// Result: ["apple", "banana", "cherry"]

// Sort with custom comparator
{ "sort": [
    { "var": "items" },
    { "-": [{ "var": "a.price" }, { "var": "b.price" }] }
]}
// Data: {
//   "items": [
//     { "name": "B", "price": 20 },
//     { "name": "A", "price": 10 }
//   ]
// }
// Result: [{ "name": "A", "price": 10 }, { "name": "B", "price": 20 }]
```

**Try it:**

<div class="playground-widget" data-logic='{"sort": [[3, 1, 4, 1, 5, 9]]}' data-data='{}'>
</div>

---

## slice

Extract a portion of an array.

**Syntax:**
```json
{ "slice": [array, start] }
{ "slice": [array, start, end] }
```

**Arguments:**
- `array` - Source array
- `start` - Starting index (negative counts from end)
- `end` - Ending index, exclusive (optional, negative counts from end)

**Returns:** Array slice.

**Examples:**

```json
// From index 2 to end
{ "slice": [[1, 2, 3, 4, 5], 2] }
// Result: [3, 4, 5]

// From index 1 to 3
{ "slice": [[1, 2, 3, 4, 5], 1, 3] }
// Result: [2, 3]

// Last 2 elements
{ "slice": [[1, 2, 3, 4, 5], -2] }
// Result: [4, 5]

// First 3 elements
{ "slice": [[1, 2, 3, 4, 5], 0, 3] }
// Result: [1, 2, 3]

// Pagination
{ "slice": [
    { "var": "items" },
    { "*": [{ "var": "page" }, 10] },
    { "+": [{ "*": [{ "var": "page" }, 10] }, 10] }
]}
// Data: { "items": [...], "page": 0 }
// Result: first 10 items
```

**Try it:**

<div class="playground-widget" data-logic='{"slice": [[1, 2, 3, 4, 5], 1, 3]}' data-data='{}'>
</div>
