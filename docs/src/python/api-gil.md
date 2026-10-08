# API & GIL Management

`datalogic-py` gives you a session context manager for arena reuse, and it releases the Global Interpreter Lock (GIL) while the Rust engine works, so threads evaluate in parallel.

## Session Lifecycle (Context Manager)

For tight loops, use the `session()` context manager. The session keeps one memory arena and resets it at the start of each evaluate call.

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

The binding releases the GIL while the engine evaluates, compiles a rule, runs `check`, or parses the JSON text you pass to `DataHandle`. It holds the GIL while it converts Python values to and from the engine's arena, and while a custom-operator callback runs.

*   **Parallel execution:** If you run `rule.evaluate` inside a `ThreadPoolExecutor` or a `threading.Thread`, the evaluations run concurrently on separate CPU cores inside the Rust engine.
*   **Sharing:** Share one `Engine` and the compiled `Rule` objects across all threads. Keep `Session` objects thread-local (one per thread): only the thread that opened a session may call it.

```python
import concurrent.futures
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({">=": [{"var": "age"}, 18]})

users = [{"age": 20}, {"age": 15}, {"age": 32}, {"age": 12}]

# The engine runs these evaluations on several OS threads at once
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
    results = list(executor.map(rule.evaluate, users))

print(results) # [True, False, True, False]
```

## Data Handles, Typed Results, and Batch Evaluation

The session tier has three faster entry points on top of `evaluate`,
all taking a pre-parsed `DataHandle` instead of a dict or JSON string:

| Tier | Entry point | Use when |
|------|-------------|----------|
| Data handle | `DataHandle(json)` then `rule.evaluate_data(data)` / `session.evaluate_data(rule, data)` | You evaluate the same payload many times: parse once, zero parse work per call |
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
the wrong type (a datetime or duration result counts as a string);
`evaluate_truthy` coerces any result through the engine's truthiness
rules and never mismatches. `Rule.evaluate_data_str` and
`Session.evaluate_data_str` return the result as a JSON string.

## Checking and Describing Rules

These `Engine` and `Rule` methods inspect rules without evaluating them ([Rule Analysis](../advanced/rule-analysis.md) covers the diagnostics and facts in depth):

| Call | Returns |
|------|---------|
| `engine.check(rule, mode=None)` | Every problem the engine can see, as `{code, severity, message, pointer, operator}` dicts; `mode` is `"engine"`, `"strict"` or `"template"` |
| `engine.compile_checked(rule)` | A `Rule`, or raises `CompileError` (`.diagnostics`) when `check` finds an error |
| `engine.compile_template(rule)` / `engine.compile_strict(rule)` | A `Rule` compiled in templating mode, or outside it, whatever the engine's mode |
| `rule.facts()` | What the rule reads (`reads`, each path as segments), which operators it calls, and whether it is `deterministic` |
| `engine.operators()` | One dict per built-in operator the engine has: `name`, `aliases`, `family`, `min_args`, `max_args`, ... |
| `engine.truthy(value)` | Whether a value is truthy under the engine's configured rules; a `str` is JSON text |

```python
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({"+": [{"var": "a.b"}, {"var": "c"}]})
rule.facts()["reads"]       # [["a", "b"], ["c"]]
engine.truthy({})           # False (an empty dict is falsy, like [])
```

Two more `Engine` features: `Engine(custom_operators={"name": fn})`
registers host-language operators (each callable receives the
pre-evaluated arguments as a JSON-array string and returns a JSON
string), and `engine.evaluate_with_trace(logic_json, data_json)`
returns the step-by-step trace envelope the React debugger consumes.
`datalogic_py.__version__` reports the installed version. Full
reference: [Python README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/python#data-handles-typed-results-and-batch-evaluation).

## Error Handling

Every exception the binding raises descends from `DataLogicError` and
carries `.error_type`, `.operator` (the innermost failing operator),
`.node_ids` and `.path`. `ParseError` covers malformed JSON and
unsupported Python types, `EvaluateError` covers evaluation and
configuration failures, and `CompileError` covers rules
`compile_checked` refuses. [Configuration & Errors](configuration-and-errors.md#error-handling)
lists the tags and attributes.

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
