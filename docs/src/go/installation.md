# Installation & cgo Setup

The Go binding `datalogic-go` links the Rust core into your Go program statically through `cgo`.

## Go Module Path

Add the Go module dependency. Go Modules requires the `/v5` major-version suffix for versions 2 and above:

```bash
go get github.com/GoPlasmatic/datalogic-rs/bindings/go/v5@latest
```

Import it in your Go code:

```go
import datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
```

## Binary Staging

`datalogic-go` tags ship prebuilt static libraries for the following targets:

| OS | Architecture | Subdirectory |
|---|---|---|
| Linux | amd64 | `linux_amd64/` |
| Linux | arm64 | `linux_arm64/` |
| macOS | amd64 (Intel) | `darwin_amd64/` |
| macOS | arm64 (Apple Silicon) | `darwin_arm64/` |
| Windows | amd64 | `windows_amd64/` |
| Windows | arm64 | `windows_arm64/` |

cgo build tags select the matching static library (`libdatalogic_c.a`) at build time.

## Requirements

*   **Go 1.25 or newer** (the module's `go.mod` directive; older toolchains with `GOTOOLCHAIN=auto` download it on demand).
*   **Compilation:** You only need a standard C compiler (e.g. `gcc` or `clang` / Xcode command line tools) to link the static library during `go build`.
*   **No Rust Required:** You do **not** need the Rust toolchain installed on the machine building the Go application; the static library already contains the compiled Rust engine.
