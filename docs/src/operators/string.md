# String Operators

String manipulation and searching operations.

## cat

Concatenate strings together.

**Syntax:**
```json
{ "cat": [a, b, ...] }
```

**Arguments:**
- `a`, `b`, ... - Values to concatenate (variadic)

**Returns:** Concatenated string.

**Examples:**

```json
// Simple concatenation
{ "cat": ["Hello", " ", "World"] }
// Result: "Hello World"

// With variables
{ "cat": ["Hello, ", { "var": "name" }, "!"] }
// Data: { "name": "Alice" }
// Result: "Hello, Alice!"

// Non-strings are converted
{ "cat": ["Value: ", 42] }
// Result: "Value: 42"

{ "cat": ["Is active: ", true] }
// Result: "Is active: true"

// Building paths
{ "cat": ["/users/", { "var": "userId" }, "/profile"] }
// Data: { "userId": 123 }
// Result: "/users/123/profile"
```

**Try it:**

<div class="playground-widget" data-logic='{"cat": ["Hello, ", {"var":"name"}, "!"]}' data-data='{"name": "Alice"}'>
</div>

---

## substr

Extract a substring.

**Syntax:**
```json
{ "substr": [string, start] }
{ "substr": [string, start, length] }
```

**Arguments:**
- `string` - Source string
- `start` - Starting index (0-based, negative counts from end)
- `length` - Number of characters (optional, negative counts from end)

**Returns:** Extracted substring.

**Examples:**

```json
// From start index
{ "substr": ["Hello World", 0, 5] }
// Result: "Hello"

// From middle
{ "substr": ["Hello World", 6] }
// Result: "World"

// Negative start (from end)
{ "substr": ["Hello World", -5] }
// Result: "World"

// Negative length (exclude from end)
{ "substr": ["Hello World", 0, -6] }
// Result: "Hello"

// Get file extension
{ "substr": ["document.pdf", -3] }
// Result: "pdf"

// With variables
{ "substr": [{ "var": "text" }, 0, 10] }
// Data: { "text": "This is a long string" }
// Result: "This is a "
```

**Try it:**

<div class="playground-widget" data-logic='{"substr": ["Hello World", -5]}' data-data='{}'>
</div>

---

## in

Check if a value is contained in a string or array.

**Syntax:**
```json
{ "in": [needle, haystack] }
```

**Arguments:**
- `needle` - Value to search for
- `haystack` - String or array to search in

**Returns:** `true` if found, `false` otherwise.

**Examples:**

```json
// String contains substring
{ "in": ["World", "Hello World"] }
// Result: true

{ "in": ["xyz", "Hello World"] }
// Result: false

// Array contains element
{ "in": [2, [1, 2, 3]] }
// Result: true

{ "in": [5, [1, 2, 3]] }
// Result: false

// Check membership
{ "in": [{ "var": "role" }, ["admin", "moderator"]] }
// Data: { "role": "admin" }
// Result: true

// Check substring
{ "in": ["@", { "var": "email" }] }
// Data: { "email": "user@example.com" }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"in": [{"var":"role"}, ["admin", "moderator"]]}' data-data='{"role": "admin"}'>
</div>

---

## length

Get the length of a string or array.

**Syntax:**
```json
{ "length": value }
```

**Arguments:**
- `value` - String or array

**Returns:** Length (number of characters or elements).

**Examples:**

```json
// String length
{ "length": "Hello" }
// Result: 5

// Array length
{ "length": [1, 2, 3, 4, 5] }
// Result: 5

// Empty values
{ "length": "" }
// Result: 0

{ "length": [] }
// Result: 0

// With variables
{ "length": { "var": "items" } }
// Data: { "items": ["a", "b", "c"] }
// Result: 3

// Check minimum length
{ ">=": [{ "length": { "var": "password" } }, 8] }
// Data: { "password": "secret123" }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{">=": [{"length": {"var":"password"}}, 8]}' data-data='{"password": "secret123"}'>
</div>

---

## starts_with

Check if a string starts with a prefix.

**Syntax:**
```json
{ "starts_with": [string, prefix] }
```

**Arguments:**
- `string` - String to check
- `prefix` - Prefix to look for

**Returns:** `true` if string starts with prefix, `false` otherwise.

**Examples:**

