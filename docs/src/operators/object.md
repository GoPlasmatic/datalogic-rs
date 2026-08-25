# Object Operators

Operations for taking objects apart: enumerate keys, values, or key/value rows.
These are the read-side complement of templating's computed-key object
construction; `entries` in particular turns any object into rows that the
array vocabulary (`map`, `filter`, `group_by`, ...) can iterate.

> **Feature flags (Rust crate).** All object operators require the `ext-object` feature. Every language binding enables all operator features. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

## keys

List an object's keys.

**Syntax:**
```json
{ "keys": [object] }
```

**Arguments:**
- `object` - A value that resolves to an object

**Returns:** Array of the object's key strings, in stored order.

**Examples:**

```json
{ "keys": [{ "var": "scores" }] }
// Data: { "scores": { "alice": 10, "bob": 7 } }
// Result: ["alice", "bob"]
```

**Try it:**

<div class="playground-widget" data-logic='{"keys": [{"var": "scores"}]}' data-data='{"scores": {"alice": 10, "bob": 7}}'>
</div>

---

## values

List an object's values.

**Syntax:**
```json
{ "values": [object] }
```

**Arguments:**
- `object` - A value that resolves to an object

**Returns:** Array of the object's values, in stored order.

**Examples:**

```json
{ "values": [{ "var": "scores" }] }
// Data: { "scores": { "alice": 10, "bob": 7 } }
// Result: [10, 7]

// Sum an object's values
{ "reduce": [
    { "values": [{ "var": "scores" }] },
    { "+": [{ "var": "accumulator" }, { "var": "current" }] },
    0
]}
// Result: 17
```

**Try it:**

<div class="playground-widget" data-logic='{"values": [{"var": "scores"}]}' data-data='{"scores": {"alice": 10, "bob": 7}}'>
</div>

---

## entries

Turn an object into an array of `{key, value}` rows.

**Syntax:**
```json
{ "entries": [object] }
```

**Arguments:**
- `object` - A value that resolves to an object

**Returns:** Array of `{"key": ..., "value": ...}` objects, in stored order.

**Examples:**

```json
{ "entries": [{ "var": "scores" }] }
// Data: { "scores": { "alice": 10, "bob": 7 } }
// Result: [
//   { "key": "alice", "value": 10 },
//   { "key": "bob", "value": 7 }
// ]

// Iterate an object with the array vocabulary
{ "map": [
    { "entries": [{ "var": "scores" }] },
    { "cat": [{ "var": "key" }, ": ", { "var": "value" }] }
]}
// Result: ["alice: 10", "bob: 7"]
```

**Try it:**

<div class="playground-widget" data-logic='{"entries": [{"var": "scores"}]}' data-data='{"scores": {"alice": 10, "bob": 7}}'>
</div>

---

**Notes (all three operators):**
- `null` input yields `[]`, convenient when the object field may be absent.
- Any other non-object input (array, string, number, boolean) is an error.
- Keys come out in stored order, duplicates included, exactly as the object carries them. What order an object carries depends on how the data entered: JSON text parsed by the engine (the `ParsedData` and string entry points, and the Node, WASM, and C-based bindings) keeps source-text order and duplicate keys, while data passed as a `serde_json::Value` or as a Python dict is key-sorted and duplicate-free (so `{ "zed": 10, "alpha": 7 }` comes out as `["alpha", "zed"]`). Do not rely on source-text order across input routes.
