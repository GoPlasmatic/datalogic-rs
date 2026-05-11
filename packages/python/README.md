# datalogic — Python bindings for `datalogic-rs`

[![PyPI](https://img.shields.io/pypi/v/datalogic.svg)](https://pypi.org/project/datalogic/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Python bindings for [`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs),
a fast Rust implementation of [JSONLogic](http://jsonlogic.com).

Unlike the original Python `jsonlogic` packages, this binding exposes the
**compile-once / evaluate-many** pattern that the underlying engine
supports — compile a rule once and evaluate it against thousands of data
inputs without re-parsing.

## Install

```bash
pip install datalogic
```

Pre-built wheels are published for Linux (manylinux + musllinux, x86_64 and
aarch64), macOS (x86_64 and arm64), and Windows (x86_64). Python 3.10 and
newer are supported via [PEP 384 stable ABI (`abi3`)](https://peps.python.org/pep-0384/) —
one wheel per platform covers every CPython 3.10+ release.

## Quick start

```python
from datalogic import apply

# One-shot — parses the rule each call. Use for ad-hoc evaluations.
result = apply(
    {"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]},
    {"score": 75},
)
# -> "pass"
```

## Compile once, evaluate many

```python
from datalogic import Engine

engine = Engine()
rule = engine.compile({"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]})

for payload in batch:
    result = rule.evaluate(payload)         # accepts a dict
    fast   = rule.evaluate_str(json_text)   # accepts a JSON string (skips dict conversion)
```

`Rule` is thread-safe — clone the reference into worker threads and
evaluate concurrently. The Rust eval call releases the GIL, so a
multi-threaded server gains real parallelism.

## Hot-loop session

For batches where you want to amortise arena reset across iterations,
open a `Session`:

```python
from datalogic import Engine

engine = Engine()
rule = engine.compile({"+": [{"var": "x"}, 1]})

with engine.session() as sess:
    for payload in batch:
        result = sess.evaluate(rule, payload)
```

`Session` is **not thread-safe** — open one per thread. The arena is
reset between iterations automatically.

## Errors

All exceptions descend from `DataLogicError`:

| Exception | When |
| --- | --- |
| `ParseError` | Malformed rule or data JSON, or unsupported Python type |
| `EvaluateError` | Operator failure at runtime — carries `.error_type`, `.operator`, `.path` |

```python
from datalogic import Engine, EvaluateError

engine = Engine()
try:
    engine.eval({"var": "missing"}, {})
except EvaluateError as e:
    print(e.error_type)  # "VariableNotFound"
    print(e.operator)    # "var"
```

## Type conversion

The dict-input path uses [`pythonize`](https://crates.io/crates/pythonize),
which is roughly 3-10× faster than a JSON-string round-trip.

**Supported:** `dict`, `list`, `str`, `int`, `float`, `bool`, `None`.

**Not supported** — these raise `ParseError` with a clear message:
- `datetime.datetime`, `datetime.date` — convert to ISO string at the Python edge
- `decimal.Decimal` — convert to `float` or `str`
- `bytes`, `set`, `tuple`
- `float('nan')`, `float('inf')` — JSON spec disallows them

For payloads with exotic types, use `evaluate_str(json_text)` and bring
your own JSON encoder (e.g. with `default=str`).

## Templating mode

```python
engine = Engine(templating=True)
rule = engine.compile({"name": {"var": "user.name"}, "ok": {">": [{"var": "score"}, 50]}})
rule.evaluate({"user": {"name": "Ada"}, "score": 99})
# -> {"name": "Ada", "ok": True}
```

## License

Apache-2.0. See the [main repository](https://github.com/GoPlasmatic/datalogic-rs)
for source and contribution guidelines.
