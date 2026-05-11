# Adding a language binding

Each language binding lives as a sibling crate under `packages/<lang>/` and
follows the same conventions, so a new binding can be added without
re-deriving the layout.

## Naming

Every binding's published artifact follows the **`datalogic-<lang>`** pattern.
`<lang>` is the short, established suffix for the target language — `rs` for
Rust, `py` for Python, `wasm` for WebAssembly, `rb` for Ruby, `go` for Go,
`java` / `kt` / `swift` for the JVM/mobile family.

| Language | Internal Cargo crate | Published artifact | Registry |
|---|---|---|---|
| Rust | `datalogic-rs` | `datalogic-rs` | crates.io |
| WebAssembly | `datalogic-wasm` | **`@goplasmatic/datalogic`** (grandfathered, predates this convention) | npm |
| Python | `datalogic-py` | `datalogic-py` (PyPI) → `import datalogic_py` | PyPI |
| C ABI | `datalogic-c` | shared `cdylib`/`staticlib` + header (consumed by Go/PHP/JVM in-tree, not separately published) | — |
| Go | `datalogic-go` | `github.com/GoPlasmatic/datalogic-rs/packages/go` (in-tree module) | Go modules |
| _future_ Node native | `datalogic-node` | `@goplasmatic/datalogic-node` (alongside the WASM `@goplasmatic/datalogic`) | npm |
| _future_ PHP | `datalogic-php` | `goplasmatic/datalogic-php` | Packagist |
| _future_ JVM | `datalogic-jvm` | `com.goplasmatic:datalogic` | Maven Central |
| _future_ Ruby | `datalogic-rb` | `datalogic-rb` | RubyGems |

For Python the PyPI distribution name is `datalogic-py` but the Python
**module** name is `datalogic_py` — Python doesn't allow hyphens in
import paths, and PyPI's normalisation already treats hyphens and
underscores as equivalent for installation.

The npm WASM package is `@goplasmatic/datalogic` (without the `-wasm`
suffix) only because it predates the convention; renaming it would force
every existing consumer to update. New language packages should follow
the convention without exception.

## Convention

| Concern | Decision |
|---|---|
| Location | `packages/<lang>/` (sibling of `core`, `wasm`, `python`, `ui`) |
| Workspace | **Excluded** from the root workspace (own `[workspace]` block) |
| Cargo | `crate-type = ["cdylib", "rlib"]` — `cdylib` for the FFI artifact, `rlib` so Rust consumers can also link it |
| Dep on core | `datalogic-rs = { path = "../core", version = "5.x", features = [...explicit list...] }` — the binding inlines the feature set it wants |
| Core feature | **No umbrella feature in core.** The binding owns its operator surface; `packages/core/Cargo.toml` stays free of binding-specific bundling so the published crate is binding-agnostic |
| Tests | `packages/<lang>/tests/` in the binding's native test runner (pytest, jest, …) |
| CI | A pair of jobs added to `.github/workflows/release.yml` — `<lang>-build-*` (one or more, possibly a matrix) followed by `publish-<lang>` (`needs: publish-crate` so a binding never ships ahead of core) |
| Release tags | `v*` (e.g. `v5.0.0`) — single unified trigger. One tag push runs validate + tests, publishes core, then fans out every binding in parallel. |
| Versioning | Bindings track the core version exactly (5.0.0 → 5.0.0). `validate` fails if any binding's `Cargo.toml` / `pyproject.toml` / `package.json` drifts from core. |

## Why these conventions

- **Excluded from root workspace.** Bindings pull in language-specific
  build deps (pyo3, napi, jni, …) that bloat the default `cargo test
  --workspace --all-features` and require contributors to install
  language toolchains (Python interpreter, Node.js, JDK) just to run
  Rust tests. Excluding keeps the core's dev loop fast.

