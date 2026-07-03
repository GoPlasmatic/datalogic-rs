"""custom-operator: register a Python `double` operator and call it from a rule.

Custom operators receive their pre-evaluated arguments as a JSON-array string
and return a JSON-value string; a raised exception becomes an evaluation
error for the caller. Built-in operator names always win.

Run from bindings/python/ (build first: maturin develop --release):
    python examples/custom_operator.py
"""

import json

from datalogic_py import Engine, EvaluateError


def double(args_json: str) -> str:
    args = json.loads(args_json)
    if not args:
        raise ValueError("double expects one numeric argument")
    return json.dumps(args[0] * 2)


engine = Engine(custom_operators={"double": double})

print(engine.eval({"double": [21]}, {}))  # 42

# Custom operators compose with built-ins.
print(engine.eval({"map": [{"var": "xs"}, {"double": [{"var": ""}]}]}, {"xs": [1, 2, 3]}))  # [2, 4, 6]

# The operator's error path surfaces as a regular EvaluateError.
try:
    engine.eval({"double": []}, {})
except EvaluateError as e:
    print(e)  # custom operator 'double' raised: ... double expects one numeric argument
