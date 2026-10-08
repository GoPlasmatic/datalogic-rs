# datalogic-py

[![PyPI](https://img.shields.io/pypi/v/datalogic-py.svg)](https://pypi.org/project/datalogic-py/)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs): one engine, every runtime.

Python bindings for [`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs),
a Rust implementation of [JSONLogic](http://jsonlogic.com). Same rules,
same semantics as the Rust crate, with the **compile-once /
evaluate-many** pattern exposed natively: compile a rule once and
evaluate it against thousands of data inputs without re-parsing. Every
binding runs the same core and passes the same 2,128-case conformance
battery (66 suites).

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** There is no v4 Python package. If you called the v4
> Rust crate or the v4 `@goplasmatic/datalogic` WASM package, see
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md)
> for the engine's v4 → v5 changes.

## Install

```bash
pip install datalogic-py
```

PyPI carries pre-built wheels for:

| Platform           | Architectures   |
|--------------------|-----------------|
| Linux (manylinux)  | x86_64, aarch64 |
| Linux (musllinux)  | x86_64, aarch64 |
| macOS              | x86_64, arm64   |
| Windows            | x86_64, arm64   |

The package supports Python 3.10 and newer through the
[PEP 384 stable ABI (`abi3`)](https://peps.python.org/pep-0384/): one
wheel per platform covers every CPython release from 3.10 on, and the
package metadata lists 3.10 through 3.14.

Every wheel ships type stubs and a `py.typed` marker
([PEP 561](https://peps.python.org/pep-0561/)), so mypy, pyright and
IDE autocomplete see the whole API.

The PyPI distribution is `datalogic-py` and the module is
`datalogic_py`: `pip install datalogic-py`, then `import datalogic_py`.
Python module names cannot contain hyphens.

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
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding);
the [API surface](#api-surface) table below lists every tier.

### One-shot: `apply(rule, data)`

```python
from datalogic_py import apply

apply({"+": [1, 2, 3]}, {})                                # 6
apply({"var": "user.age"}, {"user": {"age": 25}})          # 25
apply({"and": [{">": [{"var": "x"}, 0]}, True]}, {"x": 5}) # True
```

Both arguments accept Python `dict` / `list` values. The binding walks
them straight into the engine's arena values, with no JSON text and no
intermediate tree, which beats a `json.dumps` → `evaluate_str` →
`json.loads` round trip at every payload size the boundary benchmark
measures. Conversion work scales with node count, so large payloads
cost more (see [Performance](#performance)). If your data is already
JSON text, call the `*_str` entry points (`Rule.evaluate_str`,
`Session.evaluate_str`) and skip conversion; if you evaluate the same
payload repeatedly, parse it once into a
[`DataHandle`](#data-handles-typed-results-and-batch-evaluation). For
payloads with types the walk doesn't cover, see
[Type conversion](#type-conversion) below.

### Engine: `Engine().eval(rule, data)`

Construct an `Engine` when you need templating mode or any non-default
configuration:

```python
from datalogic_py import Engine

engine = Engine()                          # default config
engine.eval({"==": [1, 1]}, {})            # True

# Templating mode: multi-key objects become output templates
templating_engine = Engine(templating=True)
templating_engine.eval(
    {"name": {"var": "user.name"}, "ok": {">": [{"var": "score"}, 50]}},
    {"user": {"name": "Ada"}, "score": 99},
)
# {"name": "Ada", "ok": True}
```

`Engine(...)` takes keyword arguments only:

| Argument | Default | Effect |
|---|---|---|
| `templating` | `False` | Multi-key objects compile as output templates ([Templating mode](#templating-mode)) |
| `config` | `None` | Evaluation semantics, as a `dict` or JSON `str` ([Engine configuration](#engine-configuration)) |
| `custom_operators` | `None` | `{name: callable}` host operators ([Custom operators](#custom-operators)) |
| `strict_operator_names` | `False` | Refuse a custom operator named like a built-in |
| `template_key_escape` | `None` | One character that marks a template key as a literal field |
| `families` | `None` (every family) | The operator families the engine has besides the JSONLogic core ([Operator families](#operator-families)) |

### Compile once: `Engine().compile(rule)` → `Rule.evaluate(data)`

Compile the rule once when you'll evaluate it against many data inputs.

```python
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]})

for payload in batch:
    result = rule.evaluate(payload)         # accepts a dict
    fast   = rule.evaluate_str(json_text)   # accepts a JSON string (skips dict conversion)
