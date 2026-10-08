# Variable Access Operators

These operators access data from the evaluation context.

> **Feature flags (Rust crate).** `var` and `val` are baseline; `exists` requires the `ext-control` feature. Every language binding enables all operator features. An engine built with [`EngineBuilder::with_families`](../advanced/configuration.md#operator-families) has `exists` only if it names `Family::ExtControl`. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## var

Access a value from the data object using dot notation.

**Syntax:**
```json
{ "var": "path" }
{ "var": ["path", default] }
```

**Arguments:**
- `path` - Dot-separated path to the value (a string, a number, or an expression that produces one)
- `default` - Optional value returned when the path does not exist

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
- A literal path always splits on dots, so `{ "var": "a.b" }` cannot read a key named `"a.b"`; use `{ "val": "a.b" }` for that. A path computed at runtime (`{ "var": { "cat": ["a", ".b"] } }`) is looked up as a key first and walked as a dotted path only if no such key exists
- The default applies only when the path is absent: a present `null` stays `null`
- Returns `null` if the path doesn't exist and no default is provided, or raises an error under `MissingVar::Error` (see [Missing paths](#missing-paths))

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

// No default slot: a second element is another path segment
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
- A literal string argument is a single key, so `{ "val": "a.b" }` looks up the
  key `"a.b"`; it does not descend into `a` then `b`
- For nested access use the array form `{ "val": ["a", "b"] }`, where each element
  is one path segment
- There is no `[path, default]` form: `{ "val": ["a", "b"] }` always walks `a`
  then `b`. Use `{ "var": ["a", "default"] }` or `{ "??": [{ "val": "a" }, "default"] }`
  for a fallback. Under `MissingVar::Error` the `??` form raises; see
  [Missing paths](#missing-paths)
- A string computed at runtime is read the way `var` reads a computed path:
  `{ "val": { "cat": ["a", ".b"] } }` reads the key `"a.b"` if present, otherwise
  `a` then `b`. In the array form, a computed segment containing dots is walked
  as a dotted path
- `Logic::to_json()` writes a dotted `val` key back as `{ "val": "a.b" }`, so
  the serialized rule reads the same key

### Scope levels

Inside an iterator (`map`, `filter`, `all`, `some`, `none`, `group_by`, and
the per-element expressions of `sort`, `distinct`, and `reduce`) the current
element is the data context, so `{ "val": "field" }` reads the element. A
leading `[N]` array selects a different frame. This is the only way to reach
an enclosing iterator's element, or iteration metadata (the element's index
or key).

**Syntax:**
```json
{ "val": [] }
{ "val": [[1], "index"] }
{ "val": [[1], "key"] }
{ "val": [[N]] }
{ "val": [[N], "segment1", "segment2", ...] }
```

Levels come in pairs, because the reference implementation this form comes
from keeps two entries per iterator: the iteration metadata, then the
enclosing element. `[[1]]` and `[[2]]` therefore name the same frame, one
iterator out; `[[3]]` and `[[4]]` name the one beyond it, and so on. Levels
count **frames**, not iterators of a particular kind: a `reduce` body and a
`try` catch arm each consume a level exactly like a `map` body.

- `{ "val": [] }` returns the current element itself (same as `{ "var": "" }`).
- `{ "val": [[N]] }` and `{ "val": [[N], "field", ...] }` read data from the
  frame `ceil(N / 2)` iterators out: `[[1]]`/`[[2]]` is the enclosing element,
  `[[3]]`/`[[4]]` the one outside that. A level that climbs past the outermost
  frame resolves against the root data, so from inside a single iterator
  `[[1], "field"]` and every higher level read the root.
- `{ "val": [[1], "index"] }` returns the current element's zero-based position
  while iterating an array (or an object, in stored order); `{ "val": [[1], "key"] }`
  returns its key while iterating an object. `[[3]]` reads the enclosing
  iterator's index or key, `[[5]]` the one beyond it. Only odd levels work,
  since an even level names an element rather than its metadata.
- `index` and `key` are `null` wherever no such metadata exists: `key` over an
  array, anything inside `reduce` or a `try` catch arm (those frames carry no
  iteration metadata), and a level that climbs past the outermost frame. At an
  even level they are ordinary field names, so `{ "val": [[2], "index"] }` reads
  a *field* called `index` on the enclosing element.
- The sign is ignored: `[[-2]]` and `[[2]]` are the same level.
- A marker is an array of **exactly one** number. `{ "val": [[0, 1]] }` is a
  path chain walking index 0 then index 1, not level 0 with a stray tail, and
  `{ "val": [[2, 9], "field"] }` is not level 2.
- Relative path syntax such as `"../field"` is not supported; the engine treats
  it as a literal key, which resolves to `null`.
- `var` accepts the same `[[N], ...]` form, because it compiles to `val`.

**Reading an enclosing element from a nested iterator:**

```json
{
  "map": [
    { "val": "orders" },
    { "map": [
      { "val": "lines" },
      { "*": [{ "val": "qty" }, { "val": [[2], "rate"] }] }
    ]}
  ]
}
```

`[[2]]` leaves the inner `map` and reads `rate` from the order; `[[3], "index"]`
would give the order's position, and `[[4], "field"]` the root data.

**Differences from json-logic-engine.** datalogic follows that engine for every
level it resolves, with three exceptions, each returning a value where that
engine returns `null`:

| case | json-logic-engine | datalogic |
|------|-------------------|-----------|
| odd level with a non-metadata path | the `{iterator, index}` object, or `null` | the same frame as `N + 1` |
| a level past the outermost frame | `null` | the root data |
| `{ "val": [[0], "index"] }` | the element's `index` *field* | the current index |

It also stops resolving past the fourth chain entry, where datalogic keeps
climbing.

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
- Checks only whether the path exists: a key holding `null` or `""` exists
- Segments must be object keys: `exists` does not index arrays, so
  `{ "exists": ["items", 0] }` is `false` even though `{ "val": ["items", 0] }`
  resolves. Test array elements with `{ "!==": [{ "val": ["items", 0] }, null] }`
  or `length` instead
- An empty segment list, `{ "exists": [] }`, returns `true`

---

## Missing paths

A `var` or `val` read whose path is absent evaluates to `null`, as JSONLogic
specifies. An engine whose `EvaluationConfig::missing_var` is
`MissingVar::Error` raises a `VariableNotFound` error naming the path
instead, so a misspelled path fails rather than producing `null`:

```json
{ "var": "user.nmae" }
// Data: { "user": { "name": "Ana" } }
// Result: null (default)
// Under MissingVar::Error: VariableNotFound error naming "user.nmae"
```

These are not misses, and evaluate the same under both settings:

- a `var` with a default: `{ "var": ["x", 0] }` on `{}` is `0`
- a path that is present and holds `null`
- `{ "var": "" }` and `{ "val": [] }`, the current data
- iteration metadata (`{ "val": [[1], "index"] }`), which is `null` outside an iterator
- `missing`, `missing_some` and `exists`, which report absence

`??` does not shield a miss, because its operand raises before `??` sees a
value. Use `var`'s default, or `try` (`{ "try": [{ "var": "x" }, "fallback"] }`),
which catches the error like any other. The language bindings take the
setting as `"missing_var": "error"` in their JSON config. See
[Missing Variables](../advanced/configuration.md#missing-variables).
