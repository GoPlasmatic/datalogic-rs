"""compile-once-evaluate-many: compile a rule once, evaluate ~100k payloads.

Prints the last result and a rough per-evaluation cost.

Run from bindings/python/ (build first: maturin develop --release):
    python examples/compile_once_evaluate_many.py
"""

import time

from datalogic_py import Engine

ITERATIONS = 100_000

engine = Engine()
rule = engine.compile({"*": [{"var": "price"}, {"-": [1, {"var": "discount"}]}]})

last = None
start = time.perf_counter_ns()
for i in range(ITERATIONS):
    last = rule.evaluate({"price": 100 + i % 100, "discount": 0.2})
elapsed_ns = time.perf_counter_ns() - start

print(f"last result: {last}")
print(f"{ITERATIONS} evaluations, ~{elapsed_ns // ITERATIONS} ns/op")