```

`Rule` is **thread-safe**: hand the same instance to worker threads and
evaluate concurrently. The binding releases the GIL for each Rust
evaluate call, so a multi-threaded server runs evaluations in parallel.
Compiling and `check` release it too, once the binding has read the
rule into Rust.

`compile_template(rule)` and `compile_strict(rule)` choose the
templating mode for one compile, whatever the engine was built with.
The engine's custom operators, template key escape and families still
apply. See [Templating mode](#templating-mode).

### Checking a rule before it runs

`compile` accepts a rule that will fail at evaluation, such as one that
calls an operator the engine doesn't have. `check` reports every
problem the engine can see before the rule runs, each with an
[RFC 6901](https://www.rfc-editor.org/rfc/rfc6901) JSON Pointer into
the rule:

```python
engine.check({"if": [True, {"vr": "x"}, {"map": [1]}]})
# [{"code": "UnknownOperator", "severity": "error", "pointer": "/if/1",
#   "message": "unknown operator `vr`; did you mean `var`?", "operator": "vr"},
#  {"code": "ArgumentCount", "severity": "error", "pointer": "/if/2",
#   "message": "`map` takes exactly 2 arguments, not 1", "operator": "map"}]

engine.check({"a": {"var": "x"}, "b": 1}, "template")   # [] (mode: "engine", "strict" or "template")
```

An `"error"` will fail; a `"warning"` runs but is probably a mistake
(for example an argument the operator never evaluates).
`compile_checked(rule)` compiles only a rule with no error diagnostic
and otherwise raises `CompileError`, whose `.diagnostics` holds the
same list:

```python
from datalogic_py import CompileError

try:
    engine.compile_checked({"vr": "x"})
except CompileError as e:
    print(e.diagnostics[0]["code"])   # "UnknownOperator"
```

Three more calls describe rules and the engine without running
anything:

```python
rule = engine.compile({"if": [{">": [{"var": "user.age"}, 18]}, "adult", {"var": "fallback"}]})
rule.facts()
# {"reads": [["fallback"], ["user", "age"]], "computed_reads": False,
#  "reads_complete": True, "reads_data": True,
#  "operators": [">", "if", "val"], "custom_operators": [], "deterministic": True}

engine.operators()   # one dict per built-in operator: name, aliases, family, min_args, ...
engine.truthy({})    # False: the engine's truthiness (an empty dict is falsy, like [])
engine.truthy("[]")  # False: a str is JSON text
```

`facts()` lists each data path the rule reads as its segments, after
the optimizer, so a folded branch is not listed. `reads_complete` is
`False` when a path is computed at runtime or a custom operator runs,
and `deterministic` is `False` for `now` and any custom operator.
`operators()` returns a list in the schema of the
[operator catalogue](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json).

### Session: hot loops

For batches, open a `Session`: it keeps one arena and resets it at the
start of each evaluate call, so steady-state evaluation reuses memory
instead of allocating per call.

```python
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({"+": [{"var": "x"}, 1]})

with engine.session() as sess:
    for payload in batch:
        result = sess.evaluate(rule, payload)
```

Open one `Session` per worker thread: only the thread that created a
session may call it. `Engine` and `Rule` are both thread-safe, so share
those.

## Data handles, typed results, and batch evaluation

The ABI v2 tiers. A `DataHandle` is an immutable, pre-parsed JSON
document: parse a payload once and every evaluation against it skips
JSON parsing and dict conversion. Handles are engine-independent (one
handle can feed rules compiled by different engines), safe to share
across threads for reads, and not consumed by evaluation; the binding
frees the native memory when Python garbage-collects the handle.

```python
from datalogic_py import DataHandle

data = DataHandle('{"age": 25, "status": "active"}')  # raises ParseError on bad JSON
data = DataHandle({"age": 25, "status": "active"})    # or any JSON-shaped value, copied once
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
evaluates fine but the result is not of the requested type. A datetime
or duration result counts as a string, the JSON type it serialises to,
so the message reads `result is not a boolean (got string)`.
`evaluate_truthy` coerces any result through the engine's configured
truthiness rules (the same coercion `if`/`and`/`or` apply).

The batch entry points evaluate a whole set in one native call and
report failures per item, so one bad input leaves its neighbours
unaffected:

