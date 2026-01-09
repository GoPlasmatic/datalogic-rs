# Variable Access Operators

These operators access data from the evaluation context.

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
- Numeric indices work for both arrays and string characters
- Returns `null` if path doesn't exist and no default is provided

---

## val

Alternative variable access with additional path navigation capabilities.

**Syntax:**
```json
{ "val": "path" }
{ "val": ["path", default] }
```

**Arguments:**
- `path` - Path to the value, supports additional navigation syntax
- `default` - Optional default value

**Returns:** The value at the path, or the default value, or `null`.

**Examples:**

```json
// Simple access (same as var)
{ "val": "name" }
// Data: { "name": "Bob" }
// Result: "Bob"

// Nested access
{ "val": "config.settings.enabled" }
// Data: { "config": { "settings": { "enabled": true } } }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"val": "config.settings.enabled"}' data-data='{"config": {"settings": {"enabled": true}}}'>
</div>

**Notes:**
- Similar to `var` but with extended path syntax support
- Useful for complex data navigation scenarios

---

## exists

Check if a variable path exists in the data.

**Syntax:**
```json
{ "exists": "path" }
{ "exists": { "var": "path" } }
```

**Arguments:**
- `path` - Path to check (string or var operation)

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

// Check nested path
{ "exists": "user.profile" }
// Data: { "user": { "profile": { "name": "Bob" } } }
// Result: true

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
- Useful for conditional logic based on data structure
