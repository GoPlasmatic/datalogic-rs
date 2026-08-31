# Operators Overview

datalogic-rs provides 64 built-in operators organized into logical categories. In the Rust crate, 33 baseline operators are always available in the default build (`default = []`); a further 29 canonical operators are enabled by opt-in Cargo features, and two flagd-compatible operators (`fractional`, `sem_ver`) sit behind the `flagd` feature. Every language binding (WASM, Node, Python, Go, JVM, .NET, PHP) ships with all operator features enabled, so the full set is available out of the box outside Rust. Counts are by canonical operator: `var` and `?:` are accepted as input aliases of `val` and `if`, and `match` is an alias of `switch`, so the aliases are not counted separately. This section documents each operator with syntax, examples, and notes on behavior.

## Operator Categories

| Category | Operators | Description |
|----------|-----------|-------------|
| [Variable Access](variable-access.md) | `val` (alias `var`), `exists` | Access and check data |
| [Comparison](comparison.md) | `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=` | Compare values |
| [Logical](logical.md) | `!`, `!!`, `and`, `or` | Boolean logic |
| [Arithmetic](arithmetic.md) | `+`, `-`, `*`, `/`, `%`, `max`, `min`, `abs`, `ceil`, `floor` | Math operations |
| [Control Flow](control-flow.md) | `if` (alias `?:`), `??`, `switch` (alias `match`), `type` | Conditional branching |
| [String](string.md) | `cat`, `substr`, `in`, `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split` | String manipulation |
| [Array](array.md) | `merge`, `filter`, `map`, `reduce`, `all`, `some`, `none`, `sort`, `slice`, `group_by`, `distinct` | Array operations |
| [Object](object.md) | `keys`, `values`, `entries` | Object take-apart |
| [DateTime](datetime.md) | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` | Date and time |
| [Missing Values](missing.md) | `missing`, `missing_some` | Check for missing data |
| [Error Handling](error-handling.md) | `try`, `throw` | Exception handling |
| [flagd-Compat](flagd.md) | `fractional`, `sem_ver` | Feature-flag targeting (OpenFeature flagd spec); requires `features = ["flagd"]` |

## Which operators need which Cargo feature

This split only affects the **Rust crate**: only the baseline set is built in the default build (`default = []`). A rule that uses any other operator against an engine compiled without its feature still compiles (`Engine::compile` succeeds, because an unknown key is treated like an unregistered custom operator), but evaluating it fails with an `InvalidOperator` error naming the operator. In templating mode an unknown key is echoed as data instead of erroring (see the [API reference](../rust/api-reference.md)). Every language binding enables all operator features, so the full set is always available there.

| Cargo feature | Operators |
|---------------|-----------|
| *baseline* (always on) | `val`/`var`, comparison (`==` … `<=`), `and`, `or`, `!`, `!!`, `if`/`?:`, `+ - * / %`, `min`, `max`, `cat`, `substr`, `in`, `map`, `filter`, `reduce`, `merge`, `all`, `some`, `none`, `missing`, `missing_some` |
| `ext-string` | `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split` |
| `ext-array` | `sort`, `slice`, `group_by`, `distinct` |
| `ext-object` | `keys`, `values`, `entries` |
| `ext-math` | `abs`, `ceil`, `floor` |
| `ext-control` | `exists`, `??`, `switch`/`match`, `type` |
| `error-handling` | `try`, `throw` |
| `datetime` | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` |
| `flagd` | `fractional`, `sem_ver` |

The table above is maintained by hand; the machine-readable source of
truth for a given build is
[`Engine::builtin_operator_names()`](../rust/api-reference.md#introspection-helpers),
which is derived from the compiler's own lookup table and reflects the
compiled feature set (aliases included). Use it instead of a hand-copied
list when tooling needs to know what the running engine evaluates.

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

This matters when a later operand could raise an error, or is expensive:

```json
{
  "and": [
    { "var": "denominator" },
    { "/": [100, { "var": "denominator" }] }
  ]
}
```

If `denominator` is missing or `0`, `and` returns that falsy value and the division (which would throw a `NaN` error for an integer zero divisor) is never evaluated; with `{ "denominator": 4 }` the result is `25`. Note that `var` itself never errors on a missing path, it returns `null`, so plain property access such as `{ "var": "user.profile.name" }` needs no guard.

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

- **Falsy**: `false`, `0`, `""`, `null`, `[]`, `{}`
- **Truthy**: Everything else

## Custom Operators

You can add your own operators. See [Custom Operators](../advanced/custom-operators.md) for details.

In v5 operator registration is builder-only:

```rust
let engine = Engine::builder()
    .add_operator("myop", MyOperator)
    .build();
```

Custom operators follow the same syntax in rules:

```json
{ "myop": [arg1, arg2] }
```

> **Note:** v5 removed the `preserve` operator. Wrap literals in
> templating mode (`Engine::builder().with_templating(true).build()`,
> requires `feature = "templating"`) if you need to emit a JSON object
> verbatim from a rule. Literal scalars and arrays already work inline.

> **Emitting a key that is an operator name.** Because a single-key
> object is an operator invocation, `{"type": {"var": "x"}}` runs the
> `type` operator rather than emitting a `type` field, and the same
> applies to every name on this page plus any custom operator you
> register. In templating mode you can opt into an escape prefix,
> `Engine::builder().with_template_key_escape('$')`, and write
> `{"$type": ...}` to emit `type` (`{"$$type": ...}` emits a literal
> `$type`). See
> [Structured Objects](../advanced/structured-objects.md#emitting-keys-that-are-operator-names).
