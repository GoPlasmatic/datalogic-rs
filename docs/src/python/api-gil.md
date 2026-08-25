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

Python's multi-threading is typically limited by the Global Interpreter Lock (GIL). However, `datalogic-py` releases the GIL during the evaluation phase.

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

## Data Handles, Typed Results, and Batch Evaluation

The session tier has three faster entry points on top of `evaluate`,
all taking a pre-parsed `DataHandle` instead of a dict or JSON string:

| Tier | Entry point | Use when |
|------|-------------|----------|
| Data handle | `DataHandle(json)` then `rule.evaluate_data(data)` / `session.evaluate_data(rule, data)` | The same payload is evaluated many times: parse once, zero parse work per call |
| Typed | `session.evaluate_bool/int/float/truthy(rule, data)` | Predicates and scalar results, no result conversion on the way out |
| Batch | `session.evaluate_batch(rule, datas)` / `session.evaluate_many(rules, data)` | Many evaluations per native call, with per-item errors |

```python
from datalogic_py import BatchItemError, DataHandle, Engine

engine = Engine()
rule = engine.compile({">=": [{"var": "age"}, 18]})
data = DataHandle('{"age": 25}')          # immutable, thread-safe, engine-independent

rule.evaluate_data(data)                  # True
with engine.session() as session:
    session.evaluate_bool(rule, data)     # True (strict JSON boolean)
    for r in session.evaluate_batch(rule, [data, DataHandle('{"age": 12}')]):
        if isinstance(r, BatchItemError):  # per-item failure, never raised
            print(r.tag, r.message)
        else:
            print(r)                      # "true", then "false" (JSON strings)
```

`evaluate_bool`, `evaluate_int`, and `evaluate_float` raise
`EvaluateError` with `.error_type == "TypeMismatch"` when the result has
the wrong type; `evaluate_truthy` coerces any result through the engine's
truthiness rules and never mismatches. `Rule.evaluate_data_str` and
`Session.evaluate_data_str` return the result as a JSON string.

Two more `Engine` entry points: `Engine(custom_operators={"name": fn})`
registers host-language operators (each callable receives the
pre-evaluated arguments as a JSON-array string and returns a JSON
string), and `engine.evaluate_with_trace(logic_json, data_json)`
returns the step-by-step trace envelope the React debugger consumes.
`datalogic_py.__version__` reports the installed version. Full
reference: [Python README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/python#data-handles-typed-results-and-batch-evaluation).

## Error Handling

All runtime exceptions in the Python binding inherit from `DataLogicError`. There are two main subclasses:
*   `ParseError`: Raised when rules or input datasets are malformed, or if an unsupported Python type (e.g. `bytes`, `datetime`, or `Decimal`) is provided. Tuples and sets are accepted and converted to JSON arrays.
*   `EvaluateError`: Raised during evaluation. Exposes `.error_type`, `.operator`, `.node_ids` (a list of compiled-node ids forming a leaf-to-root breadcrumb), and `.path` (a list of step dicts, each with `node_id`, `operator`, `arg_index`, and `json_pointer`). Two tags come from the binding rather than the engine: `"TypeMismatch"` (a typed session evaluation whose result has the wrong type) and `"InvalidArgument"` (e.g. a rule compiled by a different engine passed to a session).

```python
from datalogic_py import Engine, EvaluateError

engine = Engine()
try:
    engine.eval({"+": ["x", 1]}, {})  # adding a non-numeric string raises
except EvaluateError as e:
    print(f"Error: {e.error_type}")   # "Thrown" (NaN under the default config)
    print(f"Failed at: {e.operator}") # "+"
    print(f"Path: {e.path}")          # list of step dicts
```
