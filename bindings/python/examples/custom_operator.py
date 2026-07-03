"""custom-operator: register a Python `double` operator and call it from a rule.

Custom operators receive their pre-evaluated arguments as a JSON-array string
and return a JSON-value string. Built-in operator names always win.

Run from bindings/python/ (build first: maturin develop --release):
    python examples/custom_operator.py
"""

import json

from datalogic_py import Engine

engine = Engine(
    custom_operators={
        "double": lambda args_json: json.dumps(json.loads(args_json)[0] * 2),
    }
)

print(engine.eval({"double": [21]}, {}))  # 42
