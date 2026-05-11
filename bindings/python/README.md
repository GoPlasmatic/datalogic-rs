# datalogic-py

[![PyPI](https://img.shields.io/pypi/v/datalogic-py.svg)](https://pypi.org/project/datalogic-py/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Python bindings for [`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs),
a fast Rust implementation of [JSONLogic](http://jsonlogic.com). Same
rules, same semantics as the Rust crate, with the **compile-once /
evaluate-many** pattern exposed natively — compile a rule once and
evaluate it against thousands of data inputs without re-parsing.

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

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
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#choosing-your-api-five-tiers-one-engine).

| Tier         | Entry point                              | Use when                                                      |
|--------------|------------------------------------------|---------------------------------------------------------------|
| One-shot     | `apply(rule, data)`                      | Ad-hoc evaluation, one rule + one data shape                  |
| Engine       | `Engine().eval(rule, data)`              | Custom configuration (templating, future operator extensions) |
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

## Error handling

All exceptions descend from `DataLogicError`:

| Exception        | When                                                              |
|------------------|-------------------------------------------------------------------|
| `ParseError`     | Malformed rule or data JSON, unsupported operator, or unsupported Python type |
| `EvaluateError`  | Operator failure at runtime — carries `.error_type`, `.operator`, `.path` |

```python
from datalogic_py import Engine, EvaluateError

engine = Engine()
try:
    engine.eval({"var": "missing"}, {})
except EvaluateError as e:
    print(e.error_type)  # e.g. "VariableNotFound"
    print(e.operator)    # "var"
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

## Learn more

- [Repo README](https://github.com/GoPlasmatic/datalogic-rs#readme) — cross-runtime overview, all binding READMEs
- [Rust crate README](../../crates/datalogic-rs/README.md) — engine design, custom operators, configuration knobs
- [Full documentation](https://goplasmatic.github.io/datalogic-rs/) — long-form guide, operator reference
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com/)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
