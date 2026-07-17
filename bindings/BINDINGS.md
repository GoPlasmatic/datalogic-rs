# Adding a language binding

Each language binding lives as a sibling crate under `bindings/<lang>/` and
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
| WebAssembly | `datalogic-wasm` | **`@goplasmatic/datalogic-wasm`** | npm |
| Node native | `datalogic-node` | `@goplasmatic/datalogic-node` (first-class Node target — WASM `@goplasmatic/datalogic-wasm` ships alongside for browsers / Deno / Bun / Workers) | npm |
| Python | `datalogic-py` | `datalogic-py` (PyPI) → `import datalogic_py` | PyPI |
| C ABI | `datalogic-c` | shared `cdylib`/`staticlib` + header (consumed by Go/JVM/.NET/PHP in-tree, not separately published) | — |
| Go | `datalogic-go` | `github.com/GoPlasmatic/datalogic-rs/bindings/go/v5` (in-tree module; `/v5` major-version suffix required by Go modules) | Go modules |
| JVM | (no Cargo crate — Maven module) | `io.github.goplasmatic:datalogic` | Maven Central |
| .NET | (no Cargo crate — .NET project) | `Goplasmatic.Datalogic` | NuGet |
| PHP | (no Cargo crate — Composer package) | `goplasmatic/datalogic` | Packagist |
| _future_ Ruby | `datalogic-rb` | `datalogic-rb` | RubyGems |

For Python the PyPI distribution name is `datalogic-py` but the Python
**module** name is `datalogic_py` — Python doesn't allow hyphens in
import paths, and PyPI's normalisation already treats hyphens and
underscores as equivalent for installation.

Prior to v5 the WASM package shipped as `@goplasmatic/datalogic` (grandfathered
from before the convention existed). v5 brings it in line: `@goplasmatic/datalogic-wasm`
is the canonical name going forward. v4.x consumers still on the old name see
a deprecation notice on `npm install` pointing them at the new name.

## Convention

| Concern | Decision |
|---|---|
| Location | `bindings/<lang>/` (sibling of the other bindings; the core crate lives at `crates/datalogic-rs`) |
| Workspace | **Excluded** from the root workspace (own `[workspace]` block) |
| Cargo | `crate-type = ["cdylib", "rlib"]` — `cdylib` for the FFI artifact, `rlib` so Rust consumers can also link it |
| Dep on core | `datalogic-rs = { path = "../../crates/datalogic-rs", version = "5.x", features = [...explicit list...] }` — the binding inlines the feature set it wants |
| Core feature | **No umbrella feature in core.** The binding owns its operator surface; `crates/datalogic-rs/Cargo.toml` stays free of binding-specific bundling so the published crate is binding-agnostic |
| Tests | `bindings/<lang>/tests/` in the binding's native test runner (pytest, jest, …) |
| CI | A pair of jobs added to `.github/workflows/release.yml` — `<lang>-build-*` (one or more, possibly a matrix) followed by `publish-<lang>` (`needs: publish-crate` so a binding never ships ahead of core) |
| Release tags | `v*` (e.g. `v5.1.0`) — single unified trigger. One tag push runs validate + tests, publishes core, then fans out every binding in parallel. |
| Versioning | Bindings track the core version exactly (5.1.0 → 5.1.0). `validate` fails if any binding's `Cargo.toml` / `pyproject.toml` / `package.json` drifts from core. |

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
  touches `crates/datalogic-rs/Cargo.toml`. The trade-off is that bumping a
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
| WebAssembly | `bindings/wasm/` | wasm-bindgen + wasm-pack | npm: `@goplasmatic/datalogic-wasm` |
| Node native | `bindings/node/` | napi-rs + napi-cli (per-platform `.node` prebuilds with `optionalDependencies`) | npm: `@goplasmatic/datalogic-node` |
| Python | `bindings/python/` | pyo3 + maturin (abi3-py310) | PyPI: `datalogic-py` |
| C ABI | `bindings/c/` | `extern "C"` + cbindgen-generated header | (not separately published — consumed in-tree by Go/JVM/.NET/PHP) |
| Go | `bindings/go/` | cgo over `bindings/c/` (static link to `libdatalogic_c.a`) | Go modules: `github.com/GoPlasmatic/datalogic-rs/bindings/go/v5` |
| JVM | `bindings/jvm/` | FFM over `bindings/c/` cdylib | Maven Central: `io.github.goplasmatic:datalogic` |
| .NET | `bindings/dotnet/` | P/Invoke (`LibraryImport`) over `bindings/c/` cdylib | NuGet: `Goplasmatic.Datalogic` |
| PHP | `bindings/php/` | PHP FFI (`FFI::cdef`) over `bindings/c/` cdylib | Packagist: `goplasmatic/datalogic` |

