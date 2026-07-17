# datalogic Go binding

[![Go Reference](https://pkg.go.dev/badge/github.com/GoPlasmatic/datalogic-rs/bindings/go/v5.svg)](https://pkg.go.dev/github.com/GoPlasmatic/datalogic-rs/bindings/go/v5)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs) — one engine, every runtime.

Go binding for the
[`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs)
JSONLogic engine. Routes through the shared C ABI at
[`bindings/c/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c)
via cgo, linking `libdatalogic_c.a` statically — no runtime
shared-library dependency for end-user binaries.

Same rules, same semantics as the Rust crate: every binding runs the
same core and passes the same 1,565-case conformance battery
(54 suites). For the cross-runtime overview and the API-tier model
every binding implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** This Go binding is new — there is no v4 Go package. If
> you were calling the v4 Rust crate or the v4 `@goplasmatic/datalogic`
> WASM package, the engine's v4 → v5 changes are catalogued in
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md).

## Install

```sh
go get github.com/GoPlasmatic/datalogic-rs/bindings/go/v5@latest
```

The `/v5` suffix is required by Go modules for any major version ≥ 2
(see [Go modules ref — major version
suffixes](https://go.dev/ref/mod#major-version-suffixes)); the
binding's own version tracks the core crate's, so `v5.x.y` lives at
`/v5`, `v6.x.y` will live at `/v6`, etc.

Released tags ship prebuilt static libraries for:

| OS / Arch | `lib/` subdirectory | Rust target |
|---|---|---|
| Linux x86_64 | `linux_amd64/` | `x86_64-unknown-linux-gnu` |
| Linux ARM64 | `linux_arm64/` | `aarch64-unknown-linux-gnu` |
| macOS Intel | `darwin_amd64/` | `x86_64-apple-darwin` |
| macOS Apple Silicon | `darwin_arm64/` | `aarch64-apple-darwin` |
| Windows x86_64 | `windows_amd64/` | `x86_64-pc-windows-gnu` (mingw-w64) |

cgo build tags in `cgo_<os>_<arch>.go` pick the right one at build time.
You only need a C compiler to link — no Rust toolchain required.

## Module path

```go
import datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
```

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

## Development

The in-tree development workflow (Makefile targets, toolchain
requirements) lives in
[DEVELOPMENT.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/DEVELOPMENT.md),
and the release pipeline that stages prebuilt staticlibs onto
`bindings/go/v*` tags in
[bindings/BINDINGS.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/BINDINGS.md).

## API reference

The Go binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).

| Tier         | Entry point                                  | Use when                                                |
|--------------|----------------------------------------------|---------------------------------------------------------|
| One-shot     | `datalogic.Apply(rule, data)`                | Ad-hoc evaluation, one rule + one data shape            |
| Engine       | `datalogic.NewEngine().Apply(rule, data)`    | Engine reuse without compile-once                       |
| Compile once | `engine.Compile(rule)` → `rule.Evaluate(data)` | Same rule evaluated against many data inputs          |
| Session      | `engine.Session()` → `session.Evaluate(rule, data)` | Hot loops — arena reuse per goroutine            |
| Data handle  | `datalogic.ParseData(json)` → `session.EvaluateData(rule, data)` | Same payload evaluated many times: parse once, zero parse work per call |
| Typed        | `session.EvaluateBool/Int64/Float64/Truthy(rule, data)` | Predicates and scalar results, no JSON decode on the way out |
| Batch        | `session.EvaluateBatch(rule, datas)` / `session.EvaluateMany(rules, data)` | Many evaluations per native call, per-item errors |
| Traced       | `engine.TracedSession()` → `ts.Evaluate(rule, data)` | Step-level execution traces for debuggers and tooling |

## Data handles, typed results, and batch evaluation

New with C ABI v2 (5.0.1). A `DataHandle` is an immutable, pre-parsed
JSON document: parse a payload once with `datalogic.ParseData` and every
evaluation against it skips JSON parsing entirely. Handles are
engine-independent (one handle can feed rules compiled by different
engines), safe for concurrent use from multiple goroutines, and not
consumed by evaluation. `Close()` them after the last use (a GC
finalizer backstops forgotten closes, best-effort).

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
result is not of the requested type. `EvaluateTruthy` coerces any result
through the engine's configured truthiness rules (the same coercion
`if`/`and`/`or` apply).

The batch entry points evaluate a whole set in one native call and
report failures per item, so one bad input never poisons its
neighbours:

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

The call-level `error` return is reserved for argument problems (nil
session, a rule compiled by a different engine, ...). Typed and batch
evaluations take data handles only; rules must belong to the session's
engine, and sessions stay single-goroutine.

## Custom operators

Build an engine with host-language operators via the fluent builder. Each
`OperatorFunc` (`func(argsJSON string) (string, error)`) receives the
pre-evaluated arguments as a JSON-array string and returns a JSON-value
string:

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
`var`, ...) never dispatches.

