# Array Operators

Operations for working with arrays, including iteration and transformation.

> **Feature flags (Rust crate).** All array operators are baseline except `sort`, `slice`, `group_by`, and `distinct`, which require the `ext-array` feature. Every language binding enables all operator features. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## merge

Concatenate multiple arrays into one.

**Syntax:**
```json
{ "merge": [array1, array2, ...] }
```

**Arguments:**
- `array1`, `array2`, ... - Arrays to concatenate; scalars and objects are wrapped as single elements

**Returns:** A single array: the arguments concatenated one level deep. Nested arrays inside an argument are kept as elements (no deep flatten), and `null` arguments and `null` elements are dropped.

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

// Only one level is flattened
{ "merge": [[1, [2, [3]]], [4]] }
// Result: [1, [2, [3]], 4]

// Nulls are dropped
{ "merge": [[1, null, 2], null] }
// Result: [1, 2]
```

**Try it:**

<div class="playground-widget" data-logic='{"merge": [{"var":"arr1"}, {"var":"arr2"}]}' data-data='{"arr1": [1, 2], "arr2": [3, 4]}'>
</div>

**Notes:**
- Flattens exactly one level; use nested `merge` calls for deeper structures
- `null` arguments and `null` elements are skipped; a scalar argument is wrapped (`{ "merge": 1 }` is `[1]`)

---

## filter

Filter array elements based on a condition.

**Syntax:**
```json
{ "filter": [array, condition] }
```

**Arguments:**
- `array` - Array to filter (an object is also accepted, see Notes)
- `condition` - Condition applied to each element (use `{"var": ""}` for current element)

**Returns:** Array of elements where condition is truthy. For an object input, an object of the key/value pairs whose value passes.

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
- An object input is filtered by its values and returns an object of the kept pairs: `{ "filter": [{ "var": "x" }, { ">": [{ "var": "" }, 3] }] }` with `{ "x": { "a": 1, "b": 5 } }` is `{ "b": 5 }`
- A `null` or missing input yields `[]`; a scalar input is an Invalid Arguments error

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

**Notes:**
- An object input maps over its values and returns an array (`{ "a": 1, "b": 5 }` doubled is `[2, 10]`)
- A `null` or missing input yields `[]`; a scalar input is treated as a one-element array (`{ "map": [5, { "*": [{ "var": "" }, 2] }] }` is `[10]`)

---

## reduce

Reduce an array to a single value.

**Syntax:**
```json
{ "reduce": [array, reducer, initial] }
{ "reduce": [array, reducer] }
```

**Arguments:**
- `array` - Array to reduce
- `reducer` - Operation combining accumulator and current element
- `initial` - Initial value for accumulator (optional). When omitted, the first element seeds the accumulator and reduction starts from the second; an empty array without `initial` yields `null`

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

// Without initial: the first element seeds the accumulator
{ "reduce": [
    [1, 2, 3],
    { "+": [{ "var": "accumulator" }, { "var": "current" }] }
]}
// Result: 6

// Empty array without initial
{ "reduce": [
    [],
    { "+": [{ "var": "accumulator" }, { "var": "current" }] }
]}
// Result: null
```

**Try it:**

<div class="playground-widget" data-logic='{"reduce": [[1, 2, 3, 4, 5], {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}' data-data='{}'>
</div>

**Notes:**
- An object input reduces over its values
- A `null`, missing, or scalar input returns `initial` unchanged (`null` when `initial` is omitted)

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

// Empty array returns false (in this engine, all-of-empty is false)
{ "all": [[], { ">": [{ "var": "" }, 0] }] }
// Result: false
```

**Try it:**

<div class="playground-widget" data-logic='{"all": [[1, 2, 3], {">": [{"var": ""}, 0]}]}' data-data='{}'>
</div>

**Notes (all, some, none):**
- An object input checks the object's values
- A `null`, missing, or scalar input is treated as an empty collection: `all` is `false`, `some` is `false`, `none` is `true`

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
{ "sort": [array] }
{ "sort": [array, ascending] }
{ "sort": [array, ascending, key_extractor] }
```

**Arguments:**
- `array` - Array to sort (a value that resolves to an array)
- `ascending` - Optional direction boolean: `true` (or omitted) sorts ascending, `false` sorts descending
- `key_extractor` - Optional per-element expression that produces the sort key for each element

**Returns:** Sorted array.

**Examples:**

