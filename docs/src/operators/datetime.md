# DateTime Operators

Operations for working with dates, times, and durations.

> **Feature flag (Rust crate).** All datetime operators require the `datetime` feature (which pulls in `chrono` and `chrono-tz`). Every language binding enables it. See the [feature table](overview.md#which-operators-need-which-cargo-feature).

**Argument counts:** `datetime` and `timestamp` take one argument, `parse_date` and `format_date` two or three, `date_diff` three, and `now` none. A call with too few arguments raises `InvalidArguments` naming what is missing (for example `"date_diff requires two dates and a unit"`). The operators ignore arguments past the last one they read.

## Datetime and duration values

The engine treats these values as datetimes and durations wherever an operator looks for one (comparisons, `+` / `-`, `format_date`, `date_diff`, `type`):

- A string that parses as an ISO 8601 / RFC 3339 datetime, such as `"2024-01-15T10:30:00Z"`, `"2024-01-15T10:30:00-05:00"` or the naive `"2024-01-15T10:30:00"` (read as UTC), whether it is written in the rule, read from data or returned by another operator.
- A string that parses as a duration, such as `"1d:2h:3m:4s"`, `"2h30m"` or `"36h"`.
- The boundary forms `{"datetime": "..."}` and `{"timestamp": "..."}` in data. (In a rule, the same object is a call to the operator of that name.) Only an object with that single key counts: a record such as `{"datetime": "2024-01-01T00:00:00Z", "user": "alice"}` is ordinary data, so `==`, `===`, `in` and `distinct` compare every field and `type` reports `"object"`.

The operators on this page and datetime arithmetic return datetimes as ISO 8601 strings and durations as `"Xd:Xh:Xm:Xs"` strings, so a result feeds straight into the next operator.

## now

Get the current UTC datetime.

**Syntax:**
```json
{ "now": [] }
```

**Arguments:** None

**Returns:** The current UTC time as an ISO 8601 datetime string.

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
- `{ "type": { "now": [] } }` is `"datetime"`
- The result is in UTC (`Z`)
- The engine reads the clock at evaluation time and never constant-folds `now`, so a compiled rule sees a fresh time on every evaluation

---

## datetime

Parse or validate a datetime value.

**Syntax:**
```json
{ "datetime": value }
```

**Arguments:**
- `value` - An RFC 3339 datetime string (`2024-01-01T00:00:00Z`, or with a `+HH:MM`/`-HH:MM` offset; fractional seconds and a space instead of `T` are accepted), or a naive `YYYY-MM-DDTHH:MM:SS` string, which the engine reads as UTC. Date-only strings (`2024-01-01`) and colon-less offsets (`-0500`, `+05`) fail with `Invalid datetime format`; use `parse_date` for those and for custom formats

**Returns:** The argument unchanged once it parses, so the offset and spelling you passed are kept (`"2024-01-01 00:00:00Z"` comes back with its space). Any other value raises `Invalid datetime format`. `{ "type": { "datetime": "2024-01-01T00:00:00Z" } }` is `"datetime"`.

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
- `duration_string` - Duration in format like "1d:2h:3m:4s", partial like "1d", "2h", "30m", "45s", or compact like "1d2h3m4s"

**Returns:** The duration as a normalized `"Xd:Xh:Xm:Xs"` string.

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

// Compact form
{ "timestamp": "1d2h3m4s" }
// Result: "1d:2h:3m:4s"

// Units overflow into the next larger unit
{ "timestamp": "36h" }
// Result: "1d:12h:0m:0s"
```

**Notes:**
- `{ "type": { "timestamp": "1d" } }` is `"duration"`
- Units overflow into the next larger unit (`"1d:25h"` becomes `"2d:1h:0m:0s"`)
- Negative (`"-1d"`), fractional (`"1.5h"`), week (`"1w"`), and numeric (`3600`) inputs fail with `Invalid duration format`

**Try it:**

<div class="playground-widget" data-logic='{"timestamp": "1d:2h:3m:4s"}' data-data='{}'>
</div>

### Duration Arithmetic

`+` and `-` accept datetimes and durations, `*` scales a duration by a number in either order, and `/` divides a duration by a number:

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

// The result of datetime +/- duration is a UTC instant: the parsed offset is not carried through
{ "+": [
    { "datetime": "2024-01-01T10:00:00+05:30" },
    { "timestamp": "1d" }
]}
// Result: "2024-01-02T04:30:00Z"

// Any number of operands, durations on either side of the datetime
{ "+": ["1d", { "var": "d" }, "12h"] }
// Data: { "d": "2024-01-01T00:00:00Z" }
// Result: "2024-01-02T12:00:00Z"

{ "-": [{ "var": "d" }, "1d", "1d"] }
// Data: { "d": "2024-01-08T00:00:00Z" }
// Result: "2024-01-06T00:00:00Z"
```

`+` and `-` fold left to right. Datetime plus duration (in either order) is a
datetime, datetime minus datetime is a duration, and two durations add or
subtract to a duration. A step with no meaning is a `NaN` error under the
default configuration: two datetimes added, a duration minus a datetime,
or a plain number among datetime operands.

Arithmetic on a datetime that carries an offset drops that offset: the
result renders as `...Z`, and `format_date` with the bare `"z"` format
reports `+0000` for it. To render such a result in a local zone, pass the
zone argument to `format_date` (for example `"Asia/Kolkata"`, which gives
`"10:00"` for the offset example above with format `"HH:mm"`).

---

## parse_date

Parse a date string with a custom format into a datetime value.

**Syntax:**
```json
{ "parse_date": [string, format] }
{ "parse_date": [string, format, timezone] }
```

**Arguments:**
- `string` - Date string to parse
- `format` - Format string using simplified tokens
- `timezone` - Optional IANA zone name (e.g. `"Asia/Kolkata"`). Without it, the engine reads naive input as UTC; with it, the engine reads the input as wall-clock time *in that zone* and resolves it to the corresponding UTC instant.

**Returns:** The parsed instant as an ISO 8601 datetime string in UTC. Input that does not match the format raises `Failed to parse date`.

**Format Tokens:**
| Token | Description | Example |
|-------|-------------|---------|
| `yyyy` | 4-digit year | 2024 |
| `MMMM` | full month name | January |
| `MMM` | abbreviated month name | Jan |
| `MM` | 2-digit month | 01-12 |
| `dd` | 2-digit day | 01-31 |
| `HH` | 2-digit hour (24h) | 00-23 |
| `mm` | 2-digit minute | 00-59 |
| `ss` | 2-digit second | 00-59 |
| `EEEE` | full weekday name | Monday |
| `EEE` | abbreviated weekday name | Mon |

Raw [chrono `%` specifiers](https://docs.rs/chrono/latest/chrono/format/strftime/index.html) also pass through unchanged.

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

// Read a naive local time as New York wall clock (EDT in June)
{ "parse_date": ["2024-06-15 12:00:00", "yyyy-MM-dd HH:mm:ss", "America/New_York"] }
// Result: "2024-06-15T16:00:00Z"

// With variable
{ "parse_date": [{ "var": "dateStr" }, "yyyy-MM-dd"] }
// Data: { "dateStr": "2024-06-15" }
// Result: "2024-06-15T00:00:00Z"
```

**Timezone notes:**
- Zone offsets, DST included, come from the IANA table compiled into the crate (`chrono-tz`); the engine reads no tzdata files at runtime.
- An ambiguous local time (clocks rolled back, the wall-clock occurs twice) resolves to the **earlier** instant; a nonexistent one (spring-forward gap) raises `Nonexistent local time for timezone`.
- An unknown zone name written as a literal in the rule does not stop `Engine::compile`: the call raises `Invalid Arguments` (naming the operator, not the zone) when the rule is evaluated, and `Engine::check` / `Engine::compile_checked` report the zone before the rule runs. A zone computed at evaluation time, from data or from another operator such as `cat`, fails with `Unknown timezone: <name>`.

**Try it:**

<div class="playground-widget" data-logic='{"parse_date": ["2024-01-15", "yyyy-MM-dd"]}' data-data='{}'>
</div>

---

## format_date

Format a datetime as a string with a custom format.

**Syntax:**
```json
{ "format_date": [datetime, format] }
{ "format_date": [datetime, format, timezone] }
```

**Arguments:**
- `datetime` - Datetime value to format
- `format` - Format string using simplified tokens (same as parse_date)
- `timezone` - Optional IANA zone name (e.g. `"Asia/Kolkata"`). When present, `format_date` renders the instant as wall-clock time in that zone (DST-correct via the IANA table) instead of UTC.

**Returns:** Formatted date string. A first argument that is not a datetime raises `Failed to format date`. A raw `%` specifier that chrono does not know (`"%Q"`) or a trailing lone `%` raises `Invalid date format`, whether the format is written in the rule or read from data.

**Special Format:**
- `z` - Returns the timezone offset (e.g., "+0500"). Without a zone argument this is the *source* offset the datetime was parsed with; with a zone argument it is the target zone's offset at that instant.
- `z` is honoured only when it is the entire format string. Inside a longer format it is emitted literally: `"yyyy-MM-dd HH:mm z"` on `2024-01-01T10:00:00+05:30` gives `"2024-01-01 04:30 z"`.
- Without a zone argument every other token renders the UTC instant, so `"HH:mm"` on that same value gives `"04:30"` and the raw chrono `%z` gives `+0000`; the source offset is reachable only through the bare `"z"` format. To get wall-clock time plus offset in one string, pass the zone argument and use `%z` or `%Z`: `"HH:mm %z"` with `"Asia/Kolkata"` gives `"10:00 +0530"`, and `"HH:mm %Z"` gives `"10:00 IST"`.

**Examples:**

```json
// Format as date only
{ "format_date": [{ "datetime": "2024-01-15T14:30:00Z" }, "yyyy-MM-dd"] }
// Result: "2024-01-15"

// Format as US date
{ "format_date": [{ "datetime": "2024-12-25T00:00:00Z" }, "MM/dd/yyyy"] }
// Result: "12/25/2024"

// Get timezone offset (the format must be exactly "z")
{ "format_date": [{ "datetime": "2024-01-01T10:00:00+05:00" }, "z"] }
// Result: "+0500"

// Without a zone argument the other tokens render the UTC instant
{ "format_date": [{ "datetime": "2024-01-01T10:00:00+05:30" }, "HH:mm"] }
// Result: "04:30"

// Wall-clock time plus offset in one string needs the zone argument
{ "format_date": [{ "datetime": "2024-01-01T10:00:00+05:30" }, "HH:mm %z", "Asia/Kolkata"] }
// Result: "10:00 +0530"

// Render an instant as a calendar date in a zone
{ "format_date": [{ "datetime": "2026-08-17T18:30:00Z" }, "dd MMM yyyy", "Asia/Kolkata"] }
// Result: "18 Aug 2026"

// Zone offset at that instant (DST-aware)
{ "format_date": [{ "datetime": "2024-07-15T12:00:00Z" }, "z", "America/New_York"] }
// Result: "-0400"

// Format current time
{ "format_date": [{ "now": [] }, "yyyy-MM-dd"] }
// Result: "2024-01-15" (current date)

// With variable
{ "format_date": [{ "var": "date" }, "dd/MM/yyyy"] }
// Data: { "date": "2024-12-25T00:00:00Z" }
// Result: "25/12/2024"

// An unknown chrono specifier is an error (catchable with try)
{ "format_date": [{ "datetime": "2024-01-01T00:00:00Z" }, "%Q"] }
// Result: error (Invalid date format)
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
- `unit` - Unit of measurement: `"days"`, `"hours"`, `"minutes"`, `"seconds"`, or `"milliseconds"` (lowercase). Any other value is an Invalid Arguments error: `date_diff: unknown unit "weeks" (expected days, hours, minutes, seconds, or milliseconds)`

**Returns:** Difference (`date1 - date2`) as an integer in the specified unit, truncated toward zero; negative when `date1` is earlier than `date2`.

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

// Milliseconds
{ "date_diff": [
    { "datetime": "2024-01-01T00:00:01Z" },
    { "datetime": "2024-01-01T00:00:00Z" },
    "milliseconds"
]}
// Result: 1000

// Negative when the first date is earlier
{ "date_diff": [
    { "datetime": "2024-01-01T00:00:00Z" },
    { "datetime": "2024-01-02T00:00:00Z" },
    "days"
]}
// Result: -1

// Unknown units are an error (catchable with try)
{ "date_diff": [
    { "datetime": "2024-01-02T00:00:00Z" },
    { "datetime": "2024-01-01T00:00:00Z" },
    "weeks"
]}
// Result: error (Invalid Arguments)

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
