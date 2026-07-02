# Installation & cgo Setup

The Go binding `datalogic-go` bridges Go and the underlying Rust core statically using `cgo`.

## Go Module Path

To add the Go module dependency (note the `/v5` major-version suffix required by Go Modules for versions 2 and above):

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

cgo build tags automatically select the correct static library (`libdatalogic_c.a`) at build time. 

## Requirements

*   **Compilation:** You only need a standard C compiler (e.g. `gcc` or `clang` / Xcode command line tools) to link the static library during `go build`.
*   **No Rust Required:** You do **not** need the Rust toolchain installed on the machine building the Go application; the compiled Rust engine is already packaged inside the static library.