```json
// Sort numbers (ascending by default)
{ "sort": [[3, 1, 4, 1, 5, 9]] }
// Result: [1, 1, 3, 4, 5, 9]

// Sort strings
{ "sort": [["banana", "apple", "cherry"]] }
// Result: ["apple", "banana", "cherry"]

// Sort descending
{ "sort": [{ "var": "nums" }, false] }
// Data: { "nums": [3, 1, 4, 1, 5, 9] }
// Result: [9, 5, 4, 3, 1, 1]

// Sort objects ascending by a key extractor
{ "sort": [
    { "var": "items" },
    true,
    { "var": "price" }
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

**Notes:**
- The second argument is a direction boolean, not a comparator: `true` (or omitted) sorts ascending, `false` descending. A non-boolean direction falls back to ascending.
- The optional third argument is a per-element key extractor (evaluated with each element as its context), not an `a`/`b` binary comparator. There is no `a`/`b` comparator form.
- A `null` or missing input yields `null` (a literal `null` argument is Invalid Arguments). This differs from `group_by` and `distinct`, which yield `[]` for `null` input.

---

## slice

Extract a portion of an array or string.

**Syntax:**
```json
{ "slice": [collection, start] }
{ "slice": [collection, start, end] }
{ "slice": [collection, start, end, step] }
```

**Arguments:**
- `collection` - Source array, or a string (sliced by character)
- `start` - Starting index (negative counts from end); `null` means the default (the first element)
- `end` - Ending index, exclusive (optional, negative counts from end); `null` means the default (the end of the collection)
- `step` - Optional stride (default `1`); a negative step walks backwards, so `[null, null, -1]` reverses

**Returns:** Array slice, or a string slice for string input.

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

// Step: every second element
{ "slice": [[1, 2, 3, 4, 5], 0, 5, 2] }
// Result: [1, 3, 5]

// Negative step reverses
{ "slice": [[1, 2, 3, 4, 5], null, null, -1] }
// Result: [5, 4, 3, 2, 1]

// Strings are sliced by character
{ "slice": ["hello", 1, 3] }
// Result: "el"

{ "slice": ["hello", null, null, -1] }
// Result: "olleh"
```

**Try it:**

<div class="playground-widget" data-logic='{"slice": [[1, 2, 3, 4, 5], 1, 3]}' data-data='{}'>
</div>

**Notes:**
- A `null` or missing collection yields `null`
- `step` of `0` is an Invalid Arguments error; a non-numeric index (`{ "slice": [[1, 2, 3], "1"] }`) throws `NaN`

---

## group_by

Collapse an array into groups on a computed key.

**Syntax:**
```json
{ "group_by": [array, key_expression] }
```

**Arguments:**
- `array` - Array to group (a value that resolves to an array)
- `key_expression` - Per-element expression that produces each element's group key (evaluated with the element as its context, like `sort`'s key extractor)

**Returns:** Array of `{"key": ..., "items": [...]}` rows, an *array* of groups rather than an object, so the result composes directly with `map`, `filter`, and `sort`. Groups appear in order of first key occurrence, so output is deterministic for a given input.

**Examples:**

```json
// Group tasks by status
{ "group_by": [{ "var": "tasks" }, { "var": "status" }] }
// Data: { "tasks": [
//   { "id": 1, "status": "open" },
//   { "id": 2, "status": "done" },
//   { "id": 3, "status": "open" }
// ]}
// Result: [
//   { "key": "open", "items": [{ "id": 1, "status": "open" }, { "id": 3, "status": "open" }] },
//   { "key": "done", "items": [{ "id": 2, "status": "done" }] }
// ]

// Group meetings by calendar date in the user's timezone
{ "group_by": [
    { "var": "meetings" },
    { "format_date": [{ "var": "start_time" }, "dd MMM yyyy", "Asia/Kolkata"] }
]}
// Result: [ { "key": "17 Aug 2026", "items": [ ... ] }, ... ]

// Group numbers by a computed key
{ "group_by": [[1, 2, 3, 4, 5], { "%": [{ "var": "" }, 2] }] }
// Result: [
//   { "key": 1, "items": [1, 3, 5] },
//   { "key": 0, "items": [2, 4] }
// ]
```

**Try it:**

<div class="playground-widget" data-logic='{"group_by": [{"var": "tasks"}, {"var": "status"}]}' data-data='{"tasks": [{"id": 1, "status": "open"}, {"id": 2, "status": "done"}, {"id": 3, "status": "open"}]}'>
</div>

**Notes:**
- Keys are kept as their evaluated values: numbers, booleans, `null`, and even objects group correctly by deep equality; they are not stringified.
- Elements whose key expression misses (resolves to `null`) group together under a `null` key.
- `null` or empty input yields `[]`. Non-array input (scalar or object) is an error.

---

## distinct

Drop duplicate elements, by value or by a computed key.

**Syntax:**
```json
{ "distinct": [array] }
{ "distinct": [array, key_expression] }
```

**Arguments:**
- `array` - Array to deduplicate (a value that resolves to an array)
- `key_expression` - Optional per-element expression; when present, elements are deduplicated by the computed key instead of by value

**Returns:** Array with duplicates removed. The first occurrence wins, so output preserves input order.

**Examples:**

```json
// Dedup by value
{ "distinct": [[3, 1, 3, 2, 1]] }
// Result: [3, 1, 2]

// Strict equality: 1 and "1" stay distinct
{ "distinct": [[1, "1", 1]] }
// Result: [1, "1"]

// Dedup objects structurally
{ "distinct": [{ "var": "items" }] }
// Data: { "items": [{ "a": 1 }, { "a": 2 }, { "a": 1 }] }
// Result: [{ "a": 1 }, { "a": 2 }]

// Dedup by key: one row per id, first revision wins
{ "distinct": [{ "var": "rows" }, { "var": "id" }] }
// Data: { "rows": [
//   { "id": 1, "rev": "a" },
//   { "id": 2, "rev": "b" },
//   { "id": 1, "rev": "c" }
// ]}
// Result: [{ "id": 1, "rev": "a" }, { "id": 2, "rev": "b" }]
```

**Try it:**

<div class="playground-widget" data-logic='{"distinct": [[3, 1, 3, 2, 1]]}' data-data='{}'>
</div>

**Notes:**
- Equality is strict deep equality (the same predicate `in` uses): mixed types never merge, arrays and objects compare structurally.
- `null` or empty input yields `[]`. Non-array input is an error.
