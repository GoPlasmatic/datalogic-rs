# Missing Value Operators

Operators for checking if data fields are missing or undefined.

## missing

Check for missing fields in the data.

**Syntax:**
```json
{ "missing": [key1, key2, ...] }
{ "missing": key }
```

**Arguments:**
- `key1`, `key2`, ... - Field names to check

**Returns:** Array of missing field names.

**Examples:**

```json
// Check single field
{ "missing": "name" }
// Data: { "age": 25 }
// Result: ["name"]

// Check multiple fields
{ "missing": ["name", "email", "phone"] }
// Data: { "name": "Alice", "phone": "555-1234" }
// Result: ["email"]

// All fields present
{ "missing": ["name", "age"] }
// Data: { "name": "Alice", "age": 25 }
// Result: []

// All fields missing
{ "missing": ["name", "age"] }
// Data: {}
// Result: ["name", "age"]

// Nested fields
{ "missing": ["user.name", "user.email"] }
// Data: { "user": { "name": "Alice" } }
// Result: ["user.email"]
```

### Common Patterns

**Require all fields:**
```json
{ "!": { "missing": ["name", "email", "password"] } }
// Returns true only if all fields are present
```

**Check if any field is missing:**
```json
{ "!!": { "missing": ["name", "email"] } }
// Returns true if ANY field is missing
```

**Conditional validation:**
```json
{ "if": [
    { "missing": ["required_field"] },
    { "throw": "Missing required field" },
    "ok"
]}
```

**Try it:**

<div class="playground-widget" data-logic='{"missing": ["name", "email", "phone"]}' data-data='{"name": "Alice", "phone": "555-1234"}'>
</div>

---

## missing_some

Check if at least N fields are missing from a set.

**Syntax:**
```json
{ "missing_some": [minimum, [key1, key2, ...]] }
```

**Arguments:**
- `minimum` - Minimum number of fields that should be present
- `[key1, key2, ...]` - Array of field names to check

**Returns:** Array of missing field names if fewer than minimum are present, empty array otherwise.

**Examples:**

```json
// Need at least 1 of these contact methods
{ "missing_some": [1, ["email", "phone", "address"]] }
// Data: { "email": "a@b.com" }
// Result: [] (1 present, requirement met)

// Data: {}
// Result: ["email", "phone", "address"] (0 present, need at least 1)

// Need at least 2 of these
{ "missing_some": [2, ["name", "email", "phone"]] }
// Data: { "name": "Alice" }
// Result: ["email", "phone"] (only 1 present, need 2)

// Data: { "name": "Alice", "email": "a@b.com" }
// Result: [] (2 present, requirement met)

// Data: { "name": "Alice", "email": "a@b.com", "phone": "555" }
// Result: [] (3 present, exceeds requirement)
```

### Common Patterns

**Require at least one contact method:**
```json
{ "!": { "missing_some": [1, ["email", "phone", "fax"]] } }
// Returns true if at least one contact method is provided
```

**Flexible field requirements:**
```json
{ "if": [
    { "missing_some": [2, ["street", "city", "zip", "country"]] },
    "Please provide at least 2 address fields",
    "Address accepted"
]}
```

**Require majority of fields:**
```json
{ "!": { "missing_some": [3, ["field1", "field2", "field3", "field4", "field5"]] } }
// Returns true if at least 3 of 5 fields are present
```

**Try it:**

<div class="playground-widget" data-logic='{"missing_some": [1, ["email", "phone", "address"]]}' data-data='{"email": "a@b.com"}'>
</div>

---

## Comparison: missing vs missing_some

| Scenario | missing | missing_some |
|----------|---------|--------------|
| All fields required | `{ "!": { "missing": [...] } }` | N/A |
| At least N required | Complex logic needed | `{ "!": { "missing_some": [N, [...]] } }` |
| Check which are missing | Returns missing list | Returns missing list if < N present |
| No minimum | Appropriate | Use with minimum=1 |

---

## Integration with Validation

**Form validation example:**

```json
{ "if": [
    { "missing": ["username", "password"] },
    { "throw": { "code": "VALIDATION_ERROR", "missing": { "missing": ["username", "password"] } } },
    { "if": [
        { "missing_some": [1, ["email", "phone"]] },
        { "throw": { "code": "CONTACT_REQUIRED", "message": "Provide email or phone" } },
        "valid"
    ]}
]}
```

**Conditional field requirements:**

```json
// If business account, require company name
{ "if": [
    { "==": [{ "var": "accountType" }, "business"] },
    { "!": { "missing": ["companyName", "taxId"] } },
    true
]}
```