```python
from datalogic_py import BatchItemError

# One rule, many payloads:
results = sess.evaluate_batch(rule, [d0, d1, d2])
# Many rules, one payload (the rule-set / feature-flag shape):
results = sess.evaluate_many([r0, r1], data)

for i, r in enumerate(results):
    if isinstance(r, BatchItemError):   # not raised; a result object
        print(f"item {i} failed: {r.message} ({r.tag}, operator={r.operator})")
    else:
        print(f"item {i}: {r}")         # the item's JSON string
```

Typed and batch evaluations take data handles only, and sessions stay
single-threaded. A typed call or `evaluate_batch` raises
`EvaluateError` with `error_type == "InvalidArgument"` when the rule
was compiled by a different engine than the session's;
`evaluate_many` reports such a rule as its item's `BatchItemError`
(`tag == "InvalidArgument"`) and evaluates the rest.

## API surface

| Tier         | Entry point                              | Use when                                                      |
|--------------|------------------------------------------|---------------------------------------------------------------|
| One-shot     | `apply(rule, data)`                      | Ad-hoc evaluation, one rule + one data shape                  |
| Engine       | `Engine().eval(rule, data)`              | Custom configuration (templating, custom operators, config)   |
| Compile once | `Engine().compile(rule).evaluate(data)`  | Same rule evaluated against many data inputs                  |
| Compile mode | `engine.compile_template(rule)` / `engine.compile_strict(rule)` | One rule in a templating mode other than the engine's |
| Checked      | `engine.check(rule, mode=None)` / `engine.compile_checked(rule)` | Find every problem in a rule before it runs |
| Introspection | `rule.facts()` / `engine.operators()` / `engine.truthy(value)` | What a rule reads, which operators exist, the engine's truthiness |
| Session      | `with engine.session() as sess: …`       | Hot loops: reuse one arena across evaluations                 |
| Data handle  | `DataHandle(json or value)` → `sess.evaluate_data(rule, data)` | Same payload evaluated many times: parse or convert once, zero work per call |
| Typed        | `sess.evaluate_bool/int/float/truthy(rule, data)` | Predicates and scalar results, no JSON decode on the way out |
| Batch        | `sess.evaluate_batch(rule, datas)` / `sess.evaluate_many(rules, data)` | Many evaluations per native call, per-item errors |
| Metered      | `engine.eval_metered(rule, data, budget=None)` / `rule.evaluate_metered(data, budget=None)` | The operations an evaluation charges, optionally capped |
| Traced       | `engine.evaluate_with_trace(logic_json, data_json, mode=None)` | Step-by-step debugging; feeds the React debugger |

`datalogic_py.__version__` reports the installed version.

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
`var`, ...) never dispatches. With `strict_operator_names=True` the
engine refuses such a name instead: `Engine(...)` raises
`EvaluateError` with `error_type == "ConfigurationError"`. The option
also refuses a name that begins with the template key escape. A name
whose family you leave out with `families` is free for a custom
operator. Callbacks run with the GIL held.

## Engine configuration

Pass `config=` to `Engine(...)` to change evaluation semantics. The value
is a `dict` (or a JSON string) with an optional `preset` plus per-field
overrides. Unknown keys and values raise `EvaluateError` with
`error_type == "ConfigurationError"`:

```python
from datalogic_py import Engine, EvaluateError

strict = Engine(config={"preset": "strict"})
try:
    strict.eval({"+": ["", 1]}, {})   # strict rejects non-numeric coercion
except EvaluateError as e:
    print(e.error_type)               # "Thrown" (a NaN payload)

lenient = Engine(config={"division_by_zero": "return_null"})
lenient.eval({"/": [1.5, 0]}, {})     # None
```

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | `bool` |
| `missing_var` | `"null"` (default: a missing variable reads as `null`), `"error"` (raises `VariableNotFound`) |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `null` for unbounded (caps the work one evaluation may do; crossing it raises `BudgetExceeded`) |

The `preset` applies first; the remaining keys override individual fields
on top of it. Every binding parses this JSON with the same core code,
so a config that works here works in every other binding. The Rust
crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html)
documents the full semantics of each knob.

`"missing_var": "error"` turns a typo in a path into an error instead
of a `None` that flows on. A default (`{"var": ["x", 0]}`), a present
`null`, `missing`, `missing_some` and `exists` are not misses, and
`try` catches the error:

```python
engine = Engine(config={"missing_var": "error"})
engine.eval({"var": "user.nmae"}, {"user": {"name": "Ada"}})
# EvaluateError: Variable not found: user.nmae (error_type "VariableNotFound")
engine.eval({"var": ["user.nmae", "anon"]}, {"user": {"name": "Ada"}})   # "anon"
```

