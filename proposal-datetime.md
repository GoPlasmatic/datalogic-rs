# DateTime and Duration Support for datalogic-rs

## Overview

This proposal outlines the implementation of datetime and duration support for the datalogic-rs JSONLogic implementation. The goal is to enhance the library's capabilities by adding first-class support for date/time operations, making temporal logic expressions more intuitive and efficient.

## Implementation Details

### 1. Data Types

#### DateTime
- Use `chrono::DateTime<Utc>` as the underlying implementation for datetime values
- Add a new variant to the `DataValue` enum:
```rust
pub enum DataValue<'a> {
    // Existing variants...
    DateTime(chrono::DateTime<Utc>),
}
```

#### Duration
- Use `chrono::Duration` as the underlying implementation for duration/timedelta values
- Add a new variant to the `DataValue` enum:
```rust
pub enum DataValue<'a> {
    // Existing variants...
    DateTime(chrono::DateTime<Utc>),
    Duration(chrono::Duration),
}
```

### 2. Parsing and Conversion

#### DateTime Parsing
- Support converting from string to datetime using the `{"datetime": "2022-07-06T13:20:06Z"}` format
- Add an implementation for the datetime parser that handles ISO 8601 formatted strings
- Support RFC 3339 format by default (e.g., "2022-07-06T13:20:06Z")
- Handle timezone information appropriately, defaulting to UTC for timestamps without timezone info

#### Duration Parsing
- Support converting from string to duration using the `{"timestamp": "1d:2h:3m:4s"}` format
- Implement a custom duration parser that handles day/hour/minute/second components:
  - `d` for days
  - `h` for hours
  - `m` for minutes
  - `s` for seconds
  - Optional millisecond component (e.g., `123ms`)
- Additionally support ISO 8601 duration format (e.g., "P1DT2H3M4S")

### 3. Property Access via Val Operator

Extend the `val` operator to support accessing properties of DateTime and Duration values:

#### DateTime Properties
The following properties should be accessible via the `val` operator:
- `year` - Extract the year component
- `month` - Extract the month component
- `day` - Extract the day component
- `hour` - Extract the hour component
- `minute` - Extract the minute component
- `second` - Extract the second component
- `timestamp` - Convert to Unix timestamp (seconds since epoch)
- `iso` - Format as ISO 8601 string

Example:
```json
{"val": [{"var": "date"}, "year"]}
```

#### Duration Properties
The following properties should be accessible via the `val` operator:
- `days` - Extract days component
- `hours` - Extract hours component
- `minutes` - Extract minutes component
- `seconds` - Extract seconds component
- `total_seconds` - Get total duration in seconds

Example:
```json
{"val": [{"var": "duration"}, "total_seconds"]}
```

### 4. Operator Support

#### Arithmetic Operations
Extend arithmetic operators to support datetime and duration:

| Operation | Left Operand | Right Operand | Result |
|-----------|--------------|---------------|--------|
| `+` | DateTime | Duration | DateTime |
| `-` | DateTime | Duration | DateTime |
| `-` | DateTime | DateTime | Duration |
| `*` | Duration | Number | Duration |
| `/` | Duration | Number | Duration |

#### Comparison Operations
Extend comparison operators to support datetime values:

| Operation | Description |
|-----------|-------------|
| `==`, `===` | Equal |
| `!=`, `!==` | Not Equal |
| `>` | Greater Than (later date) |
| `>=` | Greater Than or Equal |
| `<` | Less Than (earlier date) |
| `<=` | Less Than or Equal |

### 5. New Operators

Add dedicated datetime operators:

#### `now` Operator
Get current date and time:
```json
{"now": []}
```

#### `format_date` Operator
Format a date according to a specified format string:
```json
{"format_date": [{"var": "date"}, "yyyy-MM-dd"]}
```

#### `parse_date` Operator
Parse a string into a date using a specified format:
```json
{"parse_date": ["2022-07-06", "yyyy-MM-dd"]}
```

#### `date_diff` Operator
Calculate the difference between two dates:
```json
{"date_diff": [{"var": "date1"}, {"var": "date2"}, "days"]}
```

### 6. Error Handling

- Add appropriate error types for date/time parsing and operation failures
- Ensure all datetime operations handle invalid input gracefully

## Usage Examples

### Creating DateTime Values
```json
{"datetime": "2022-07-06T13:20:06Z"}
```

### Creating Duration Values
```json
{"timestamp": "1d:2h:3m:4s"}
```

### Arithmetic with DateTime and Duration
```json
{"+": [{"datetime": "2022-07-06T13:20:06Z"}, {"timestamp": "1d"}]}
```

### Comparing Dates
```json
{">": [{"datetime": "2022-07-06T13:20:06Z"}, {"datetime": "2022-07-05T13:20:06Z"}]}
```

### Accessing DateTime Properties
```json
{"val": [{"var": "date"}, "year"]}
```

### Accessing Duration Properties
```json
{"val": [{"var": "duration"}, "days"]}
```

## Implementation Plan

1. Add DateTime and Duration variants to DataValue
2. Implement conversion functions for string to DateTime/Duration
3. Update the DataValue methods (is_datetime, as_datetime, etc.)
4. Extend the `val` operator to support DateTime and Duration property access
5. Extend arithmetic and comparison operators to handle datetime/duration values
6. Implement new datetime-specific operators
7. Add comprehensive tests for all new functionality
8. Update documentation with examples and usage guidance

## Dependencies

- Add `chrono` crate as a dependency in Cargo.toml
- Version requirement: `chrono = "0.4.31"` (or latest stable version)
