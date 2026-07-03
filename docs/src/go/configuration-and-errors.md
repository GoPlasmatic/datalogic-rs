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
*   `b.AddOperator(name, fn)`: register a custom operator.
*   `b.Build()`: consume the builder and return the configured `*Engine`.

`SetConfigJSON` parses the same JSON wire format every binding uses. All
keys are optional; `"preset"` picks the starting point and the remaining
keys override individual fields on top of it. Unknown keys, unknown enum
strings, and type mismatches return a `*datalogic.Error` with
`Type == "ConfigurationError"`, so typos fail loudly. Each call replaces
the builder's entire evaluation config; templating and registered
operators are unaffected.

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | bool |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

The presets: `"default"` is JSONLogic-compatible behavior;
`"safe_arithmetic"` skips non-numeric operands and returns `null` on
division by zero; `"strict"` errors on any type mismatch and disables
numeric coercion.

### Example: Strict Preset with One Override

```go
b := datalogic.NewEngineBuilder()
if err := b.SetConfigJSON(`{"preset": "strict", "division_by_zero": "return_null"}`); err != nil {
    log.Fatal(err) // typos in keys or values fail here, not silently
}
engine, err := b.Build()
if err != nil {
    log.Fatal(err)
}
defer engine.Close()

out, _ := engine.Apply(`{"/": [1, 0]}`, `{}`)    // "null" (the override wins)
_, err = engine.Apply(`{"+": [null, 1]}`, `{}`)  // err != nil: strict rejects non-numeric operands
```

Builders are not goroutine-safe: construct and `Build()` on one
goroutine, then share the resulting `Engine` freely (see
[Concurrency & Sessions](concurrency.md)). Full semantics of each knob,
with behavior tables, are in
[Configuration](../advanced/configuration.md).

## Error Handling

Every fallible operation returns a `*datalogic.Error` on failure:

| Field | Contents |
|---|---|
| `Message` | Human-readable error string |
| `Type` | The engine's stable error tag; match on this for programmatic handling |
| `Operator` | Outermost failing operator name (`"+"`, `"var"`, ...); empty when the failure didn't originate inside a named operator |
| `PathJSON` | JSON array string of `{node_id, operator, arg_index, json_pointer}` steps from the rule root to the failing node; empty when no compiled rule was in scope |

`Error()` formats as `datalogic: <Type>: <Message>`. The stable tags:
`ParseError`, `Thrown`, `TypeError`, `InvalidArguments`,
`InvalidOperator`, `VariableNotFound`, `ArithmeticError`, `Custom`,
`FormatError`, `IndexOutOfBounds`, `InvalidContextLevel`,
`ConfigurationError`. Arithmetic NaN failures and the rule-level `throw`
operator both surface as `"Thrown"`, with the thrown payload serialized
into `Message`.

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
    fmt.Println(dlErr.PathJSON) // [{"node_id":...,"operator":"+",...}]
}
```

One exception to the pattern: `TracedSession.Evaluate` reports rule
parse and evaluation failures inside its returned JSON envelope (the
`error` and `structured_error` fields), with the Go error return
reserved for binding-level failures such as invalid handles.