- **`cdylib` + `rlib`.** `cdylib` is the importable artifact every
  binding needs (`.so`/`.pyd`/`.dll`). `rlib` lets a downstream Rust
  crate (e.g. integration tests, the binding's own benchmarks) link it
  as a normal dep without duplicating source.

- **No umbrella feature in core.** Each binding inlines the explicit
  feature list it wants in its `datalogic-rs` dep stanza
  (`features = ["serde_json", "templating", "datetime", …]`). The core
  crate stays binding-agnostic — adding or removing a binding never
  touches `packages/core/Cargo.toml`. The trade-off is that bumping a
  shared operator family across all bindings is a multi-file edit, but
  that's a rare event and explicit listing makes each binding's surface
  obvious without cross-referencing.

- **Single unified release workflow.** Every binding's release jobs
  live in `.github/workflows/release.yml`. The flow is: validate +
  tests → publish core → fan out bindings in parallel
  (wasm → ui chain alongside python wheels → publish-python). One
  tag push, one workflow run, one set of status checks. The trade-off
  is that a Python wheel-build failure shows up in the same run as
  core/wasm — but the bindings are independent jobs, so a failure in
  one doesn't roll back the others.

## Existing bindings

| Binding | Path | Tech | Publishes to |
|---|---|---|---|
| WebAssembly | `packages/wasm/` | wasm-bindgen + wasm-pack | npm: `@goplasmatic/datalogic` |
| Python | `packages/python/` | pyo3 + maturin (abi3-py310) | PyPI: `datalogic-py` |
| C ABI | `packages/c/` | `extern "C"` + cbindgen-generated header | (not separately published — consumed in-tree by Go/PHP/JVM) |
| Go | `packages/go/` | cgo over `packages/c/` (static link to `libdatalogic_c.a`) | Go modules: `github.com/GoPlasmatic/datalogic-rs/packages/go` |

## Shared C ABI (`packages/c/`)

The C ABI is **not a publishable binding by itself** — it's the canonical
FFI boundary that lower-level language packages consume. Languages whose
Rust binding tools (pyo3, napi-rs, magnus, wasm-bindgen) provide a more
ergonomic surface skip the C ABI and target their runtime directly.
Languages without a mature Rust binding tool (Go, PHP, JVM via FFI)
consume the C ABI's cdylib + generated header.

| Binding route | Goes through `packages/c/`? | Why |
|---|---|---|
| Python (pyo3) | No | pyo3 gives ergonomic dict/list marshalling — better than JSON-string FFI |
| WASM (wasm-bindgen) | No | The browser doesn't have a C ABI — wasm-bindgen is the only path |
| Node native (napi-rs) | No | napi-rs exposes V8 types directly; cheaper than JSON-roundtrip |
| Ruby (magnus) | No | magnus mirrors pyo3 — direct Ruby type marshalling |
| Go (cgo) | **Yes** | No first-class Rust↔Go binding tool |
| PHP (FFI) | **Yes** | PHP's FFI extension consumes any cdylib + header |
| JVM (JNA / JNR-FFI) | **Yes** | Avoids hand-writing JNI per platform |

The C ABI's surface is JSON-in/JSON-out throughout — no struct
marshalling at the boundary. Languages that want native-type fast paths
either go around the C ABI (rows above) or add a thin native shim on top.

### Cross-platform binary distribution for C-ABI bindings

Languages that route through `packages/c/` need the static / shared
library at the consumer's build (Go cgo) or runtime (PHP FFI, JVM JNA)
time. The release workflow handles this with a shared (os, arch)
matrix that compiles `packages/c/` once on a native runner per
platform, plus a per-language packaging job that picks up the matrix
artifacts and ships them in that language's idiomatic distribution
channel.

| Lang | Distribution shape | Lib type | Path in artifact |
|---|---|---|---|
| Go | Git tag `packages/go/vX.Y.Z` with binaries staged in source tree | `.a` static | `packages/go/lib/<os>_<arch>/libdatalogic_c.a` |
| PHP | Composer package with platform binaries under `bin/` | `.so` / `.dylib` / `.dll` | `bin/<os>-<arch>/` (loaded via `FFI::cdef`) |
| JVM | JAR with platform binaries under `META-INF/native/` | `.so` / `.dylib` / `.dll` | `META-INF/native/<os>-<arch>/` (loaded via JNA `Native.load`) |

The matrix in `.github/workflows/release.yml` (`go-build-staticlib`)
currently runs only the Go binding's matrix. When PHP / JVM bindings
land, they reuse the same matrix outputs — the matrix becomes a
producer of staticlib + cdylib artifacts, and each binding's
`publish-*` job is a downstream consumer.

Supported (os, arch) matrix:

| OS | Arch | Runner | Rust target |
|---|---|---|---|
| Linux | amd64 | `ubuntu-latest` | `x86_64-unknown-linux-gnu` |
| Linux | arm64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` |
| macOS | amd64 | `macos-14` (cross from arm64 host) | `x86_64-apple-darwin` |
| macOS | arm64 | `macos-14` (native) | `aarch64-apple-darwin` |
| Windows | amd64 | `windows-latest` | `x86_64-pc-windows-gnu` (mingw — for cgo compat; PHP/JVM may need `msvc` too) |

## Open candidates

The order below reflects the current implementation plan:

1. **Node native** — `packages/node/` via `napi-rs`, published as
   `@goplasmatic/datalogic-node` alongside the existing
   `@goplasmatic/datalogic` (WASM)
2. **PHP** — `packages/php/` via PHP FFI over `packages/c/`
3. **JVM** — `packages/jvm/` via JNA or JNR-FFI over `packages/c/`
4. **Ruby** — `packages/ruby/` via `magnus`

Other plausible future bindings:

- **Swift** (`swift-bridge` or UniFFI; UniFFI also covers Kotlin)
- **.NET** (`csbindgen` over `packages/c/`)
- **Elixir** (`rustler` NIFs)

The core engine is `Send + Sync` and exposes a clean compile-once /
evaluate-many surface (`Engine`, `Logic`, `Session`), so the same
binding shape transfers to any language with a Rust FFI story.
