# datalogic-py

[![PyPI](https://img.shields.io/pypi/v/datalogic-py.svg)](https://pypi.org/project/datalogic-py/)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs) — one engine, every runtime.

Python bindings for [`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs),
a fast Rust implementation of [JSONLogic](http://jsonlogic.com). Same
rules, same semantics as the Rust crate, with the **compile-once /
evaluate-many** pattern exposed natively — compile a rule once and
evaluate it against thousands of data inputs without re-parsing. Every
binding runs the same core and passes the same 1,532-case conformance
battery (53 suites).

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** `datalogic-py` is new — there is no v4 Python package.
> If you were calling the v4 Rust crate or the v4 `@goplasmatic/datalogic`
> WASM package, the engine's v4 → v5 changes are catalogued in
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md).

## Install

```bash
pip install datalogic-py
```

Pre-built wheels are published for:

| Platform           | Architectures   |
|--------------------|-----------------|
| Linux (manylinux)  | x86_64, aarch64 |
| Linux (musllinux)  | x86_64, aarch64 |
| macOS              | x86_64, arm64   |
| Windows            | x86_64          |

Python 3.10 and newer are supported via
[PEP 384 stable ABI (`abi3`)](https://peps.python.org/pep-0384/) — one
wheel per platform covers every CPython 3.10+ release.

> **Naming:** `pip install datalogic-py` (PyPI distribution name) →
> `import datalogic_py` (Python module name). Python modules can't
> contain hyphens, so the underscore form is the import.

## Quick start

```python
from datalogic_py import apply

result = apply(
    {"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]},
    {"score": 75},
)
# -> "pass"
```

## API reference

The Python binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).

| Tier         | Entry point                              | Use when                                                      |
|--------------|------------------------------------------|---------------------------------------------------------------|
| One-shot     | `apply(rule, data)`                      | Ad-hoc evaluation, one rule + one data shape                  |
| Engine       | `Engine().eval(rule, data)`              | Custom configuration (templating, custom operators, config)   |
| Compile once | `Engine().compile(rule).evaluate(data)`  | Same rule evaluated against many data inputs                  |
| Session      | `with engine.session() as sess: …`       | Hot loops — amortise arena reset across iterations            |

### One-shot — `apply(rule, data)`

```python
from datalogic_py import apply

apply({"+": [1, 2, 3]}, {})                                # 6
apply({"var": "user.age"}, {"user": {"age": 25}})          # 25
apply({"and": [{">": [{"var": "x"}, 0]}, True]}, {"x": 5}) # True
```

Both arguments accept Python `dict` / `list` values (converted via
[`pythonize`](https://crates.io/crates/pythonize), roughly 3–10× faster
than a JSON-string round-trip). For payloads with types `pythonize`
doesn't cover, see [Type conversion](#type-conversion) below.

### Engine — `Engine().eval(rule, data)`

Construct an `Engine` when you need templating mode or any non-default
configuration:

```python
from datalogic_py import Engine

engine = Engine()                          # default config
engine.eval({"==": [1, 1]}, {})            # True

# Templating mode — multi-key objects become output templates
templating_engine = Engine(templating=True)
templating_engine.eval(
    {"name": {"var": "user.name"}, "ok": {">": [{"var": "score"}, 50]}},
    {"user": {"name": "Ada"}, "score": 99},
)
# {"name": "Ada", "ok": True}
```

### Compile once — `Engine().compile(rule)` → `Rule.evaluate(data)`

Compile the rule once when you'll evaluate it against many data inputs.

```python
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]})

for payload in batch:
    result = rule.evaluate(payload)         # accepts a dict
    fast   = rule.evaluate_str(json_text)   # accepts a JSON string (skips dict conversion)
```

`Rule` is **thread-safe** — clone the reference into worker threads and
evaluate concurrently. The Rust eval call releases the GIL, so a
multi-threaded server gains real parallelism.

### Session — hot loops

For batches where you want to amortise arena reset across iterations,
open a `Session`. The arena is reset between iterations automatically.

```python
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({"+": [{"var": "x"}, 1]})

with engine.session() as sess:
    for payload in batch:
        result = sess.evaluate(rule, payload)
```

`Session` is the per-thread workhorse — open one per worker thread.
The arena that makes it fast can't be shared across threads (the same
way a database connection is per-task in a connection-pool model);
`Engine` and `Rule` are both thread-safe, so share those.

## Custom operators

Pass `custom_operators={"name": callable}` to `Engine(...)`. Each callable
receives the operator's pre-evaluated arguments as a JSON-array string and
returns a JSON string of the result:

```python
import json
from datalogic_py import Engine

engine = Engine(custom_operators={
    "double": lambda args_json: json.dumps(json.loads(args_json)[0] * 2),
})
engine.eval_str('{"double": [21]}', '{}')  # "42"
```

**Built-ins win**: a custom registration of a built-in name (`+`, `if`,
`var`, ...) never dispatches. Callbacks run with the GIL held.

## Engine configuration

Pass `config=` to `Engine(...)` to change evaluation semantics. The value
is a `dict` (or a JSON string) with an optional `preset` plus per-field
overrides. Unknown keys raise `EvaluateError`, so typos fail loudly:

```python
from datalogic_py import Engine, EvaluateError

strict = Engine(config={"preset": "strict"})
try:
    strict.eval({"+": ["", 1]}, {})   # strict rejects non-numeric coercion
except EvaluateError as e:
    print(e.error_type)

lenient = Engine(config={"division_by_zero": "return_null"})
lenient.eval({"/": [1.5, 0]}, {})     # None
```

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | `bool` |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

The `preset` applies first; the remaining keys override individual fields
on top of it. Every binding shares this JSON schema and parses it with
the same core code, so a config that works here works in the WASM and
Node bindings too. The full semantics of each knob are documented on the
Rust crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html).

## Error handling

All exceptions descend from `DataLogicError`:

| Exception        | When                                                              |
|------------------|-------------------------------------------------------------------|
| `ParseError`     | Malformed rule or data JSON, or an unsupported Python type in the input |
| `EvaluateError`  | Operator failure at runtime (including unknown operators, tag `InvalidOperator`) — carries `.error_type`, `.operator`, `.path` |

```python
from datalogic_py import Engine, EvaluateError

engine = Engine()
try:
    engine.eval({"+": ["x", 1]}, {})  # arithmetic on a non-numeric string raises
except EvaluateError as e:
    print(e.error_type)  # a runtime error tag
    print(e.operator)    # "+"
    print(e.path)        # JSON-pointer-style path through the compiled tree
```

## Threading

| Type      | Pattern                                                                          |
|-----------|----------------------------------------------------------------------------------|
| `Engine`  | Build once; share across threads                                                 |
| `Rule`    | Compile once; share across threads — `evaluate` releases the GIL for parallelism |
| `Session` | One per worker thread — the per-task workhorse                                   |

## Type conversion

The dict-input path uses [`pythonize`](https://crates.io/crates/pythonize):

**Supported:** `dict`, `list`, `str`, `int`, `float`, `bool`, `None`.

**Not supported** — these raise `ParseError` with a clear message:

- `datetime.datetime`, `datetime.date` — convert to ISO string at the
  Python edge
- `decimal.Decimal` — convert to `float` or `str`
- `bytes`, `set`, `tuple`
- `float('nan')`, `float('inf')` — JSON spec disallows them

For payloads with exotic types, use `rule.evaluate_str(json_text)` and
bring your own JSON encoder (e.g. with `default=str`).

## Templating mode

```python
engine = Engine(templating=True)
rule = engine.compile({
    "name": {"var": "user.name"},
    "ok": {">": [{"var": "score"}, 50]},
})
rule.evaluate({"user": {"name": "Ada"}, "score": 99})
# -> {"name": "Ada", "ok": True}
```

## Execution tracing

`Engine.evaluate_with_trace(logic, data)` evaluates with step-by-step
tracing and returns a JSON string envelope. The shape is identical to the
WASM binding's `evaluateWithTrace`, so the
[React debugger component](https://github.com/GoPlasmatic/datalogic-rs/tree/main/ui)
can consume it directly:

```python
import json
from datalogic_py import Engine

engine = Engine()
trace = json.loads(engine.evaluate_with_trace(
    '{">": [{"var": "score"}, 50]}',
    '{"score": 75}',
))
trace["result"]           # True
trace["expression_tree"]  # {"id", "expression", "children"} tree
trace["steps"]            # per-node execution log, in evaluation order
```

Both arguments are JSON strings. Runtime failures do not raise: the
envelope carries an `error` message and a `structured_error` object
instead, alongside the steps recorded up to the failure. Tracing skips
the optimizer so every operator in the rule appears in the trace; use it
for debugging, not hot paths.

## Performance

<!-- canonical-bench v5.0 -->
Geomean across 44 operator benchmark suites (Apple M2 Pro, median of 3 runs; [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **9.7 ns/op** — 4.9× faster than json-logic-engine (compiled, the fastest JS engine), 22.5× faster than jsonlogic-rs (the closest Rust alternative), and 43.7× faster than the json-logic-js reference implementation. The WASM build under Node measures 855.6 ns (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The pyo3 boundary adds a small per-call marshalling cost on top of the
core numbers. Use `rule.evaluate_str(json_text)` when you already have
a JSON string and want to skip the `pythonize` dict-conversion path;
`evaluate` releases the GIL, so a multi-threaded server gains real
parallelism on top of the engine's native speed.

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [Documentation — Python](https://goplasmatic.github.io/datalogic-rs/python/installation.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