### Custom operator support

Every binding exposes a way to register user-defined JSONLogic operators
written in the host language. The cross-binding contract is the same:
the host callback receives the operator's pre-evaluated arguments as a
JSON-array string and returns a JSON-value string. Bindings differ only
in how the registration is plumbed into their constructor surface:

| Binding | API shape | Example |
|---|---|---|
| WASM (`@goplasmatic/datalogic-wasm`) | Options bag on `new Engine(opts)` | `new Engine({ customOperators: { foo: argsJson => '...' } })` |
| Node (`@goplasmatic/datalogic-node`) | Second positional arg on `new Engine(opts, ops)` | `new Engine({}, { foo: argsJson => '...' })` |
| Python (`datalogic-py`) | Keyword arg on `Engine(...)` | `Engine(custom_operators={"foo": lambda a: "..."})` |
| C ABI (`bindings/c/`) | Explicit builder + function-pointer callback | `datalogic_engine_builder_add_operator(b, "foo", cb, user_data)` |
| Go (`bindings/go/`) | Fluent builder over the C ABI | `NewEngineBuilder().AddOperator("foo", fn).Build()` |
| JVM (`io.github.goplasmatic:datalogic`) | Fluent builder | `Engine.builder().addOperator("foo", argsJson -> "...").build()` |
| .NET (`Goplasmatic.Datalogic`) | Fluent builder | `Engine.Builder().AddOperator("foo", argsJson => "...").Build()` |
| PHP (`goplasmatic/datalogic`) | Fluent builder | `Engine::builder()->addOperator('foo', fn ($a) => '...')->build()` |

**Built-ins win** on every binding: registering a name that collides
with a built-in JSONLogic operator (`+`, `if`, `var`, …) has no effect
at evaluation time — the built-in dispatches first.

### Two npm packages, one engine

The JS-side surface is intentionally split into two packages that share
the Rust core:

- **`@goplasmatic/datalogic-node`** is the first-class Node target.
  napi-rs gives the binding direct access to V8 types and per-platform
  native code — the same Rust engine, just behind a thin FFI layer.
  Node services should pick this by default.
- **`@goplasmatic/datalogic-wasm`** is the WebAssembly build. Run it in
  browsers, Deno, Bun, Cloudflare Workers, or any other runtime where a
  single artifact across platforms beats per-platform native prebuilds.
  Node consumers who want one artifact shared with a browser frontend
  can still use it — but the native package is faster.

Both packages track the same version and ship from the same release
workflow; pick the one that matches the runtime, not the language.

## Shared C ABI (`bindings/c/`)

The C ABI is **not a publishable binding by itself** — it's the canonical
FFI boundary that lower-level language packages consume. Languages whose
Rust binding tools (pyo3, napi-rs, magnus, wasm-bindgen) provide a more
ergonomic surface skip the C ABI and target their runtime directly.
Languages without a mature Rust binding tool (Go, PHP, JVM via FFI)
consume the C ABI's cdylib + generated header.

| Binding route | Goes through `bindings/c/`? | Why |
|---|---|---|
| Python (pyo3) | No | pyo3 gives ergonomic dict/list marshalling — better than JSON-string FFI |
| WASM (wasm-bindgen) | No | The browser doesn't have a C ABI — wasm-bindgen is the only path |
| Node native (napi-rs) | No | napi-rs exposes V8 types directly; cheaper than JSON-roundtrip |
| Ruby (magnus) | No | magnus mirrors pyo3 — direct Ruby type marshalling |
| Go (cgo) | **Yes** | No first-class Rust↔Go binding tool |
| JVM (FFM) | **Yes** | Avoids hand-writing JNI per platform |
| .NET (P/Invoke / `LibraryImport`) | **Yes** | NativeAOT-ready source-gen P/Invoke over the cdylib |
| PHP (FFI) | **Yes** | PHP's FFI extension consumes any cdylib + curated header |

The C ABI's surface is JSON-in/JSON-out throughout — no struct
marshalling at the boundary. Languages that want native-type fast paths
either go around the C ABI (rows above) or add a thin native shim on top.

### Cross-platform binary distribution for C-ABI bindings

Languages that route through `bindings/c/` need the static / shared
library at the consumer's build (Go cgo) or runtime (PHP FFI, JVM FFM)
time. The release workflow handles this with a shared (os, arch)
matrix that compiles `bindings/c/` once on a native runner per
platform, plus a per-language packaging job that picks up the matrix
artifacts and ships them in that language's idiomatic distribution
channel.

