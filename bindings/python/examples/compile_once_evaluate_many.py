"""compile-once-evaluate-many: compile a rule once, evaluate ~100k payloads.

Prints the last result and a rough per-evaluation cost for each tier —
dict payloads, then pre-parsed data handles (the hot path: zero parse
work per call), and finally the whole set as one batch call.

Run from bindings/python/ (build first: maturin develop --release):
    python examples/compile_once_evaluate_many.py
"""

import time

from datalogic_py import DataHandle, Engine

ITERATIONS = 100_000

engine = Engine()
rule = engine.compile({"*": [{"var": "price"}, {"-": [1, {"var": "discount"}]}]})

# Tier 1 — compiled rule, dict data (converted per call).
last = None
start = time.perf_counter_ns()
for i in range(ITERATIONS):
    last = rule.evaluate({"price": 100 + i % 100, "discount": 0.2})
elapsed_ns = time.perf_counter_ns() - start
print(f"dict data:    last result {last}, {ITERATIONS} evaluations, ~{elapsed_ns // ITERATIONS} ns/op")

# Tier 2 — session + pre-parsed data handles: parse each distinct
# payload once, then every evaluation skips conversion entirely.
handles = [
    DataHandle('{"price": %d, "discount": 0.2}' % (100 + i)) for i in range(100)
]
session = engine.session()

start = time.perf_counter_ns()
for i in range(ITERATIONS):
    last = session.evaluate_data(rule, handles[i % 100])
elapsed_ns = time.perf_counter_ns() - start
print(f"data handles: last result {last}, {ITERATIONS} evaluations, ~{elapsed_ns // ITERATIONS} ns/op")

# Tier 3 — one native call for the whole set: per-item results (and
# per-item BatchItemError values) come back in order.
results = session.evaluate_batch(rule, handles)
print(f"batch:        {len(results)} results in one call, first {results[0]}, last {results[-1]}")
