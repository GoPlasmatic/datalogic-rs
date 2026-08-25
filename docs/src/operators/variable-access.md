# Variable Access Operators

These operators access data from the evaluation context.

> **Feature flags (Rust crate).** `var` and `val` are baseline; `exists` requires the `ext-control` feature. Every language binding enables all operator features. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## var

Access a value from the data object using dot notation.

**Syntax:**
```json
{ "var": "path" }
{ "var": ["path", default] }
```

**Arguments:**
- `path` - Dot-separated path to the value (string)
- `default` - Optional default value if path doesn't exist

**Returns:** The value at the path, or the default value, or `null`.

**Examples:**

```json
// Simple access
{ "var": "name" }
// Data: { "name": "Alice" }
// Result: "Alice"

// Nested access
{ "var": "user.address.city" }
// Data: { "user": { "address": { "city": "NYC" } } }
// Result: "NYC"

// Array index access
{ "var": "items.0" }
// Data: { "items": ["a", "b", "c"] }
// Result: "a"

// Default value
{ "var": ["missing", "default"] }
// Data: {}
// Result: "default"

// Access entire data object
{ "var": "" }
// Data: { "x": 1, "y": 2 }
// Result: { "x": 1, "y": 2 }
```

**Try it:**

<div class="playground-widget" data-logic='{"var": "user.address.city"}' data-data='{"user": {"address": {"city": "NYC"}}}'>
</div>

**Notes:**
- Empty string `""` returns the entire data context
- In array operations (`map`, `filter`, `reduce`), `""` refers to the current element
- Numeric segments index arrays (`items.0`); strings are not indexable, so `{ "var": "s.1" }` on `{ "s": "hello" }` is `null`. Use `substr` to read characters
- Returns `null` if path doesn't exist and no default is provided

---

## val

Alternative variable access with explicit path segments and scope levels.

**Syntax:**
```json
{ "val": "key" }
{ "val": ["segment1", "segment2", ...] }
{ "val": [[N], "segment1", ...] }
{ "val": [] }
```

