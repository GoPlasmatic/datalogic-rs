# Operators Overview

datalogic-rs provides 59 built-in operators organized into logical categories. This section documents each operator with syntax, examples, and notes on behavior.

## Operator Categories

| Category | Operators | Description |
|----------|-----------|-------------|
| [Variable Access](variable-access.md) | `var`, `val`, `exists` | Access and check data |
| [Comparison](comparison.md) | `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=` | Compare values |
| [Logical](logical.md) | `!`, `!!`, `and`, `or` | Boolean logic |
| [Arithmetic](arithmetic.md) | `+`, `-`, `*`, `/`, `%`, `max`, `min`, `abs`, `ceil`, `floor` | Math operations |
| [Control Flow](control-flow.md) | `if`, `?:`, `??` | Conditional branching |
| [String](string.md) | `cat`, `substr`, `in`, `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split` | String manipulation |
| [Array](array.md) | `merge`, `filter`, `map`, `reduce`, `all`, `some`, `none`, `sort`, `slice` | Array operations |
| [DateTime](datetime.md) | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` | Date and time |
| [Missing Values](missing.md) | `missing`, `missing_some` | Check for missing data |
| [Error Handling](error-handling.md) | `try`, `throw` | Exception handling |

## Operator Syntax

All operators follow the JSONLogic format:

```json
{ "operator": [arg1, arg2, ...] }
```

Some operators accept a single argument without an array:

```json
{ "var": "name" }
// Equivalent to:
{ "var": ["name"] }
```

## Lazy Evaluation

Several operators use lazy (short-circuit) evaluation:

- **`and`**: Stops at first falsy value
- **`or`**: Stops at first truthy value
- **`if`**: Only evaluates the matching branch
- **`?:`**: Only evaluates the matching branch
- **`??`**: Only evaluates fallback if first value is null

This is important when operations have side effects or when you want to avoid errors:

```json
{
  "and": [
    { "var": "user" },
    { "var": "user.profile.name" }
  ]
}
```

If `user` is null, the second condition is never evaluated, avoiding an error.

## Type Coercion

Operators handle types differently:

### Loose vs Strict

- `==` and `!=` perform type coercion
- `===` and `!==` require exact type match

```json
{ "==": [1, "1"] }   // true (loose)
{ "===": [1, "1"] }  // false (strict)
```

### Numeric Coercion

Arithmetic operators attempt to convert values to numbers:

```json
{ "+": ["5", 3] }  // 8 (string "5" becomes number 5)
```

### Truthiness

Boolean operators use configurable truthiness rules. By default (JavaScript-style):

- **Falsy**: `false`, `0`, `""`, `null`, `[]`
- **Truthy**: Everything else

## Custom Operators

You can add your own operators. See [Custom Operators](../advanced/custom-operators.md) for details.

```rust
engine.add_operator("myop".to_string(), Box::new(MyOperator));
```

Custom operators follow the same syntax:

```json
{ "myop": [arg1, arg2] }
```
