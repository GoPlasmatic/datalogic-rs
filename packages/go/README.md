# datalogic Go binding

Go binding for the [`datalogic-rs`](../core) JSONLogic engine. Routes
through the shared C ABI at [`packages/c/`](../c) via cgo, linking
`libdatalogic_c.a` statically — no runtime shared-library dependency
for end-user binaries.

## Install

```sh
go get github.com/GoPlasmatic/datalogic-rs/packages/go@latest
```

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
import datalogic "github.com/GoPlasmatic/datalogic-rs/packages/go"
```

## In-tree development

Contributors working in this monorepo build the static library locally
instead of using a tagged release. The `lib/` and `include/`
directories are gitignored on `main`; only release tags carry the
prebuilt artifacts.

```sh
cd packages/go
make build      # cargo-builds packages/c, stages lib/<host>/ + include/
make test       # runs `go test -v ./...`
make print-platform   # prints the host's lib/ subdirectory name
```

Requirements for the in-tree path:

- Go 1.22+
- A Rust toolchain (the underlying C ABI crate lives in `packages/c/`)
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
into `packages/go/lib/<os>_<arch>/` and the header into
`packages/go/include/` on a synthetic commit, and pushes a
`packages/go/v<version>` tag pointing at that commit. The synthetic
commit is reachable only through the tag — `main` stays
binary-free.

## Quick start

```go
package main

import (
    "fmt"
    datalogic "github.com/GoPlasmatic/datalogic-rs/packages/go"
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

| Type | Safe to share across goroutines? |
|---|---|
| `Engine` | Yes |
| `Rule` | Yes — `Evaluate` is safe to call from many goroutines |
| `Session` | **No** — open one per goroutine |

## How it links

The binding is a cgo wrapper over the shared C ABI:

```
datalogic-rs (Rust)  →  packages/c/  →  libdatalogic_c.a  →  cgo → Go
```

`make build` keeps `lib/` and `include/` in sync with the Rust source.
The same staticlib will eventually back the PHP and JVM bindings.
