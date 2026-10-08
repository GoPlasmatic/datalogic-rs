# datalogic-go

[![Go Reference](https://pkg.go.dev/badge/github.com/GoPlasmatic/datalogic-rs/bindings/go/v5.svg)](https://pkg.go.dev/github.com/GoPlasmatic/datalogic-rs/bindings/go/v5)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs): one engine, every runtime.

Go binding for the
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs)
JSONLogic engine, with the same rules and semantics as the Rust crate.
It calls the shared C ABI in
[`bindings/c/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c)
through cgo and links `libdatalogic_c.a` statically, so your binaries
carry no runtime shared-library dependency. Every binding runs the same
core and passes the same 2,128-case conformance battery (66 suites).

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** There is no v4 Go package. If you called the v4 Rust
> crate or the v4 `@goplasmatic/datalogic` WASM package, see
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md)
> for the engine's v4 → v5 changes.

## Install

```sh
go get github.com/GoPlasmatic/datalogic-rs/bindings/go/v5@v5.8.1
```

```go
import datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
```

Go modules require the `/v5` suffix for any major version ≥ 2
(see [Go modules ref: major version
suffixes](https://go.dev/ref/mod#major-version-suffixes)). The
binding's version tracks the core crate's, so `v5.x.y` lives at `/v5`
and `v6.x.y` will live at `/v6`.

Released tags ship prebuilt static libraries (about 13.5 MB on
darwin/arm64, built without debuginfo and with fat LTO) for:

| OS / Arch | `lib/` subdirectory | Rust target |
|---|---|---|
| Linux x86_64 | `linux_amd64/` | `x86_64-unknown-linux-gnu` |
| Linux ARM64 | `linux_arm64/` | `aarch64-unknown-linux-gnu` |
| macOS Intel | `darwin_amd64/` | `x86_64-apple-darwin` |
| macOS Apple Silicon | `darwin_arm64/` | `aarch64-apple-darwin` |
| Windows x86_64 | `windows_amd64/` | `x86_64-pc-windows-gnu` (mingw-w64) |
| Windows ARM64 | `windows_arm64/` | `aarch64-pc-windows-gnullvm` (llvm-mingw) |

cgo build tags in `cgo_<os>_<arch>.go` pick the right one at build time.
You need Go 1.25 or newer and a C compiler to link; you don't need a
Rust toolchain.

## Quick start

```go
package main

import (
    "fmt"
    datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

func main() {
    // One-shot:
    out, _ := datalogic.Apply(`{"+":[1,2]}`, `{}`)
    fmt.Println(out)  // 3

    // Reusing a compiled rule:
    e := datalogic.NewEngine()
    defer e.Close()
    rule, _ := e.Compile(`{"var":"x"}`)
    defer rule.Close()
    for _, x := range []int{1, 7, 42} {
        out, _ := rule.Evaluate(fmt.Sprintf(`{"x":%d}`, x))
        fmt.Println(out)
    }

    // Hot-loop session (arena reuse):
    s := e.Session()
    defer s.Close()
    for _, x := range []int{1, 7, 42} {
        out, _ := s.Evaluate(rule, fmt.Sprintf(`{"x":%d}`, x))
        fmt.Println(out)
    }
}
```

Every call takes and returns JSON text. `datalogic.Version()` reports
the linked engine version.

## Compile once, evaluate many

`engine.Compile(ruleJSON)` returns a `*Rule` you can evaluate from any
number of goroutines. `CompileTemplate` and `CompileStrict` choose the
templating mode for one compile, whatever the engine was built with
(`CompileMode(ruleJSON, mode)` takes a `Mode`: `ModeEngine`,
`ModeStrict` or `ModeTemplate`). The engine's custom operators,
template key escape and families still apply.

```go
tpl, err := engine.CompileTemplate(`{"user": {"var": "name"}, "source": "api"}`)
out, _ := tpl.Evaluate(`{"name": "ana"}`) // {"user":"ana","source":"api"}
```

`Compile` accepts a rule that fails only when it runs, such as one that
calls an operator the engine doesn't have. `Check` reports every
problem the engine can see before the rule runs, as a JSON array of
`{code, severity, message, pointer, operator}` with an RFC 6901 JSON
Pointer into the rule. `CompileChecked` compiles only a rule with no
error diagnostic; otherwise it returns a `*Error` with
`Type == "CompileError"` whose `DiagnosticsJSON` lists them:

```go
diags, _ := engine.Check(`{"if": [true, {"vr": "x"}, {"map": [1]}]}`, datalogic.ModeEngine)
// [{"code":"UnknownOperator","severity":"error","message":"unknown operator `vr`; did you mean `var`?","pointer":"/if/1","operator":"vr"},
//  {"code":"ArgumentCount","severity":"error","message":"`map` takes exactly 2 arguments, not 1","pointer":"/if/2","operator":"map"}]

_, err = engine.CompileChecked(`{"vr": "x"}`)
var dlErr *datalogic.Error
if errors.As(err, &dlErr) && dlErr.Type == "CompileError" {
    fmt.Println(dlErr.DiagnosticsJSON)
}
```

Three more calls describe rules and the engine without running
anything. `rule.Facts()` returns what a compiled rule reads (each path
as its segments) and calls, `engine.Operators()` the operator catalogue
(the schema of
[`operators.json`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json)),
and `engine.Truthy(valueJSON)` the engine's truthiness for a JSON value
(under the default rules `{}` is falsy, like `[]`):

```go
r, _ := engine.Compile(`{"if": [{">": [{"var": "user.age"}, 18]}, "adult", {"var": "fallback"}]}`)
facts, _ := r.Facts()
// {"reads":[["fallback"],["user","age"]],"computed_reads":false,"reads_complete":true,
//  "reads_data":true,"operators":[">","if","val"],"custom_operators":[],"deterministic":true}

empty, _ := engine.Truthy(`{}`) // false
```

## Sessions (hot loops)

A `Session` reuses one arena across evaluations and resets it at the
start of every call. Open one per goroutine: sessions are not
goroutine-safe. `EvaluateMetered` also reports the operations an
evaluation charged; a `budget` of 0 means the engine's `ops_budget`, or
unbounded when it has none.

```go
s := engine.Session()
defer s.Close()

out, _ := s.Evaluate(rule, `{"x": 1}`)
out, ops, err := s.EvaluateMetered(rule, `{"x": 1}`, 10_000) // *Error with Type "BudgetExceeded" past the budget
```

A rule must come from the engine the session was opened on; a rule
from another engine fails with `Type == "InvalidArgument"`.

## Data handles, typed results, and batch evaluation

A `DataHandle` is an immutable, pre-parsed JSON document: parse a
payload once with `datalogic.ParseData` and every evaluation against it
skips JSON parsing. Handles are engine-independent (one handle can feed
rules compiled by different engines), safe for concurrent use from
several goroutines, and not consumed by evaluation. `Close()` them
after the last use; a GC finalizer frees a handle you forget.

```go
data, err := datalogic.ParseData(`{"age": 25, "status": "active"}`)
if err != nil { /* invalid JSON */ }
defer data.Close()

out, _ := rule.EvaluateData(data)       // goroutine-safe, like rule.Evaluate
out, _ = session.EvaluateData(rule, data) // hot path: session arena + no parse
```

For predicates and scalar results, the typed session evaluations skip
the JSON result string too:

```go
ok, err  := session.EvaluateBool(rule, data)   // strict JSON boolean
n, err   := session.EvaluateInt64(rule, data)  // exact integer result
f, err   := session.EvaluateFloat64(rule, data) // any JSON number
t, err   := session.EvaluateTruthy(rule, data) // JSONLogic truthiness, never mismatches
```

`EvaluateBool`, `EvaluateInt64`, and `EvaluateFloat64` return a `*Error`
with `Type == "TypeMismatch"` when the rule evaluates fine but the
result is not of the requested type. A datetime or duration result
counts as a string, the JSON type it serialises to, so the message
reads `result is not a boolean (got string)`. `EvaluateTruthy` coerces
any result through the engine's configured truthiness rules (the same
coercion `if`/`and`/`or` apply).

The batch entry points evaluate a whole set in one native call and
report failures per item, so one bad input leaves its neighbours
unaffected:

```go
// One rule, many payloads:
results, err := session.EvaluateBatch(rule, []*datalogic.DataHandle{d0, d1, d2})
// Many rules, one payload (the rule-set / feature-flag shape):
results, err = session.EvaluateMany([]*datalogic.Rule{r0, r1}, data)

for i, r := range results {
    if r.Err != nil {
        e := r.Err.(*datalogic.Error)
        fmt.Printf("item %d failed: %s (%s)\n", i, e.Message, e.Type)
        continue
    }
    fmt.Printf("item %d: %s\n", i, r.Value)
}
```

The call-level `error` return covers argument problems only: a nil
session, a nil or foreign rule passed to `EvaluateBatch`, a nil data
handle passed to `EvaluateMany`. A nil handle in the `EvaluateBatch`
slice, or a nil or foreign rule in the `EvaluateMany` slice, fails
only its own item. Typed and batch evaluations take data handles only,
and sessions stay single-goroutine.

## API surface

The Go binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).

| Tier         | Entry point                                  | Use when                                                |
|--------------|----------------------------------------------|---------------------------------------------------------|
| One-shot     | `datalogic.Apply(rule, data)`                | Ad-hoc evaluation, one rule + one data shape            |
| Engine       | `datalogic.NewEngine().Apply(rule, data)`    | Engine reuse without compile-once                       |
| Compile once | `engine.Compile(rule)` → `rule.Evaluate(data)` | Same rule evaluated against many data inputs          |
| Compile mode | `engine.CompileTemplate(rule)` / `CompileStrict(rule)` / `CompileMode(rule, mode)` | One rule in a templating mode other than the engine's |
| Checked      | `engine.Check(rule, mode)` / `engine.CompileChecked(rule)` | Find every problem in a rule before it runs |
| Introspection | `rule.Facts()` / `engine.Operators()` / `engine.Truthy(value)` | What a rule reads, which operators exist, the engine's truthiness |
| Session      | `engine.Session()` → `session.Evaluate(rule, data)` | Hot loops: arena reuse per goroutine             |
| Metered      | `session.EvaluateMetered(rule, data, budget)` | The operations an evaluation charges, optionally capped |
| Data handle  | `datalogic.ParseData(json)` → `session.EvaluateData(rule, data)` | Same payload evaluated many times: parse once, zero parse work per call |
| Typed        | `session.EvaluateBool/Int64/Float64/Truthy(rule, data)` | Predicates and scalar results, no JSON decode on the way out |
| Batch        | `session.EvaluateBatch(rule, datas)` / `session.EvaluateMany(rules, data)` | Many evaluations per native call, per-item errors |
| Traced       | `engine.TracedSession()` → `ts.Evaluate(rule, data)` / `ts.EvaluateMode(rule, data, mode)` | Step-level execution traces for debuggers and tooling |

## Custom operators

Build an engine with host-language operators through the fluent
builder. Each `OperatorFunc` (`func(argsJSON string) (string, error)`)
receives the pre-evaluated arguments as a JSON-array string and returns
a JSON-value string; a returned error fails the evaluation:

```go
engine, _ := datalogic.NewEngineBuilder().
    AddOperator("double", func(argsJSON string) (string, error) {
        var args []float64
        if err := json.Unmarshal([]byte(argsJSON), &args); err != nil {
            return "", err
        }
        return fmt.Sprintf("%v", args[0]*2), nil
    }).
    Build()
defer engine.Close()

out, _ := engine.Apply(`{"double":[21]}`, `{}`) // "42"
```

**Built-ins win**: a custom registration of a built-in name (`+`, `if`,
`var`, ...) never dispatches. Call `StrictOperatorNames(true)` before
`AddOperator` to have `Build` refuse such a name with
`Type == "ConfigurationError"` instead; it also refuses a name that
begins with the template key escape. A name whose family you leave out
with `Families` is free for a custom operator.

The callbacks stay alive while the `Engine`, or any `Rule`, `Session`
or `TracedSession` derived from it, is reachable, so a rule keeps
calling its custom operators after `engine.Close()`. The engine may
call an operator from any goroutine that evaluates; synchronise any
state the callback shares.

## Engine configuration

`datalogic.NewEngine()` returns an engine with the default
configuration and `datalogic.NewTemplatingEngine()` one in templating
mode. Everything else goes through the builder:

| Builder method | Effect |
|---|---|
| `SetConfigJSON(json) error` | Evaluation semantics from the shared JSON config (table below); the one setter that returns an error instead of the builder |
| `Templating(on)` | Multi-key objects compile as output templates |
| `TemplateKeyEscape(r)` | A template key starting with rune `r` is a literal output field: with `'$'`, `{"$type": ...}` emits `type` |
| `Families(names...)` | Keep the engine to the JSONLogic core plus these operator families |
| `StrictOperatorNames(on)` | Refuse a custom operator named like a built-in |
| `AddOperator(name, fn)` | Register a custom operator |
| `Build()` | Consume the builder and return the `*Engine`, or the first error a setter recorded |

`SetConfigJSON` reads the same JSON object every binding parses with
the same core code. All keys are optional; `preset` picks the starting
point and the remaining keys override individual fields on top of it.
Unknown keys, unknown enum strings and type mismatches return a
`*Error` with `Type == "ConfigurationError"`. Each call replaces the
builder's whole evaluation config; templating and registered operators
are unaffected.

```go
b := datalogic.NewEngineBuilder()
if err := b.SetConfigJSON(`{"preset": "strict", "division_by_zero": "return_null"}`); err != nil {
    log.Fatal(err) // unknown keys and values fail here
}
engine, _ := b.Build()
defer engine.Close()

_, err := engine.Apply(`{"+":[null,1]}`, `{}`)
// err != nil: the strict preset rejects non-numeric operands
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

The Rust crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html)
documents the full semantics of each knob.

`ops_budget` matters most for untrusted rules: one operation is one
node the engine dispatches, one item an iterator walks, plus what
operators charge for the data they move. Crossing the budget fails with
`Error.Type == "BudgetExceeded"` before the engine does the work, and a
`try` in the rule cannot recover from it. The count is deterministic
for a given rule, data and engine version, so every machine refuses the
same rule and data. `session.EvaluateMetered` sets a budget for one
call.

`"missing_var": "error"` makes a `var` or `val` read that finds nothing
fail with `Type == "VariableNotFound"` instead of reading as `null`. A
default (`{"var": ["x", 0]}`), a present `null`, `missing`,
`missing_some` and `exists` are not misses, and `try` catches the
error.

`Families("ExtString", "DateTime")` keeps the engine to the JSONLogic
core plus the families named, using the `family` names of
`engine.Operators()` (`ExtString`, `ExtArray`, `ExtObject`, `ExtMath`,
`ExtControl`, `ErrorHandling`, `DateTime`, `Tensor`, `Flagd`). A family
left out is not there for that engine: its names compile as unknown
operators, which fail at evaluation with `InvalidOperator`. An unknown
family name fails `Build` with `Type == "ConfigurationError"`.

## Error handling

Every fallible operation returns a `*datalogic.Error` on failure:

| Field | Contents |
|---|---|
| `Message` | Human-readable error string |
| `Type` | The engine's stable error tag; match on this |
| `Operator` | The innermost failing operator (`"+"`, `"var"`, ...); empty when the failure didn't happen inside a named operator |
| `PathJSON` | JSON array of `{node_id, operator, arg_index, json_pointer}` steps from the rule root to the failing node; empty when no compiled rule was in scope |
| `NodeIDsJSON` | The compiled-node breadcrumb, leaf to root, as a JSON array of ids |
| `DiagnosticsJSON` | `Type == "CompileError"` only: every problem `CompileChecked` found |

```go
_, err := engine.Apply(`{"+": [1, {"*": ["x", 2]}]}`, `{}`)
var dlErr *datalogic.Error
if errors.As(err, &dlErr) {
    fmt.Println(dlErr.Type)     // "Thrown" (a NaN payload under the default config)
    fmt.Println(dlErr.Operator) // "*"
    fmt.Println(dlErr.PathJSON) // [{..."operator":"+"...},{..."json_pointer":"/+/1",..."operator":"*"}]
}
```

Besides the engine's tags, the binding reports `TypeMismatch` (a typed
evaluation whose result has the wrong type), `InvalidArgument` (nil or
closed handles, a rule compiled by a different engine than the
session's), `CompileError` (from `CompileChecked`) and `InternalError`
(a panic caught at the FFI boundary). A batch item whose error JSON is
malformed decodes as `Type == "InternalError"` with the raw JSON as
the `Message`, as in the JVM, .NET and PHP bindings.

## Threading

| Type      | Pattern                                                                            |
|-----------|------------------------------------------------------------------------------------|
| `Engine`  | Construct once; share across goroutines                                            |
| `Rule`    | Compile once; share across goroutines: `Evaluate` is safe to call from many        |
| `DataHandle` | Parse once; immutable, share across goroutines and engines                      |
| `Session` | One per goroutine: the per-task workhorse                                          |
| `TracedSession` | Share across goroutines; every `Evaluate` uses a fresh internal arena        |
| `EngineBuilder` | One goroutine; `Build` before sharing the `Engine`                           |

`Close` is safe to call more than once and from several goroutines at
once: exactly one call frees the handle. Closing a handle while another
goroutine is still using it is not supported; finish or join that work
first. A GC finalizer frees a handle you never close, and an
`EngineBuilder` you never `Build`.

## Traced evaluation

`Engine.TracedSession()` mirrors the engine's trace tier: each
`Evaluate` compiles the rule with the optimizer disabled, so every
operator in the rule surfaces as an execution step, and returns a JSON
envelope instead of a bare result:

```go
ts := engine.TracedSession()
defer ts.Close()

out, _ := ts.Evaluate(`{"+":[{"var":"x"},1]}`, `{"x":41}`)
// {"result":42,"expression_tree":{...},"steps":[...],"pointers":{"1":"/+/0/var","2":"/+/0","3":"/+/1","4":""}}
```

`EvaluateMode(rule, data, mode)` compiles the rule in a `Mode`, so you
can trace a rule you compile with `CompileTemplate`. The envelope shape
is shared with the WASM binding, so trace consumers (debuggers,
visualizers) see one format across languages:

| Field | Contents |
|---|---|
| `result` | The evaluation result (object keys in the order the rule produced them), `null` on engine error |
| `expression_tree` | The compiled expression tree |
| `steps` | Ordered execution steps with per-node results |
| `pointers` | Each node id mapped to the RFC 6901 JSON Pointer of the rule value it was compiled from; absent when the rule does not compile |
| `error`, `structured_error` | Present only when the engine failed. Rule parse and evaluation errors land here, not in the Go error return, which is reserved for binding-level failures. |

Tracing pays for compile-per-call plus step recording. Use it for
debugging and tooling, not hot paths.

## Performance

<!-- canonical-bench v5.1 -->
Geomean across 51 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **10.3 ns/op**, 7.0× faster than json-logic-engine (compiled, the fastest JS engine), 28.1× faster than jsonlogic-rs (the closest Rust alternative), and 83.6× faster than the json-logic-js reference implementation. The WASM build under Node measures 900.5 ns geomean (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The cgo boundary adds a per-call marshalling cost on top of the core
numbers; the
[boundary report](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BINDINGS-OVERHEAD.md)
measures it per tier.

## How it links

The binding is a cgo wrapper over the shared C ABI:

```
datalogic-rs (Rust)  →  bindings/c/  →  libdatalogic_c.a  →  cgo → Go
```

Go is the only binding that links the staticlib; the JVM, .NET, and PHP
bindings load the same C ABI as a shared library (cdylib).

The binding targets C ABI **v2**: inputs cross as (pointer, length)
UTF-8 with Go string bytes passed zero-copy, results are copied out of
session-owned buffers or owned bufs, and errors travel as status codes
plus an error handle (no thread-local state, so no OS-thread pinning).
At load time the package checks `datalogic_abi_version() == 2` and
panics on a mismatch, so a stale `libdatalogic_c.a` fails at init
instead of corrupting memory at call time.

## Building from source

You need Go 1.25 or newer, a C compiler and a Rust toolchain.
`make build` compiles the C ABI crate with the `go-release` profile and
stages the host platform's staticlib and header for cgo:

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs
cd datalogic-rs/bindings/go
make build   # build the staticlib in bindings/c, stage lib/<os>_<arch>/ + include/
make test    # go test -v ./...
```

[DEVELOPMENT.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/DEVELOPMENT.md)
covers the repo-wide workflow;
[bindings/BINDINGS.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/BINDINGS.md)
covers how releases stage the staticlibs onto `bindings/go/v*` tags.

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [Documentation: Go](https://goplasmatic.github.io/datalogic-rs/go/installation.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)
- [C ABI internals](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c#readme)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
