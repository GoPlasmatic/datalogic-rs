# Configuration & Errors

Tune evaluation semantics with the `config=` keyword argument and handle
failures through the binding's exception hierarchy.

## Engine Configuration

`Engine(...)` accepts a keyword-only `config=` argument: a `dict` (or a
JSON string) with an optional `"preset"` key plus per-field overrides.
The preset applies first; the remaining keys override individual fields
on top of it. Unknown keys or values raise `EvaluateError` with
`error_type == "ConfigurationError"`.

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | bool |
| `missing_var` | `"null"` (default: a missing variable reads as `None`), `"error"` (raises `VariableNotFound`) |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `None` / `null` for unbounded (caps the work one evaluation may do; crossing it raises `BudgetExceeded`) |

The presets: `"default"` is JSONLogic-compatible behavior;
`"safe_arithmetic"` skips non-numeric operands and returns `None` on
float division by zero (integer/integer division by zero always
raises, whatever `division_by_zero` says); `"strict"` errors on any
type mismatch and disables lenient numeric coercion. See
[Division by Zero](../advanced/configuration.md#division-by-zero) for
the full table.

### Example: Strict Preset with One Override

```python
from datalogic_py import Engine, EvaluateError

# Start from the strict preset, then relax division by zero.
engine = Engine(config={
    "preset": "strict",
    "division_by_zero": "return_null",
})

engine.eval({"/": [1.5, 0]}, {})      # None (the override wins)

try:
    engine.eval({"+": ["abc", 2]}, {})  # strict rejects non-numeric strings
except EvaluateError as e:
    print(e.error_type)                 # "Thrown"
```

Two details: `division_by_zero` governs the float path only, so
`{"/": [1, 0]}` (integer / integer) raises under every setting; and the
strict preset rejects non-numeric strings, `None`, and `""` in
arithmetic, while it still coerces numeric strings such as `"1"`
(`{"+": ["1", 2]}` returns `3`).

A JSON string works anywhere the dict does:
`Engine(config='{"preset": "safe_arithmetic"}')`. Every binding parses
this JSON schema with the same core code, so a config that works here
works in every other binding. Full semantics of each knob, with
behavior tables, are in
[Configuration](../advanced/configuration.md).

### Missing Variables

By default a `var` or `val` read that finds nothing evaluates to `None`,
as JSONLogic specifies. `"missing_var": "error"` raises `VariableNotFound` instead,
so a typo in a path fails where it happens. A default
(`{"var": ["x", 0]}`), a present `null`, `missing`, `missing_some` and
`exists` are not misses, and `try` catches the error:

```python
from datalogic_py import Engine, EvaluateError

engine = Engine(config={"missing_var": "error"})
try:
    engine.eval({"var": "user.nmae"}, {"user": {"name": "Ada"}})
except EvaluateError as e:
    print(e.error_type)   # "VariableNotFound"
engine.eval({"var": ["user.nmae", "anon"]}, {"user": {"name": "Ada"}})  # "anon"
```

### Other Engine Options

| Argument | Effect |
|---|---|
| `templating=True` | Compile multi-key objects as output templates; `compile_template` / `compile_strict` choose the mode per compile |
| `template_key_escape="$"` | A template key starting with this character is a literal output field (`{"$type": ...}` emits `type`) |
| `families=["ExtString", ...]` | Keep the engine to the JSONLogic core plus these operator families (the `family` of each `engine.operators()` row); the others compile as unknown operators |
| `strict_operator_names=True` | Raise `ConfigurationError` for a custom operator named like a built-in instead of registering one that never runs |

## Error Handling

All exceptions raised by the binding descend from `DataLogicError`:

| Exception | When |
|-----------|------|
| `DataLogicError` | Base class; catch this for "anything from datalogic" |
| `ParseError` | Malformed rule or data JSON, or an unsupported Python type in the dict path |
| `EvaluateError` | Everything else the engine reports: runtime operator failures, unknown operators (`"InvalidOperator"`), invalid configuration (`"ConfigurationError"`) |
| `CompileError` | `compile_checked` refused the rule; `.diagnostics` lists every problem as `{code, severity, message, pointer, operator}` dicts |

Every exception carries these attributes:

*   `.error_type`: the engine's stable error tag, e.g. `"Thrown"`, `"TypeError"`, `"InvalidArguments"`, `"InvalidOperator"`, `"VariableNotFound"`, `"BudgetExceeded"`, or `"CompileError"`. The binding adds `"TypeMismatch"` (a typed session evaluation whose result has the wrong type) and `"InvalidArgument"` (for example a rule compiled by a different engine passed to a session).
*   `.operator`: the innermost failing operator name (`"+"`, `"var"`, ...), or `None`.
*   `.node_ids`: a leaf-to-root breadcrumb of compiled-node ids.
*   `.path`: a root-to-leaf list of step dicts, each with `node_id`, `operator`, `arg_index`, and `json_pointer`; `None` when no compiled rule was available to resolve it.

A `BudgetExceeded` error also carries `.budget` and `.spent`.

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
    print(e.path)         # [{"node_id": ..., "operator": "+", ...}]
```

A rule that executes the `throw` operator raises `EvaluateError` with
`.error_type == "Thrown"`; the message carries the thrown payload as
JSON (`{"throw": "boom"}` gives `Thrown: {"type":"boom"}`).

## Type Conversion

The dict-input path (`apply`, `Engine.eval`, `Rule.evaluate`) walks
Python objects straight into the engine's arena representation; it
falls back to [`pythonize`](https://crates.io/crates/pythonize) only
for unusual shapes (subclasses, sets, mappings, out-of-range ints),
with identical results either way.

**Supported:** `dict`, `list`, `tuple`, `str`, `int`, `float`, `bool`,
`None`. The binding converts tuples and sets (`set`, `frozenset`) to
JSON arrays (set order is unspecified).

**Non-finite floats:** `float('nan')` and `float('inf')` have no JSON
encoding, so they become `null` on input and come back as `None` in
results. The same applies to results the engine produces: with
`division_by_zero = "return_infinity"`, `engine.eval({"/": [1.5, 0]}, {})`
returns `None`, indistinguishable from `"return_null"`, and the JSON
string entry points (`eval_str`, `evaluate_str`) serialize it as
`null` too.

**Not supported** (these raise `ParseError`):

*   `datetime.datetime`, `datetime.date`: convert to an ISO string at the Python edge
*   `decimal.Decimal`: convert to `float` or `str`
*   `bytes`, `bytearray`

For payloads with exotic types, use `rule.evaluate_str(json_text)` and
bring your own JSON encoder (e.g. `json.dumps(payload, default=str)`).
