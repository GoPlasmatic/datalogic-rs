# DateTime Operators

Operations for working with dates, times, and durations.

## now

Get the current UTC datetime.

**Syntax:**
```json
{ "now": [] }
```

**Arguments:** None

**Returns:** Current UTC datetime as ISO 8601 string.

**Examples:**

```json
{ "now": [] }
// Result: "2024-01-15T14:30:00Z" (current time)

// Check if date is in the future
{ ">": [{ "var": "expiresAt" }, { "now": [] }] }
// Data: { "expiresAt": "2025-12-31T00:00:00Z" }
// Result: true or false depending on current time

// Check if event is happening now
{ "and": [
    { "<=": [{ "var": "startTime" }, { "now": [] }] },
    { ">=": [{ "var": "endTime" }, { "now": [] }] }
]}
```

**Try it:**

<div class="playground-widget" data-logic='{"now": []}' data-data='{}'>
</div>

**Notes:**
- Returns ISO 8601 formatted string (e.g., "2024-01-15T14:30:00Z")
- Always returns UTC time
- Useful for time-based conditions and comparisons

---

## datetime

Parse or validate a datetime value.

**Syntax:**
```json
{ "datetime": value }
```

**Arguments:**
- `value` - ISO 8601 datetime string

**Returns:** The validated datetime string (preserving timezone information).

**Examples:**

```json
// Parse ISO string
{ "datetime": "2024-01-01T00:00:00Z" }
// Result: "2024-01-01T00:00:00Z"

// With timezone offset
{ "datetime": "2024-01-01T10:00:00+05:30" }
// Result: "2024-01-01T10:00:00+05:30"

// Compare datetimes
{ ">": [
    { "datetime": "2024-06-15T00:00:00Z" },
    { "datetime": "2024-01-01T00:00:00Z" }
]}
// Result: true

// Add duration to datetime
{ "+": [
    { "datetime": "2024-01-01T00:00:00Z" },
    { "timestamp": "7d" }
]}
// Result: "2024-01-08T00:00:00Z"
```

**Try it:**

<div class="playground-widget" data-logic='{"datetime": "2024-01-01T00:00:00Z"}' data-data='{}'>
</div>

---

## timestamp

Create or parse a duration value. Durations represent time periods (not points in time).

**Syntax:**
```json
{ "timestamp": duration_string }
```

**Arguments:**
- `duration_string` - Duration in format like "1d:2h:3m:4s" or partial like "1d", "2h", "30m", "45s"

**Returns:** Normalized duration string in format "Xd:Xh:Xm:Xs".

**Duration Format:**
- `d` - Days
- `h` - Hours
- `m` - Minutes
- `s` - Seconds

**Examples:**

```json
// Full duration format
{ "timestamp": "1d:2h:3m:4s" }
// Result: "1d:2h:3m:4s"

// Days only
{ "timestamp": "2d" }
// Result: "2d:0h:0m:0s"

// Hours only
{ "timestamp": "5h" }
// Result: "0d:5h:0m:0s"

// Minutes only
{ "timestamp": "30m" }
// Result: "0d:0h:30m:0s"

// Compare durations
{ ">": [{ "timestamp": "2d" }, { "timestamp": "36h" }] }
// Result: true (2 days > 36 hours)

// Duration equality
{ "==": [{ "timestamp": "1d" }, { "timestamp": "24h" }] }
// Result: true
```

**Try it:**

<div class="playground-widget" data-logic='{"timestamp": "1d:2h:3m:4s"}' data-data='{}'>
</div>

### Duration Arithmetic

Durations can be used in arithmetic operations:

```json
// Multiply duration
{ "*": [{ "timestamp": "1d" }, 2] }
// Result: "2d:0h:0m:0s"

// Divide duration
{ "/": [{ "timestamp": "2d" }, 2] }
// Result: "1d:0h:0m:0s"

// Add durations
{ "+": [{ "timestamp": "1d" }, { "timestamp": "12h" }] }
// Result: "1d:12h:0m:0s"

// Subtract durations
{ "-": [{ "timestamp": "2d" }, { "timestamp": "12h" }] }
// Result: "1d:12h:0m:0s"

// Add duration to datetime
{ "+": [
    { "datetime": "2024-01-01T00:00:00Z" },
    { "timestamp": "7d" }
]}
// Result: "2024-01-08T00:00:00Z"

// Subtract duration from datetime
{ "-": [
    { "datetime": "2024-01-15T00:00:00Z" },
    { "timestamp": "7d" }
]}
// Result: "2024-01-08T00:00:00Z"

// Difference between two datetimes (returns duration)
{ "-": [
    { "datetime": "2024-01-08T00:00:00Z" },
    { "datetime": "2024-01-01T00:00:00Z" }
]}
// Result: "7d:0h:0m:0s"
```

---

## parse_date

Parse a date string with a custom format into an ISO datetime.

**Syntax:**
```json
{ "parse_date": [string, format] }
```

**Arguments:**
- `string` - Date string to parse
- `format` - Format string using simplified tokens

**Returns:** Parsed datetime as ISO 8601 string.

