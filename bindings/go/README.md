# datalogic Go binding

Go binding for the [`datalogic-rs`](../../crates/datalogic-rs) JSONLogic
engine. Routes through the shared C ABI at [`bindings/c/`](../c) via
cgo, linking `libdatalogic_c.a` statically — no runtime shared-library
dependency for end-user binaries.

Same rules, same semantics as the Rust crate. For the cross-runtime
overview and the API-tier model every binding implements, see the
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

## In-tree development

Contributors working in this monorepo build the static library locally
instead of using a tagged release. The `lib/` and `include/`
directories are gitignored on `main`; only release tags carry the
prebuilt artifacts.

```sh
cd bindings/go
make build      # cargo-builds bindings/c, stages lib/<host>/ + include/
make test       # runs `go test -v ./...`
make print-platform   # prints the host's lib/ subdirectory name
```

Requirements for the in-tree path:

- Go 1.22+
- A Rust toolchain (the underlying C ABI crate lives in `bindings/c/`)
- A C compiler (xcode-select on macOS, gcc/clang on Linux)
- The Makefile auto-detects host OS/arch and stages into
  `lib/<host_os>_<host_arch>/` — only the matching `cgo_*_*.go` file
  needs that subdirectory populated locally.

Re-run `make build` after any change to the C ABI's Rust source —
cargo's incremental compile makes this fast, and the staging step is
a couple of copies.

## How releases are built

CI (`.github/workflows/release.yml`) runs a matrix on a `v*` tag push,
producing `libdatalogic_c.a` on a native runner for each supported
(os, arch). The `publish-go` job collects all artifacts, stages them
into `bindings/go/lib/<os>_<arch>/` and the header into
`bindings/go/include/` on a synthetic commit, and pushes a
`bindings/go/v<version>` tag pointing at that commit. The synthetic
commit is reachable only through the tag — `main` stays
binary-free.

## API reference

The Go binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#choosing-your-api-five-tiers-one-engine).

| Tier         | Entry point                                  | Use when                                                |
|--------------|----------------------------------------------|---------------------------------------------------------|
| One-shot     | `datalogic.Apply(rule, data)`                | Ad-hoc evaluation, one rule + one data shape            |
| Engine       | `datalogic.NewEngine().Apply(rule, data)`    | Engine reuse without compile-once                       |
| Compile once | `engine.Compile(rule)` → `rule.Evaluate(data)` | Same rule evaluated against many data inputs          |
| Session      | `engine.Session()` → `session.Evaluate(rule, data)` | Hot loops — arena reuse per goroutine            |

### Quick start

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

## Error handling

Every fallible operation returns a `*datalogic.Error` on failure,
carrying the engine's stable error tag, the failing operator (when
applicable), and a JSON-encoded path from the rule's compiled tree:

```go
_, err := rule.Evaluate(`{}`)
if err != nil {
    e := err.(*datalogic.Error)
    fmt.Println(e.Type)      // "Thrown" | "ParseError" | "NaN" | ...
    fmt.Println(e.Operator)  // outermost failing operator name
    fmt.Println(e.PathJSON)  // JSON array string of {operator, json_pointer, ...}
}
```

## Threading

| Type      | Pattern                                                                            |
|-----------|------------------------------------------------------------------------------------|
| `Engine`  | Construct once; share across goroutines                                            |
| `Rule`    | Compile once; share across goroutines — `Evaluate` is safe to call from many       |
| `Session` | One per goroutine — the per-task workhorse                                         |

## Performance

This package wraps the same Rust engine measured as `dlrs:engine` in the
[cross-library benchmark][bench] — geomean **9.7 ns/op across 44 operator
suites**, ~5× faster than `json-logic-engine` (compiled JS) and ~22×
faster than `jsonlogic-rs` (the closest native-Rust alternative). The
cgo boundary adds a small per-call marshalling cost on top.

[bench]: https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md

## How it links

The binding is a cgo wrapper over the shared C ABI:

```
datalogic-rs (Rust)  →  bindings/c/  →  libdatalogic_c.a  →  cgo → Go
```

`make build` keeps `lib/` and `include/` in sync with the Rust source.
The same staticlib will eventually back the PHP and JVM bindings.

## Learn more

- [Repo README](https://github.com/GoPlasmatic/datalogic-rs#readme) — cross-runtime overview, all binding READMEs
- [Rust crate README](../../crates/datalogic-rs/README.md) — engine design, custom operators, configuration knobs
- [C ABI README](../c/README.md) — the FFI boundary this binding consumes
- [Full documentation](https://goplasmatic.github.io/datalogic-rs/) — long-form guide, operator reference
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