### Operator families

`families=[...]` keeps the engine to the JSONLogic core plus the
families you name, using the `family` names of `engine.operators()`:
`"ExtString"`, `"ExtArray"`, `"ExtObject"`, `"ExtMath"`,
`"ExtControl"`, `"ErrorHandling"`, `"DateTime"`, `"Tensor"`, `"Flagd"`
(`"Core"` is always there). A family you leave out is not there for
that engine: its names compile as unknown operators, which fail at
evaluation with `InvalidOperator` (and are errors in `check` and
`compile_checked`), and a custom operator may take them. An unknown
family name raises `EvaluateError` with
`error_type == "ConfigurationError"`.

```python
strings_only = Engine(families=["ExtString"])
strings_only.eval({"upper": "abc"}, None)   # "ABC"
strings_only.eval({"abs": -1}, None)        # EvaluateError, error_type "InvalidOperator"
```

### Metering: what a rule costs

`eval_metered` returns `(result_json, ops)`: the result as a JSON `str`
and the operations the evaluation charged, so you can see what a rule
costs whether or not a budget is set. `Rule.evaluate_metered(data,
budget=None)` does the same on an already-compiled rule.

```python
engine = Engine()
engine.eval_metered({"map": [{"var": "xs"}, {"*": [{"var": ""}, 2]}]}, {"xs": [1, 2, 3]})
# ('[2,4,6]', 10)

# The optional `budget` argument caps the operations for that one call,
# overriding the engine's `ops_budget` config key.
engine.eval_metered(rule, data, budget=100_000)
```

One operation is one node the engine dispatches, one item an iterator
walks, or whatever an operator charges for the data it moves (the tensor
family prices itself in elements). Literals and constant-folded subtrees
cost nothing. The count is deterministic for a given rule, data and
engine version. Exceeding the budget raises
`EvaluateError` with `.error_type == "BudgetExceeded"`, carrying
`.budget` and `.spent`. The engine refuses the evaluation before doing
the work, and a `try` in the rule cannot recover from it. A `budget`
of `0` raises `EvaluateError` with `error_type == "InvalidArgument"`;
pass `None` for the engine's own budget.

## Error handling

All exceptions descend from `DataLogicError`:

| Exception        | When                                                              |
|------------------|-------------------------------------------------------------------|
| `ParseError`     | Malformed rule or data JSON, or an unsupported Python type in the input |
| `EvaluateError`  | Every other engine failure: runtime operator errors, unknown operators (`InvalidOperator`), invalid configuration (`ConfigurationError`) |
| `CompileError`   | `compile_checked` refused the rule; `.diagnostics` lists every problem and `error_type` is `"CompileError"` |