**Format Tokens:**
| Token | Description | Example |
|-------|-------------|---------|
| `yyyy` | 4-digit year | 2024 |
| `MM` | 2-digit month | 01-12 |
| `dd` | 2-digit day | 01-31 |
| `HH` | 2-digit hour (24h) | 00-23 |
| `mm` | 2-digit minute | 00-59 |
| `ss` | 2-digit second | 00-59 |

**Examples:**

```json
// Parse US date format
{ "parse_date": ["12/25/2024", "MM/dd/yyyy"] }
// Result: "2024-12-25T00:00:00Z"

// Parse European format
{ "parse_date": ["25-12-2024", "dd-MM-yyyy"] }
// Result: "2024-12-25T00:00:00Z"

// Parse date only
{ "parse_date": ["2024-01-15", "yyyy-MM-dd"] }
// Result: "2024-01-15T00:00:00Z"

// With variable
{ "parse_date": [{ "var": "dateStr" }, "yyyy-MM-dd"] }
// Data: { "dateStr": "2024-06-15" }
// Result: "2024-06-15T00:00:00Z"
```

**Try it:**

<div class="playground-widget" data-logic='{"parse_date": ["2024-01-15", "yyyy-MM-dd"]}' data-data='{}'>
</div>

---

## format_date

Format a datetime as a string with a custom format.

**Syntax:**
```json
{ "format_date": [datetime, format] }
```

**Arguments:**
- `datetime` - Datetime value to format
- `format` - Format string using simplified tokens (same as parse_date)

**Returns:** Formatted date string.

**Special Format:**
- `z` - Returns timezone offset (e.g., "+0500")

**Examples:**

```json
// Format as date only
{ "format_date": [{ "datetime": "2024-01-15T14:30:00Z" }, "yyyy-MM-dd"] }
// Result: "2024-01-15"

// Format as US date
{ "format_date": [{ "datetime": "2024-12-25T00:00:00Z" }, "MM/dd/yyyy"] }
// Result: "12/25/2024"

// Get timezone offset
{ "format_date": [{ "datetime": "2024-01-01T10:00:00+05:00" }, "z"] }
// Result: "+0500"

// Format current time
{ "format_date": [{ "now": [] }, "yyyy-MM-dd"] }
// Result: "2024-01-15" (current date)

// With variable
{ "format_date": [{ "var": "date" }, "dd/MM/yyyy"] }
// Data: { "date": "2024-12-25T00:00:00Z" }
// Result: "25/12/2024"
```

**Try it:**

<div class="playground-widget" data-logic='{"format_date": [{"datetime": "2024-01-15T14:30:00Z"}, "yyyy-MM-dd"]}' data-data='{}'>
</div>

---

## date_diff

Calculate the difference between two dates in a specified unit.

**Syntax:**
```json
{ "date_diff": [date1, date2, unit] }
```

**Arguments:**
- `date1` - First datetime
- `date2` - Second datetime
- `unit` - Unit of measurement: "days", "hours", "minutes", "seconds"

**Returns:** Difference as an integer in the specified unit.

**Examples:**

```json
// Days between dates
{ "date_diff": [
    { "datetime": "2024-12-31T00:00:00Z" },
    { "datetime": "2024-01-01T00:00:00Z" },
    "days"
]}
// Result: 365

// Hours difference
{ "date_diff": [
    { "datetime": "2024-01-01T12:00:00Z" },
    { "datetime": "2024-01-01T00:00:00Z" },
    "hours"
]}
// Result: 12

// With variables
{ "date_diff": [
    { "var": "end" },
    { "var": "start" },
    "days"
]}
// Data: {
//   "start": "2024-01-01T00:00:00Z",
//   "end": "2024-01-15T00:00:00Z"
// }
// Result: 14

// Check if within 24 hours
{ "<": [
    { "date_diff": [{ "now": [] }, { "var": "timestamp" }, "hours"] },
    24
]}
// Data: { "timestamp": "2024-01-15T10:00:00Z" }
// Result: true or false

// Days since creation
{ "date_diff": [
    { "now": [] },
    { "var": "createdAt" },
    "days"
]}
```

**Try it:**

<div class="playground-widget" data-logic='{"date_diff": [{"datetime": "2024-01-15T00:00:00Z"}, {"datetime": "2024-01-01T00:00:00Z"}, "days"]}' data-data='{}'>
</div>

---

## DateTime Patterns

### Check if date is in the past

```json
{ "<": [{ "var": "date" }, { "now": [] }] }
```

### Check if date is in the future

```json
{ ">": [{ "var": "date" }, { "now": [] }] }
```

### Check if within time window

```json
{ "and": [
    { ">=": [{ "now": [] }, { "var": "startTime" }] },
    { "<=": [{ "now": [] }, { "var": "endTime" }] }
]}
```

### Add days to a date

```json
{ "+": [
    { "var": "date" },
    { "timestamp": "7d" }
]}
```

### Calculate days until expiration

```json
{ "date_diff": [
    { "var": "expiresAt" },
    { "now": [] },
    "days"
]}
```

### Check if expired

```json
{ "<": [{ "var": "expiresAt" }, { "now": [] }] }
```