**Arguments:**
- `key` - A single literal key (string)
- `["segment1", "segment2", ...]` - Path segments walked in order. Every array element is one segment (a string key or a numeric array index); there is no default-value slot
- `[[N], ...]` - An optional leading scope level for iteration frames (see [Scope levels](#scope-levels) below)
- `[]` - No segments: returns the current context (the whole data object, or the current element inside an iterator)

**Returns:** The value at the path, or `null` if any segment is missing. `val` has no default argument: use `var`'s second argument or `??` for a fallback.

**Examples:**

```json
// Simple access (same as var)
{ "val": "name" }
// Data: { "name": "Bob" }
// Result: "Bob"

// Nested access (use the array form; a dot string is NOT split)
{ "val": ["config", "settings", "enabled"] }
// Data: { "config": { "settings": { "enabled": true } } }
// Result: true

// A dot-path string is treated as a single literal key, so it does NOT navigate
{ "val": "config.settings.enabled" }
// Data: { "config": { "settings": { "enabled": true } } }
// Result: null (looks up the key "config.settings.enabled", which is absent)

// Numeric segments index arrays
{ "val": ["items", 0] }
// Data: { "items": ["a", "b"] }
// Result: "a"

// No default slot: a second element is just another path segment
{ "val": ["missing", "default"] }
// Data: {}
// Result: null (looks up data.missing.default)

// Fallbacks use ?? (or var's default form) instead
{ "??": [{ "val": ["missing", "key"] }, "default"] }
// Data: {}
// Result: "default"
```

**Try it:**

<div class="playground-widget" data-logic='{"val": ["config", "settings", "enabled"]}' data-data='{"config": {"settings": {"enabled": true}}}'>
</div>

**Notes:**
- `val` does NOT support `var`'s dot-path strings: a string argument is a single
  literal key, so `{ "val": "a.b" }` looks up the key `"a.b"`, it does not descend
  into `a` then `b`
- For nested access use the array form `{ "val": ["a", "b"] }`, where each element
  is one path segment
- There is no `[path, default]` form: `{ "val": ["a", "b"] }` always walks `a`
  then `b`. Use `{ "var": ["a", "default"] }` or `{ "??": [{ "val": "a" }, "default"] }`
  for a fallback
- Useful for complex data navigation where path segments are computed

### Scope levels

Inside an iterator (`map`, `filter`, `all`, `some`, `none`, `group_by`, and
the per-element expressions of `sort`, `distinct`, and `reduce`) the current
element is the data context, so `{ "val": "field" }` reads the element. A
leading `[N]` array selects a different frame. This is the only way to reach
iteration metadata (the element's index or key).

**Syntax:**
```json
{ "val": [] }
{ "val": [[1], "index"] }
{ "val": [[1], "key"] }
{ "val": [[N], "segment1", "segment2", ...] }
```

- `{ "val": [] }` returns the current element itself (same as `{ "var": "" }`).
- `{ "val": [[1], "index"] }` returns the current element's zero-based position
  while iterating an array (or an object, in stored order); `{ "val": [[1], "key"] }`
  returns its key while iterating an object. Both are `null` where no such
  metadata exists: `key` over an array, and both inside `reduce`.
- `{ "val": [[N], "field", ...] }` with `N` at or above the number of enclosing
  iterators resolves against the root data. The conformance-suite convention is
  `[[2], ...]` from inside one iterator; inside a single iterator `[[1], "field"]`
  reaches the root as well, because there is only one frame to leave.
- Inside nested iterators the enclosing iterator's element is not addressable in
  the current engine: `[[1], "field"]` reads the current (innermost) element and
  every higher level reads the root. Bind the outer value in your data, or
  restructure the rule, when an inner iterator needs it.
- The level form needs a segment after the level. `{ "val": [[1]] }` on its own
  is the ordinary path form and looks up the key `"1"`.
- Relative path syntax such as `"../field"` is not supported; it is treated as a
  literal key and resolves to `null`.
- `var` accepts the same `[[N], ...]` form, because it compiles to `val`.

**Examples:**

```json
// Pair each element with its index
{ "map": [{ "var": "items" }, { "cat": [{ "val": [[1], "index"] }, ":", { "var": "" }] }] }
// Data: { "items": ["a", "b"] }
// Result: ["0:a", "1:b"]

// Keep every element except the first
{ "filter": [[10, 20, 30], { ">": [{ "val": [[1], "index"] }, 0] }] }
// Result: [20, 30]

// Object iteration exposes the key
{ "map": [{ "var": "scores" }, { "val": [[1], "key"] }] }
// Data: { "scores": { "alice": 10, "bob": 7 } }
// Result: ["alice", "bob"]

// Compare each element against a root-level value
{ "filter": [
    { "var": "people" },
    { "==": [{ "val": "department" }, { "val": [[2], "department"] }] }
]}
// Data: {
//   "department": "Engineering",
//   "people": [
//     { "name": "Jay", "department": "Engineering" },
//     { "name": "Louisa", "department": "Sales" }
//   ]
// }
// Result: [{ "name": "Jay", "department": "Engineering" }]

// Add a root value to each element
{ "map": [{ "var": "numbers" }, { "+": [{ "val": [[2], "offset"] }, { "val": [] }] }] }
// Data: { "numbers": [1, 2, 3], "offset": 10 }
// Result: [11, 12, 13]
```

---

## exists

Check if a variable path exists in the data.

**Syntax:**
```json
{ "exists": "key" }
{ "exists": ["key1", "key2", ...] }
{ "exists": { "var": "path" } }
```

**Arguments:**
- `key` - A single top-level key (string), or
- `["key1", "key2", ...]` - An array of object keys walked in order for nested access, or
- A `var` operation that resolves to the key/path to check

**Returns:** `true` if the path exists, `false` otherwise.

**Examples:**

```json
// Check if key exists
{ "exists": "name" }
// Data: { "name": "Alice" }
// Result: true

// Check missing key
{ "exists": "age" }
// Data: { "name": "Alice" }
// Result: false

// Check nested path (use the array form; a dot string is one literal key)
{ "exists": ["user", "profile"] }
// Data: { "user": { "profile": { "name": "Bob" } } }
// Result: true

// A dot-path string checks a single literal key, so it does not descend
{ "exists": "user.profile" }
// Data: { "user": { "profile": { "name": "Bob" } } }
// Result: false (no top-level key named "user.profile")

// Check with var
{ "exists": { "var": "fieldName" } }
// Data: { "fieldName": "name", "name": "Alice" }
// Result: true (checks if "name" exists)
```

**Try it:**

<div class="playground-widget" data-logic='{"exists": "name"}' data-data='{"name": "Alice"}'>
</div>

**Notes:**
- Returns `false` for paths that don't exist
- Does not check if the value is null/empty, only if the path exists
- Segments must be object keys: `exists` does not index arrays, so
  `{ "exists": ["items", 0] }` is `false` even though `{ "val": ["items", 0] }`
  resolves. Test array elements with `{ "!=": [{ "val": ["items", 0] }, null] }`
  or `length` instead
- An empty segment list, `{ "exists": [] }`, returns `true`
- Useful for conditional logic based on data structure