```json
{ "starts_with": ["Hello World", "Hello"] }
// Result: true

{ "starts_with": ["Hello World", "World"] }
// Result: false

// Check URL scheme
{ "starts_with": [{ "var": "url" }, "https://"] }
// Data: { "url": "https://example.com" }
// Result: true

// Case sensitive
{ "starts_with": ["Hello", "hello"] }
// Result: false
```

**Try it:**

<div class="playground-widget" data-logic='{"starts_with": [{"var":"url"}, "https://"]}' data-data='{"url": "https://example.com"}'>
</div>

---

## ends_with

Check if a string ends with a suffix.

**Syntax:**
```json
{ "ends_with": [string, suffix] }
```

**Arguments:**
- `string` - String to check
- `suffix` - Suffix to look for

**Returns:** `true` if string ends with suffix, `false` otherwise.

**Examples:**

```json
{ "ends_with": ["Hello World", "World"] }
// Result: true

{ "ends_with": ["Hello World", "Hello"] }
// Result: false

// Check file extension
{ "ends_with": [{ "var": "filename" }, ".pdf"] }
// Data: { "filename": "report.pdf" }
// Result: true

// Case sensitive
{ "ends_with": ["test.PDF", ".pdf"] }
// Result: false
```

**Try it:**

<div class="playground-widget" data-logic='{"ends_with": [{"var":"filename"}, ".pdf"]}' data-data='{"filename": "report.pdf"}'>
</div>

---

## upper

Convert string to uppercase.

**Syntax:**
```json
{ "upper": string }
```

**Arguments:**
- `string` - String to convert

**Returns:** Uppercase string.

**Examples:**

```json
{ "upper": "hello" }
// Result: "HELLO"

{ "upper": "Hello World" }
// Result: "HELLO WORLD"

// With variable
{ "upper": { "var": "name" } }
// Data: { "name": "alice" }
// Result: "ALICE"
```

**Try it:**

<div class="playground-widget" data-logic='{"upper": {"var":"name"}}' data-data='{"name": "alice"}'>
</div>

---

## lower

Convert string to lowercase.

**Syntax:**
```json
{ "lower": string }
```

**Arguments:**
- `string` - String to convert

**Returns:** Lowercase string.

**Examples:**

```json
{ "lower": "HELLO" }
// Result: "hello"

{ "lower": "Hello World" }
// Result: "hello world"

// Case-insensitive comparison
{ "==": [
    { "lower": { "var": "input" } },
    "yes"
]}
// Data: { "input": "YES" }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"==": [{"lower": {"var":"input"}}, "yes"]}' data-data='{"input": "YES"}'>
</div>

---

## trim

Remove leading and trailing whitespace.

**Syntax:**
```json
{ "trim": string }
```

**Arguments:**
- `string` - String to trim

**Returns:** String with whitespace removed from both ends.

**Examples:**

```json
{ "trim": "  hello  " }
// Result: "hello"

{ "trim": "\n\ttext\n\t" }
// Result: "text"

// Clean user input
{ "trim": { "var": "userInput" } }
// Data: { "userInput": "  search query  " }
// Result: "search query"
```

**Try it:**

<div class="playground-widget" data-logic='{"trim": {"var":"userInput"}}' data-data='{"userInput": "  search query  "}'>
</div>

---

## split

Split a string into an array.

**Syntax:**
```json
{ "split": [string, delimiter] }
```

**Arguments:**
- `string` - String to split
- `delimiter` - Delimiter to split on

**Returns:** Array of substrings.

**Examples:**

```json
// Split by space
{ "split": ["Hello World", " "] }
// Result: ["Hello", "World"]

// Split by comma
{ "split": ["a,b,c", ","] }
// Result: ["a", "b", "c"]

// Split by empty string (characters)
{ "split": ["abc", ""] }
// Result: ["a", "b", "c"]

// Parse CSV-like data
{ "split": [{ "var": "tags" }, ","] }
// Data: { "tags": "rust,json,logic" }
// Result: ["rust", "json", "logic"]

// Get first part
{ "var": "0" }
// Applied to: { "split": ["user@example.com", "@"] }
// Result: "user"
```

**Try it:**

<div class="playground-widget" data-logic='{"split": [{"var":"tags"}, ","]}' data-data='{"tags": "rust,json,logic"}'>
</div>
