# DateTime Operators

Operations for working with dates and times.

## now

Get the current timestamp.

**Syntax:**
```json
{ "now": [] }
```

**Arguments:** None

**Returns:** Current Unix timestamp in milliseconds.

**Examples:**

```json
{ "now": [] }
// Result: 1704067200000 (example timestamp)

// Check if date is in the future
{ ">": [{ "var": "expiresAt" }, { "now": [] }] }
// Data: { "expiresAt": 1735689600000 }
// Result: true or false depending on current time

// Calculate age in days
{ "/": [
    { "-": [{ "now": [] }, { "var": "createdAt" }] },
    86400000
]}
// 86400000 = milliseconds in a day
```

**Try it:**

<div class="playground-widget" data-logic='{"now": []}' data-data='{}'>
</div>

**Notes:**
- Returns milliseconds since Unix epoch (January 1, 1970)
- Useful for time-based conditions and calculations

---

## datetime

Parse or create a datetime value.

**Syntax:**
```json
{ "datetime": value }
{ "datetime": [value, format] }
```

**Arguments:**
- `value` - Timestamp (number) or date string
- `format` - Optional format string for parsing

**Returns:** Datetime value (as timestamp or formatted string depending on usage).

**Examples:**

```json
// From timestamp
{ "datetime": 1704067200000 }
// Result: datetime object

// From ISO string
{ "datetime": "2024-01-01T00:00:00Z" }
// Result: datetime object

// Parse with format
{ "datetime": ["01/15/2024", "%m/%d/%Y"] }
// Result: datetime object for January 15, 2024
```

---

## timestamp

Convert a datetime to Unix timestamp.

**Syntax:**
```json
{ "timestamp": datetime }
```

**Arguments:**
- `datetime` - Datetime value or ISO string

**Returns:** Unix timestamp in milliseconds.

**Examples:**

```json
// From ISO string
{ "timestamp": "2024-01-01T00:00:00Z" }
// Result: 1704067200000

// With variable
{ "timestamp": { "var": "date" } }
// Data: { "date": "2024-06-15T12:00:00Z" }
// Result: 1718452800000
```

**Try it:**

<div class="playground-widget" data-logic='{"timestamp": "2024-01-01T00:00:00Z"}' data-data='{}'>
</div>

---

## parse_date

Parse a date string into a datetime.

**Syntax:**
```json
{ "parse_date": [string, format] }
```

**Arguments:**
- `string` - Date string to parse
- `format` - Format string (strftime-style)

**Returns:** Parsed datetime.

**Format Specifiers:**
- `%Y` - 4-digit year (2024)
- `%m` - Month (01-12)
- `%d` - Day (01-31)
- `%H` - Hour 24h (00-23)
- `%M` - Minute (00-59)
- `%S` - Second (00-59)
- `%y` - 2-digit year (24)
- `%b` - Abbreviated month (Jan)
- `%B` - Full month (January)

**Examples:**

```json
// Parse US date format
{ "parse_date": ["12/25/2024", "%m/%d/%Y"] }
// Result: datetime for December 25, 2024

// Parse European format
{ "parse_date": ["25-12-2024", "%d-%m-%Y"] }
// Result: datetime for December 25, 2024

// Parse with time
{ "parse_date": ["2024-01-15 14:30:00", "%Y-%m-%d %H:%M:%S"] }
// Result: datetime for January 15, 2024 at 2:30 PM

// With variable
{ "parse_date": [{ "var": "dateStr" }, "%Y-%m-%d"] }
// Data: { "dateStr": "2024-06-15" }
// Result: datetime for June 15, 2024
```

---

## format_date

Format a datetime as a string.

**Syntax:**
```json
{ "format_date": [datetime, format] }
```

**Arguments:**
- `datetime` - Datetime value to format
- `format` - Format string (strftime-style)

**Returns:** Formatted date string.

**Examples:**

```json
// Format as ISO date
{ "format_date": [{ "now": [] }, "%Y-%m-%d"] }
// Result: "2024-01-15"

// Format as US date
{ "format_date": [{ "var": "date" }, "%m/%d/%Y"] }
// Data: { "date": "2024-12-25T00:00:00Z" }
// Result: "12/25/2024"

// Format with time
{ "format_date": [{ "now": [] }, "%Y-%m-%d %H:%M:%S"] }
// Result: "2024-01-15 14:30:00"

// Human-readable format
{ "format_date": [{ "var": "date" }, "%B %d, %Y"] }
// Data: { "date": "2024-01-15T00:00:00Z" }
// Result: "January 15, 2024"

// Just time
{ "format_date": [{ "now": [] }, "%H:%M"] }
// Result: "14:30"
```

---

## date_diff

Calculate the difference between two dates.

**Syntax:**
```json
{ "date_diff": [date1, date2, unit] }
```

**Arguments:**
- `date1` - First datetime
- `date2` - Second datetime
- `unit` - Unit of measurement: "days", "hours", "minutes", "seconds", "milliseconds"

**Returns:** Difference as a number in the specified unit.

**Examples:**

```json
// Days between dates
{ "date_diff": [
    "2024-12-31T00:00:00Z",
    "2024-01-01T00:00:00Z",
    "days"
]}
// Result: 365

// Hours difference
{ "date_diff": [
    { "var": "end" },
    { "var": "start" },
    "hours"
]}
// Data: {
//   "start": "2024-01-01T00:00:00Z",
//   "end": "2024-01-01T12:00:00Z"
// }
// Result: 12

// Check if within 24 hours
{ "<": [
    { "date_diff": [{ "now": [] }, { "var": "timestamp" }, "hours"] },
    24
]}
// Data: { "timestamp": "2024-01-15T10:00:00Z" }
// Result: true or false

// Calculate age in years (approximate)
{ "floor": {
    "/": [
        { "date_diff": [{ "now": [] }, { "var": "birthdate" }, "days"] },
        365.25
    ]
}}
// Data: { "birthdate": "1990-01-15T00:00:00Z" }
// Result: age in years
```

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

### Calculate expiration

```json
{ "+": [
    { "var": "createdAt" },
    { "*": [{ "var": "ttlDays" }, 86400000] }
]}
// Returns expiration timestamp
```
