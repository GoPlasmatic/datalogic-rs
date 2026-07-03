"""getting-started: one-shot JSONLogic evaluation with datalogic-py,
plus the typed-result tier for boolean predicates.

Run from bindings/python/ (build first: maturin develop --release):
    python examples/getting_started.py
"""

from datalogic_py import DataHandle, Engine, apply

rule = {
    "and": [
        {">=": [{"var": "age"}, 18]},
        {"==": [{"var": "status"}, "active"]},
    ]
}
data = {"age": 25, "status": "active"}

# One-shot: compile + evaluate in a single call.
print(apply(rule, data))  # True

# Typed result: for predicates, skip the JSON round trip entirely.
# Compile the rule, parse the data once into a handle, and read the
# result directly as a Python bool.
engine = Engine()
compiled = engine.compile(rule)
parsed = DataHandle('{"age": 25, "status": "active"}')

session = engine.session()
print(session.evaluate_bool(compiled, parsed))  # True
