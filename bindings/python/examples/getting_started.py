"""getting-started: one-shot JSONLogic evaluation with datalogic-py.

Run from bindings/python/ (build first: maturin develop --release):
    python examples/getting_started.py
"""

from datalogic_py import apply

rule = {
    "and": [
        {">=": [{"var": "age"}, 18]},
        {"==": [{"var": "status"}, "active"]},
    ]
}
data = {"age": 25, "status": "active"}

print(apply(rule, data))  # True
