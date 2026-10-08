# Quick Start

Evaluate rules in Python with the `datalogic-py` binding.

## Simple One-Shot Evaluation

Use `apply` for one-off evaluations:

```python
from datalogic_py import apply

# Arithmetic
result = apply({"+": [1, 2, 3]}, {})
print(result) # 6

# Variable Access
result = apply(
    {"var": "user.age"},
    {"user": {"age": 25}}
)
print(result) # 25
```

## Reusable Compiled Rules

For production loops, compile the rule once. The engine parses the rule a single time into a compiled node tree, so each evaluation skips parsing:

```python
from datalogic_py import Engine

engine = Engine()

# 1. Compile once
rule = engine.compile({"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]})

# 2. Evaluate many times
for user in [{"score": 75}, {"score": 30}, {"score": 90}]:
    print(rule.evaluate(user)) # prints "pass", "fail", "pass"
```

## Catching Rule Mistakes Before They Run

`compile` accepts a rule that calls an operator the engine doesn't have; the call fails when it runs. `compile_checked` refuses such a rule up front and lists every problem, each located by a JSON Pointer into the rule:

```python
from datalogic_py import CompileError, Engine

engine = Engine()
try:
    engine.compile_checked({"if": [True, {"vr": "x"}, "no"]})
except CompileError as e:
    for d in e.diagnostics:
        print(d["pointer"], d["message"])  # /if/1 unknown operator `vr`; did you mean `var`?
```

`engine.check(rule)` returns the same list without compiling. [API & GIL Management](api-gil.md) covers the other introspection calls.

## Parsing Performance: `evaluate` vs `evaluate_str`

*   `rule.evaluate(dict_data)` accepts a Python `dict` or `list` and walks the Python objects straight into the engine's arena (it falls back to `pythonize` only for unusual types such as subclasses and sets). In the boundary benchmark this is about 3 to 6 times faster than a `json.dumps` → `evaluate_str` → `json.loads` round trip.
*   `rule.evaluate_str(json_string)` accepts a raw JSON string. If you already have a serialized JSON payload (e.g. read from a network socket or file), use this method to skip Python-to-Rust dictionary marshaling.

Next: [Configuration & Errors](configuration-and-errors.md) covers engine configuration presets, the exception hierarchy, and type conversion.
