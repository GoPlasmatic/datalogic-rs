# Quick Start

Evaluate rules instantly in Python using the `datalogic-py` binding.

## Simple One-Shot Evaluation

Use `apply` for simple, one-off evaluations:

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

For production loops, compile the rule once. This eliminates parsing overhead and parses the rule directly into the optimized Rust bytecode:

```python
from datalogic_py import Engine

engine = Engine()

# 1. Compile once
rule = engine.compile({"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]})

# 2. Evaluate many times
for user in [{"score": 75}, {"score": 30}, {"score": 90}]:
    print(rule.evaluate(user)) # prints "pass", "fail", "pass"
```

## Parsing Performance: `evaluate` vs `evaluate_str`

*   `rule.evaluate(dict_data)` accepts a Python `dict` or `list` and converts it directly into Rust types using `pythonize`. This is 3–10× faster than a standard JSON-string round-trip.
*   `rule.evaluate_str(json_string)` accepts a raw JSON string. If you already have a serialized JSON payload (e.g. read from a network socket or file), use this method to bypass Python-to-Rust dictionary marshaling completely.
