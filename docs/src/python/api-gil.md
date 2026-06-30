# API & GIL Management

`datalogic-py` provides context managers for arena recycling and releases the Global Interpreter Lock (GIL) to enable true parallelism.

## Session Lifecycle (Context Manager)

For tight loops, use the `session()` context manager. It manages a reusable memory arena and automatically resets it between iterations.

```python
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({"+": [{"var": "x"}, 1]})

data_items = [{"x": 1}, {"x": 2}, {"x": 3}]

with engine.session() as session:
    for item in data_items:
        # Reuses the same internal memory buffer, avoiding allocations
        result = session.evaluate(rule, item)
        print(result)
```

## Global Interpreter Lock (GIL) Release

Python's multi-threading is typically limited by the Global Interpreter Lock (GIL). However, `datalogic-py` releases the GIL during the compilation and evaluation phases.

*   **Parallel execution:** If you run `rule.evaluate` inside a `ThreadPoolExecutor` or standard Python `threading.Thread`, multiple evaluations will run concurrently on separate CPU cores inside the Rust engine.
*   **Best Practice:** Share a single `Engine` and compiled `Rule` across all threads. Keep `Session` objects thread-local (one per thread).

```python
import concurrent.futures
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({">=": [{"var": "age"}, 18]})

users = [{"age": 20}, {"age": 15}, {"age": 32}, {"age": 12}]

# Evaluates concurrently across OS threads, bypassing Python's GIL
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
    results = list(executor.map(rule.evaluate, users))

print(results) # [True, False, True, False]
```

## Error Handling

All runtime exceptions in the Python binding inherit from `DataLogicError`. There are two main subclasses:
*   `ParseError`: Raised when rules or input datasets are malformed, or if an unsupported Python type (e.g. `set` or `tuple`) is provided.
*   `EvaluateError`: Raised during evaluation. Exposes `.error_type`, `.operator`, and `.path` (a JSON pointer to the failing expression).

```python
from datalogic_py import Engine, EvaluateError

engine = Engine()
try:
    engine.eval({"var": "missing_variable"}, {})
except EvaluateError as e:
    print(f"Error: {e.error_type}")  # e.g., "VariableNotFound"
    print(f"Failed at: {e.operator}") # "var"
    print(f"Path: {e.path}")
```