| Lang | Distribution shape | Lib type | Path in artifact |
|---|---|---|---|
| Go | Git tag `bindings/go/vX.Y.Z` with binaries staged in source tree | `.a` static | `bindings/go/lib/<os>_<arch>/libdatalogic_c.a` |
| JVM | JAR with platform binaries at the classpath root | `.so` / `.dylib` / `.dll` | `<os-arch>/` (loaded via FFM `java.lang.foreign`) |
| .NET | NuGet package with platform binaries under `runtimes/<rid>/native/` | `.so` / `.dylib` / `.dll` | `runtimes/{linux,osx,win}-{x64,arm64}/native/` |
| PHP | Composer package with platform binaries under `lib/<os>-<arch>/` | `.so` / `.dylib` / `.dll` | `lib/<os>-<arch>/` (loaded via `FFI::cdef`) |

Two matrices in `.github/workflows/`:
- `release-build-go.yml` produces the `.a` staticlib per platform (Go only).
- `release-build-c-cdylib.yml` produces the `.so`/`.dylib`/`.dll` cdylib
  per platform; .NET, JVM, and PHP packaging jobs each consume those
  artifacts and re-stage them under their idiomatic on-disk layout.

Supported (os, arch) matrix:

| OS | Arch | Runner | Rust target |
|---|---|---|---|
| Linux | amd64 | `ubuntu-latest` | `x86_64-unknown-linux-gnu` |
| Linux | arm64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` |
| macOS | amd64 | `macos-14` (cross from arm64 host) | `x86_64-apple-darwin` |
| macOS | arm64 | `macos-14` (native) | `aarch64-apple-darwin` |
| Windows | amd64 | `windows-latest` | `x86_64-pc-windows-gnu` (mingw — for cgo compat; PHP/JVM may need `msvc` too) |
| Windows | arm64 | `windows-11-arm` | `aarch64-pc-windows-gnullvm` (llvm-mingw — installed in-job; no native mingw-w64 ARM64 port exists) |

### Go tag mechanics: synthetic release commits

The Go module is the one distribution channel where binaries must live
in the source tree itself. `bindings/go/lib/` and `bindings/go/include/`
are gitignored on `main`; only release tags carry the prebuilt
artifacts. On a `v*` tag push, the `publish-go` job in `release.yml`
collects the staticlib artifacts from the `release-build-go.yml` matrix,
stages them into `bindings/go/lib/<os>_<arch>/` plus the generated C
header into `bindings/go/include/`, records the result as a synthetic
commit, and pushes a `bindings/go/v<version>` tag pointing at that
commit. Only the tag is pushed: the synthetic commit is reachable
exclusively through it, and `main` stays binary-free.

## Open candidates

Bindings that haven't landed yet:

- **Ruby** — `bindings/ruby/` via `magnus` (PR registry: RubyGems)
- **Swift** — `swift-bridge` or UniFFI (also covers Kotlin natively)
- **Elixir** — `rustler` NIFs

The core engine is `Send + Sync` and exposes a clean compile-once /
evaluate-many surface (`Engine`, `Logic`, `Session`), so the same
binding shape transfers to any language with a Rust FFI story.

## Binding README template

Every binding README is a registry landing page first (npm, PyPI,
pkg.go.dev, Maven Central, NuGet, Packagist) and a repo document second.
New bindings follow this section order:

1. H1: the published package name, no links
2. Badge row (registry version, CI, license) plus the line
   `Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs) — one engine, every runtime.`
3. Three-sentence pitch ending with the conformance stat: every binding
   runs the same core and passes the same 1,565-case conformance
   battery (54 suites)
4. At most one version blockquote (v4 rename / "new in v5" steering)
5. Install
6. Quick start
7. Compile-once / evaluate-many
8. Sessions (hot-loop arena reuse)
9. API surface table
10. Custom operators
11. Engine configuration: the shared config table, byte-identical
    across bindings
12. Error handling
13. Threading table
14. Tracing
15. Performance: the canonical-bench block plus one boundary sentence
    naming this binding's FFI layer
16. Building from source (15 lines or fewer)
17. Learn more footer: repository README, Rust crate deep-dive, the
    binding's docs-site chapter, online playground, JSONLogic spec
18. License

Two invariants:

- **Absolute URLs only.** Registry pages render the README standalone,
  so relative links 404 there.
- **The `<!-- canonical-bench v5.1 -->` block is quoted verbatim** in
  every binding README (comment line plus one unwrapped paragraph), so
  drift is greppable: `grep -A1 -r "canonical-bench" bindings/` must
  return byte-identical paragraphs.
