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
binding runs the same core and passes the same 1,553-case conformance
battery (54 suites).

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
| Windows            | x86_64, arm64   |

Python 3.10 and newer are supported via
[PEP 384 stable ABI (`abi3`)](https://peps.python.org/pep-0384/) — one
wheel per platform covers every CPython 3.10+ release.

The package is fully typed ([PEP 561](https://peps.python.org/pep-0561/)):
every wheel ships type stubs and a `py.typed` marker, so mypy, pyright,
and IDE autocomplete see the complete API surface out of the box.

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
| Data handle  | `DataHandle(json)` → `sess.evaluate_data(rule, data)` | Same payload evaluated many times: parse once, zero parse work per call |
| Typed        | `sess.evaluate_bool/int/float/truthy(rule, data)` | Predicates and scalar results, no JSON decode on the way out |
| Batch        | `sess.evaluate_batch(rule, datas)` / `sess.evaluate_many(rules, data)` | Many evaluations per native call, per-item errors |

### One-shot — `apply(rule, data)`

```python
from datalogic_py import apply

apply({"+": [1, 2, 3]}, {})                                # 6
apply({"var": "user.age"}, {"user": {"age": 25}})          # 25
apply({"and": [{">": [{"var": "x"}, 0]}, True]}, {"x": 5}) # True
```

Both arguments accept Python `dict` / `list` values, converted by a
direct walk between Python objects and the engine's arena values (no
JSON text, no intermediate tree — 2.5-3.5× faster than the
pythonize-based conversion earlier builds used, and faster than a
`json.dumps` → `evaluate_str` → `json.loads` round-trip at every
payload size we measure). Payload size still matters: conversion work
scales with node count, so an 8 KB dict costs ~20 µs of walk on top of
the evaluation. If your data is already JSON text, call the `*_str`
entry points (`Rule.evaluate_str`, `Session.evaluate_str`) and skip
conversion; if the same payload is evaluated repeatedly, parse it once
into a [`DataHandle`](#data-handles-typed-results-and-batch-evaluation)
and skip the per-call cost entirely. For payloads with types the walk
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

## Data handles, typed results, and batch evaluation

The ABI v2 tiers. A `DataHandle` is an immutable, pre-parsed JSON
document: parse a payload once and every evaluation against it skips
JSON parsing (and dict conversion) entirely. Handles are
engine-independent (one handle can feed rules compiled by different
engines), safe to share across threads for reads, and not consumed by
evaluation — the native memory is released when the handle is
garbage-collected.

```python
from datalogic_py import DataHandle

data = DataHandle('{"age": 25, "status": "active"}')  # raises ParseError on bad JSON
data.allocated_bytes                    # bytes held by the handle's arena

rule.evaluate_data(data)                # thread-safe, like rule.evaluate
rule.evaluate_data_str(data)            # same, JSON str out
sess.evaluate_data(rule, data)          # hot path: session arena + no parse
sess.evaluate_data_str(rule, data)
```

For predicates and scalar results, the typed session evaluations skip
the result conversion too:

```python
ok = sess.evaluate_bool(rule, data)     # strict JSON boolean
n  = sess.evaluate_int(rule, data)      # exact integer result
f  = sess.evaluate_float(rule, data)    # any JSON number
t  = sess.evaluate_truthy(rule, data)   # JSONLogic truthiness, never mismatches
```

`evaluate_bool`, `evaluate_int`, and `evaluate_float` raise
`EvaluateError` with `error_type == "TypeMismatch"` when the rule
evaluates fine but the result is not of the requested type.
`evaluate_truthy` coerces any result through the engine's configured
truthiness rules (the same coercion `if`/`and`/`or` apply).

The batch entry points evaluate a whole set in one native call and
report failures per item, so one bad input never poisons its
neighbours:

```python
from datalogic_py import BatchItemError

# One rule, many payloads:
results = sess.evaluate_batch(rule, [d0, d1, d2])
# Many rules, one payload (the rule-set / feature-flag shape):
results = sess.evaluate_many([r0, r1], data)

for i, r in enumerate(results):
    if isinstance(r, BatchItemError):   # not raised — a result object
        print(f"item {i} failed: {r.message} ({r.tag}, operator={r.operator})")
    else:
        print(f"item {i}: {r}")         # the item's JSON string
```

Exceptions are reserved for argument problems (a rule compiled by a
different engine, a non-handle list element, …). Typed and batch
evaluations take data handles only; rules must belong to the session's
engine, and sessions stay single-threaded.

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

Two `error_type` tags come from the binding itself rather than the
engine, mirroring the C ABI: `"TypeMismatch"` (a typed evaluation whose
result has the wrong type) and `"InvalidArgument"` (e.g. a rule
compiled by a different engine passed to a session's handle-based entry
points). Per-item batch failures don't raise at all — they surface as
`BatchItemError` values (`.tag`, `.message`, `.operator`) in the result
list.

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

| Type         | Pattern                                                                          |
|--------------|----------------------------------------------------------------------------------|
| `Engine`     | Build once; share across threads                                                 |
| `Rule`       | Compile once; share across threads — `evaluate` releases the GIL for parallelism |
| `Session`    | One per worker thread — the per-task workhorse                                   |
| `DataHandle` | Parse once; immutable, share across threads for reads (evaluation never mutates it) |

## Type conversion

The dict-input path walks Python objects directly into the engine's
arena representation (with a [`pythonize`](https://crates.io/crates/pythonize)
fallback for the long tail — behaviour is identical either way, only
speed differs):

**Fast direct walk:** `dict`, `list`, `tuple`, `str`, `int`, `float`,
`bool`, `None`.

**Handled via the fallback:** `set`/`frozenset` (become JSON arrays,
iteration order), container/scalar subclasses (`IntEnum`,
`OrderedDict`, …), mappings and dataclasses.

**Conversion details worth knowing:**

- `float('nan')` / `float('inf')` become JSON `null` (they have no JSON
  encoding)
- ints above `2^63 - 1` up to `2^64 - 1` degrade to `float`; beyond
  that they raise `ParseError`
- dict keys must be `str` (anything else raises `ParseError`) and
  objects are presented to the engine in sorted-key order, so
  object-iteration results are deterministic
- result dicts also come back key-sorted

**Not supported** — these raise `ParseError` with a clear message:

- `datetime.datetime`, `datetime.date` — convert to ISO string at the
  Python edge
- `decimal.Decimal` — convert to `float` or `str`
- `bytes`, `bytearray`

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
Geomean across 50 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **8.9 ns/op**, 7.9× faster than json-logic-engine (compiled, the fastest JS engine), 30.6× faster than jsonlogic-rs (the closest Rust alternative), and 104.2× faster than the json-logic-js reference implementation. The WASM build under Node measures 901.1 ns geomean (101× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The pyo3 boundary adds a small per-call marshalling cost on top of the
core numbers; the dict paths use direct Python ↔ arena walks, so that
cost scales with payload node count, not with a JSON round-trip. Use
`rule.evaluate_str(json_text)` when you already have a JSON string, and
a `DataHandle` when the same payload is evaluated repeatedly — on the
boundary harness's 8 KB workload, `session.evaluate_data_str` measures
~1.3 µs/op against ~12 µs for `session.evaluate_str` (the per-call JSON
parse) and ~24 µs for the dict path (the per-call conversion walk).
Every evaluate call releases the GIL, so a multi-threaded server gains
real parallelism on top of the engine's native speed.

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
