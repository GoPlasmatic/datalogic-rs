# Configuration & Errors

Tune evaluation semantics with the `config=` keyword argument and handle
failures through the binding's exception hierarchy.

## Engine Configuration

`Engine(...)` accepts a keyword-only `config=` argument: a `dict` (or a
JSON string) with an optional `"preset"` key plus per-field overrides.
The preset applies first; the remaining keys override individual fields
on top of it. Unknown keys or values raise `EvaluateError`, so typos
fail loudly instead of being silently ignored.

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
`"safe_arithmetic"` skips non-numeric operands and returns `None` on
division by zero; `"strict"` errors on any type mismatch and disables
numeric coercion.

### Example: Strict Preset with One Override

```python
from datalogic_py import Engine, EvaluateError

# Start from the strict preset, then relax division by zero.
engine = Engine(config={
    "preset": "strict",
    "division_by_zero": "return_null",
})

engine.eval({"/": [1, 0]}, {})        # None (the override wins)

try:
    engine.eval({"+": ["1", 2]}, {})  # strict does not coerce "1" to a number
except EvaluateError as e:
    print(e.error_type)
```

A JSON string works anywhere the dict does:
`Engine(config='{"preset": "safe_arithmetic"}')`. Every binding shares
this JSON schema and parses it with the same core code, so a config that
works here works in the WASM, Node, and Go bindings too. Full semantics
of each knob, with behavior tables, are in
[Configuration](../advanced/configuration.md).

## Error Handling

All exceptions raised by the binding descend from `DataLogicError`:

| Exception | When |
|-----------|------|
| `DataLogicError` | Base class; catch this for "anything from datalogic" |
| `ParseError` | Malformed rule or data JSON, or an unsupported Python type in the dict path |
| `EvaluateError` | Everything else the engine reports: runtime operator failures, unknown operators (`"InvalidOperator"`), invalid configuration (`"ConfigurationError"`) |

`EvaluateError` carries structured attributes:

*   `.error_type`: the engine's stable error tag, e.g. `"Thrown"`, `"TypeError"`, `"InvalidArguments"`, `"InvalidOperator"`.
*   `.operator`: the outermost failing operator name (`"+"`, `"var"`, ...), or `None`.
*   `.node_ids`: a leaf-to-root breadcrumb of compiled-node ids.
*   `.path`: a root-to-leaf list of step dicts, each with `node_id`, `operator`, `arg_index`, and `json_pointer`; `None` when no compiled rule was available to resolve it.

### Parse Failures vs. Evaluate Failures

```python
from datalogic_py import Engine, ParseError, EvaluateError

engine = Engine()

try:
    engine.compile('{"var": ')            # truncated JSON
except ParseError as e:
    print(f"bad rule: {e}")

rule = engine.compile({"+": [{"var": "x"}, 1]})
try:
    rule.evaluate({"x": "not a number"})
except EvaluateError as e:
    print(e.error_type)   # "Thrown" (NaN under the default config)
    print(e.operator)     # "+"
    print(e.path)         # [{"node_id": ..., "operator": "+", ...}, ...]
```

A rule that executes the `throw` operator raises `EvaluateError` with
`.error_type == "Thrown"`; the thrown payload is serialized into the
exception message as `Thrown: <payload JSON>`.

## Type Conversion

The dict-input path (`apply`, `Engine.eval`, `Rule.evaluate`) converts
Python values with [`pythonize`](https://crates.io/crates/pythonize):

**Supported:** `dict`, `list`, `str`, `int`, `float`, `bool`, `None`.

**Not supported**, these raise `ParseError` with a clear message:

*   `datetime.datetime`, `datetime.date`: convert to an ISO string at the Python edge
*   `decimal.Decimal`: convert to `float` or `str`
*   `bytes`, `set`, `tuple`
*   `float('nan')`, `float('inf')`: the JSON spec disallows them

For payloads with exotic types, use `rule.evaluate_str(json_text)` and
bring your own JSON encoder (e.g. `json.dumps(payload, default=str)`).