Every exception carries `.error_type` (the engine's error tag),
`.operator` (the innermost failing operator, or `None`), `.node_ids`
(a leaf-to-root breadcrumb of compiled-node ids) and `.path` (a
root-to-leaf list of step dicts with `node_id`, `operator`,
`arg_index` and `json_pointer`, or `None` when no compiled rule was in
scope).

Two `error_type` tags come from the binding itself rather than the
engine, as in the C ABI: `"TypeMismatch"` (a typed evaluation whose
result has the wrong type) and `"InvalidArgument"` (for example a rule
compiled by a different engine passed to a session's handle-based entry
points). Per-item batch failures don't raise; they surface as
`BatchItemError` values (`.tag`, `.message`, `.operator`) in the result
list.

```python
from datalogic_py import Engine, EvaluateError

engine = Engine()
try:
    engine.eval({"+": [1, {"*": ["x", 2]}]}, {})  # arithmetic on a non-numeric string
except EvaluateError as e:
    print(e.error_type)  # "Thrown" (a NaN payload under the default config)
    print(e.operator)    # "*", the innermost operator that failed
    print(e.path)        # [{"operator": "+", "json_pointer": "", ...},
                         #  {"operator": "*", "json_pointer": "/+/1", ...}]
```

## Threading

| Type         | Pattern                                                                          |
|--------------|----------------------------------------------------------------------------------|
| `Engine`     | Build once; share across threads                                                 |
| `Rule`       | Compile once; share across threads: `evaluate` releases the GIL for parallelism  |
| `Session`    | One per worker thread; only its creating thread may call it                      |
| `DataHandle` | Parse once; immutable, share across threads for reads (evaluation never mutates it) |

## Type conversion

The dict-input path walks Python objects into the engine's arena
representation, with a [`pythonize`](https://crates.io/crates/pythonize)
fallback for the shapes the walk doesn't cover. Both give the same
results; only speed differs.

**Fast direct walk:** `dict`, `list`, `tuple`, `str`, `int`, `float`,
`bool`, `None`.

**Handled by the fallback:** `set`/`frozenset` (become JSON arrays, in
iteration order), container and scalar subclasses (`IntEnum`,
`OrderedDict`, …), mappings and dataclasses.

**Conversion details:**

- `float('nan')` / `float('inf')` become JSON `null` (they have no JSON
  encoding)
- ints above `2^63 - 1` up to `2^64 - 1` become `float`; larger ones
  raise `ParseError`
- dict keys must be `str` (anything else raises `ParseError`), and the
  engine sees object keys in sorted order, so object-iteration results
  are deterministic
- result dicts come back key-sorted
- a datetime or duration result comes back as a string

**Not supported.** These raise `ParseError`:

- `datetime.datetime`, `datetime.date`: convert to ISO string at the
  Python edge
- `decimal.Decimal`: convert to `float` or `str`
- `bytes`, `bytearray`

For payloads with exotic types, use `rule.evaluate_str(json_text)` and
bring your own JSON encoder (for example with `default=str`).

## Templating mode

In templating mode a multi-key object is an output template and an
unknown key an output field. `Engine(templating=True)` sets the mode
for every compile on that engine; `compile_template` and
`compile_strict` set it for one compile, so one engine can check a
condition strictly and build an output template:

```python
engine = Engine()
engine.compile({"user": {"var": "name"}, "source": "api"})   # raises EvaluateError ("InvalidOperator")
engine.compile_template({"user": {"var": "name"}, "source": "api"}).evaluate({"name": "ana"})
# {"source": "api", "user": "ana"}
```

To emit a key that is also an operator name, set `template_key_escape`
to one character; a key that starts with it becomes a literal field
with the character stripped:

```python
engine = Engine(templating=True, template_key_escape="$")
engine.eval({"$type": "user", "id": {"var": "id"}}, {"id": 7})
# {"id": 7, "type": "user"}
```

## Execution tracing

`Engine.evaluate_with_trace(logic, data, mode=None)` evaluates with
step-by-step tracing and returns a JSON string envelope. The shape is
identical to the WASM binding's `evaluateWithTrace`, so the
[React debugger component](https://github.com/GoPlasmatic/datalogic-rs/tree/main/ui)
can consume it:

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
trace["pointers"]         # {"<node id>": "<JSON Pointer into the rule>", ...}
```

Both arguments are JSON strings. `mode` is `"engine"` (the default),
`"strict"` or `"template"`, so you can trace a rule you compile with
`compile_template`. Runtime failures do not raise: the envelope
carries an `error` message and a `structured_error` object instead,
alongside the steps recorded up to the failure. `pointers` maps each
node id to the JSON Pointer of the rule value it was compiled from, and
is absent when the rule does not compile. The result keeps the key
order the rule produced. Tracing skips the optimizer so every operator
in the rule appears in the trace; use it for debugging, not hot paths.

## Performance

<!-- canonical-bench v5.1 -->
Geomean across 51 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **10.3 ns/op**, 7.0× faster than json-logic-engine (compiled, the fastest JS engine), 28.1× faster than jsonlogic-rs (the closest Rust alternative), and 83.6× faster than the json-logic-js reference implementation. The WASM build under Node measures 900.5 ns geomean (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The pyo3 boundary adds a per-call marshalling cost on top of the core
numbers; the dict paths use direct Python ↔ arena walks, so that cost
scales with payload node count. Use `rule.evaluate_str(json_text)` when
you already have a JSON string, and a `DataHandle` when you evaluate the
same payload repeatedly. On the
[boundary harness's](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BINDINGS-OVERHEAD.md)
8 KB workload, `session.evaluate_data_str` measures about 1.4 µs/op
against about 12 µs for `session.evaluate_str` (the per-call JSON
parse) and about 25 µs for the dict path (the per-call conversion
walk). Every evaluate call releases the GIL, so a multi-threaded server
runs evaluations in parallel.

## Building from source

The binding lives in
[`bindings/python/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/python)
and builds with [maturin](https://www.maturin.rs) (needs a Rust
toolchain and Python 3.10+):

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs
cd datalogic-rs/bindings/python
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest
maturin develop --release   # build + install into the venv
pytest                      # run the test suite
```

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [Documentation: Python](https://goplasmatic.github.io/datalogic-rs/python/installation.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