## Engine configuration

Non-default evaluation behavior (strict arithmetic, division-by-zero
policy, truthiness flavor, recursion limits) is set on the builder as a
JSON object string, parsed by the same shared config parser every
binding uses:

```go
b := datalogic.NewEngineBuilder()
if err := b.SetConfigJSON(`{"preset": "strict", "division_by_zero": "return_null"}`); err != nil {
    log.Fatal(err) // unknown keys and values fail loudly, not silently
}
engine, _ := b.Build()
defer engine.Close()

_, err := engine.Apply(`{"+":[null,1]}`, `{}`)
// err != nil: the strict preset rejects non-numeric operands
```

All keys are optional. `preset` (`"default"`, `"safe_arithmetic"`, or
`"strict"`) picks the starting point; the remaining keys override
individual fields on top of it: `arithmetic_nan_handling`,
`division_by_zero`, `loose_equality_errors`, `truthy_evaluator`,
`numeric_coercion`, and `max_recursion_depth`. The accepted values for
each key are listed on the `SetConfigJSON` doc comment; the underlying
knobs are described in the
[Rust crate README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme).

## Traced evaluation

`Engine.TracedSession()` mirrors the engine's trace tier: each
`Evaluate` compiles the rule with the optimizer disabled, so every
operator in the rule surfaces as an execution step, and returns a JSON
envelope instead of a bare result:

```go
ts := engine.TracedSession()
defer ts.Close()

out, _ := ts.Evaluate(`{"+":[{"var":"x"},1]}`, `{"x":41}`)
// {"result":42,"expression_tree":{...},"steps":[...]}
```

The envelope shape is shared with the WASM binding, so trace consumers
(debuggers, visualizers) see one format across languages:

| Field | Contents |
|---|---|
| `result` | The evaluation result, `null` on engine error |
| `expression_tree` | The compiled expression tree |
| `steps` | Ordered execution steps with per-node results |
| `error`, `structured_error` | Present only when the engine failed. Rule parse and evaluation errors land here, not in the Go error return, which is reserved for binding-level failures. |

Tracing pays for compile-per-call plus step recording. Use it for
debugging and tooling, not hot paths.

## Error handling

Every fallible operation returns a `*datalogic.Error` on failure,
carrying the engine's stable error tag, the failing operator (when
applicable), and a JSON-encoded path from the rule's compiled tree:

```go
_, err := rule.Evaluate(`{}`)
if err != nil {
    e := err.(*datalogic.Error)
    fmt.Println(e.Type)      // "Thrown" | "ParseError" | "InvalidOperator" | ...
    fmt.Println(e.Operator)  // outermost failing operator name
    fmt.Println(e.PathJSON)  // JSON array string of {operator, json_pointer, ...}
}
```

## Threading

| Type      | Pattern                                                                            |
|-----------|------------------------------------------------------------------------------------|
| `Engine`  | Construct once; share across goroutines                                            |
| `Rule`    | Compile once; share across goroutines — `Evaluate` is safe to call from many       |
| `DataHandle` | Parse once; immutable, share across goroutines and engines                      |
| `Session` | One per goroutine — the per-task workhorse                                         |
| `TracedSession` | Share across goroutines; every `Evaluate` uses a fresh internal arena        |

## Performance

<!-- canonical-bench v5.1 -->
Geomean across 51 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **10.3 ns/op**, 7.0× faster than json-logic-engine (compiled, the fastest JS engine), 28.1× faster than jsonlogic-rs (the closest Rust alternative), and 83.6× faster than the json-logic-js reference implementation. The WASM build under Node measures 900.5 ns geomean (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The cgo boundary adds a small per-call marshalling cost on top of the
core numbers.

## How it links

The binding is a cgo wrapper over the shared C ABI:

```
datalogic-rs (Rust)  →  bindings/c/  →  libdatalogic_c.a  →  cgo → Go
```

`make build` keeps `lib/` and `include/` in sync with the Rust source.
Go is the only binding that links the staticlib; the JVM, .NET, and PHP
bindings consume the same C ABI as a shared library (cdylib).

The binding targets C ABI **v2**: inputs cross as (pointer, length)
UTF-8 with Go string bytes passed zero-copy, results are copied out of
session-owned buffers or owned bufs, and errors travel as status codes
plus an error handle (no thread-local state, so no OS-thread pinning).
At load time the package asserts `datalogic_abi_version() == 2` and
panics on a stale library, so a mismatched `libdatalogic_c.a` fails
loudly at init instead of corrupting at call time.

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [Documentation — Go](https://goplasmatic.github.io/datalogic-rs/go/installation.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)
- [C ABI internals](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c#readme)
