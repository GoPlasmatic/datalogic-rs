# Configuration & Errors

Configure evaluation semantics through the engine builder and handle
failures through the `*datalogic.Error` type.

## Engine Configuration

`datalogic.NewEngine()` returns an engine with default configuration;
`datalogic.NewTemplatingEngine()` returns one with templating mode
enabled. Everything else goes through the builder:

*   `datalogic.NewEngineBuilder()`: create a fresh builder.
*   `b.SetConfigJSON(configJSON)`: set the evaluation configuration from a JSON object string; returns an `error` on invalid config.
*   `b.Templating(on)`: toggle templating mode.
*   `b.TemplateKeyEscape(r)`: a template key starting with rune `r` becomes a literal output field (with `'$'`, `{"$type": ...}` emits `type`).
*   `b.Families(names...)`: keep the engine to the JSONLogic core plus the operator families named (see [Operator Families](#operator-families)).
*   `b.StrictOperatorNames(on)`: make `Build` refuse a custom operator named like a built-in, instead of registering one that never runs. Call it before `AddOperator`.
*   `b.AddOperator(name, fn)`: register a custom operator.
*   `b.Build()`: consume the builder and return the configured `*Engine`, or the first error a setter recorded.

`SetConfigJSON` parses the same JSON wire format every binding uses. All
keys are optional; `"preset"` picks the starting point and the remaining
keys override individual fields on top of it. Unknown keys, unknown enum
strings, and type mismatches return a `*datalogic.Error` with
`Type == "ConfigurationError"`. Each call replaces the builder's entire
evaluation config; templating and registered operators are unaffected.

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | bool |
| `missing_var` | `"null"` (default: a missing variable reads as `null`), `"error"` (fails with `VariableNotFound`) |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `null` for unbounded (caps the work one evaluation may do; crossing it fails with `BudgetExceeded`) |

The presets: `"default"` is JSONLogic-compatible behavior;
`"safe_arithmetic"` skips non-numeric operands and returns `null` on
float division by zero (integer/integer division by zero always
errors, whatever `division_by_zero` says); `"strict"` errors on any
type mismatch and disables lenient numeric coercion. See
[Division by Zero](../advanced/configuration.md#division-by-zero) for
the full table.

### Example: Strict Preset with One Override

```go
b := datalogic.NewEngineBuilder()
if err := b.SetConfigJSON(`{"preset": "strict", "division_by_zero": "return_null"}`); err != nil {
    log.Fatal(err) // typos in keys or values fail here
}
engine, err := b.Build()
if err != nil {
    log.Fatal(err)
}
defer engine.Close()

out, _ := engine.Apply(`{"/": [1.5, 0]}`, `{}`)  // "null" (the override wins)
_, err = engine.Apply(`{"+": [null, 1]}`, `{}`)  // err != nil: strict rejects non-numeric operands
```

`division_by_zero` governs the float path only: `{"/": [1, 0]}`
(integer / integer) returns an error under every setting. The strict
preset rejects non-numeric strings, `null`, and `""` in arithmetic;
it still coerces numeric strings such as `"1"`, so
`{"+": ["1", 2]}` returns `"3"`.

`"missing_var": "error"` makes a `var` or `val` read that finds nothing
fail with `Type == "VariableNotFound"` instead of reading as `null`. A
default (`{"var": ["x", 0]}`), a present `null`, `missing`,
`missing_some` and `exists` are not misses, and `try` catches the
error.

`"ops_budget"` caps the operations any evaluation on the engine may
charge; `session.EvaluateMetered(rule, data, budget)` sets a budget for
one call and returns the count (`budget` 0 means the engine's own).

### Operator Families

`b.Families("ExtString", "DateTime")` keeps the engine to the JSONLogic
core plus the families named, using the `family` names
`engine.Operators()` reports: `ExtString`, `ExtArray`, `ExtObject`,
`ExtMath`, `ExtControl`, `ErrorHandling`, `DateTime`, `Tensor`,
`Flagd`. By default an engine has every family. A family left out is
not there for that engine: its names compile as unknown operators,
which fail at evaluation with `InvalidOperator` (and as errors in
`Check` and `CompileChecked`), and a custom operator may take them. An
unknown family name fails `Build` with `Type == "ConfigurationError"`.

Builders are not goroutine-safe: construct and `Build()` on one
goroutine, then share the resulting `Engine` freely (see
[Concurrency & Sessions](concurrency.md)). A builder you drop without
calling `Build` is freed by its finalizer. Full semantics of each
knob, with behavior tables, are in
[Configuration](../advanced/configuration.md).

## Error Handling

Every fallible operation returns a `*datalogic.Error` on failure:

| Field | Contents |
|---|---|
| `Message` | Human-readable error string |
| `Type` | The engine's stable error tag; match on this for programmatic handling |
| `Operator` | Innermost failing operator name (`"+"`, `"var"`, ...); empty when the failure didn't originate inside a named operator |
| `PathJSON` | JSON array string of `{node_id, operator, arg_index, json_pointer}` steps from the rule root to the failing node; empty when no compiled rule was in scope |
| `NodeIDsJSON` | The compiled-node breadcrumb, leaf to root, as a JSON array of ids |
| `DiagnosticsJSON` | `Type == "CompileError"` only: every problem `CompileChecked` found, as `{code, severity, message, pointer, operator}` objects |

`Error()` formats as `datalogic: <Type>: <Message>`. The engine's tags:
`ParseError`, `Thrown`, `TypeError`, `InvalidArguments`,
`InvalidOperator`, `VariableNotFound`, `ArithmeticError`, `Custom`,
`FormatError`, `IndexOutOfBounds`, `InvalidContextLevel`,
`ConfigurationError`, `BudgetExceeded`. Arithmetic NaN failures and
the rule-level `throw` operator both surface as `"Thrown"`, with the
thrown payload serialized into `Message`. Four more tags come from the
binding layer rather than the engine: `TypeMismatch` (a typed
`Session` evaluation such as `EvaluateBool` whose result has the wrong
type), `InvalidArgument` (nil or closed handles, a rule compiled by a
different engine than the session's), `CompileError` (a rule
`CompileChecked` refused), and `InternalError` (a panic caught at the
FFI boundary, or a batch item whose error JSON could not be decoded).

### Compile Failures vs. Evaluate Failures

`engine.Compile` fails with `Type == "ParseError"` on malformed rule
JSON; `Operator` and `PathJSON` are empty because no compiled rule
exists yet. `rule.Evaluate` and `session.Evaluate` fail with runtime
tags and populate the full struct. Use `errors.As` to get the typed
error:

```go
rule, err := engine.Compile(`{"+": [{"var": "x"}, 1]}`)
if err != nil {
    var dlErr *datalogic.Error
    if errors.As(err, &dlErr) && dlErr.Type == "ParseError" {
        log.Fatalf("bad rule: %s", dlErr.Message)
    }
}
defer rule.Close()

_, err = rule.Evaluate(`{"x": "not a number"}`)
var dlErr *datalogic.Error
if errors.As(err, &dlErr) {
    fmt.Println(dlErr.Type)     // "Thrown" (NaN under the default config)
    fmt.Println(dlErr.Operator) // "+"
    fmt.Println(dlErr.PathJSON) // [{"arg_index":null,"json_pointer":"","node_id":...,"operator":"+"}]
}
```

One exception to the pattern: `TracedSession.Evaluate` reports rule
parse and evaluation failures inside its returned JSON envelope (the
`error` and `structured_error` fields), with the Go error return
reserved for binding-level failures such as invalid handles.
