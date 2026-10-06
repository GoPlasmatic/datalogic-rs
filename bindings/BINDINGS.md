# Adding a language binding

Each language binding lives as a sibling crate under `bindings/<lang>/` and
follows the same conventions, so you can add a new binding without
re-deriving the layout.

## Naming

Every binding's published artifact follows the **`datalogic-<lang>`** pattern.
`<lang>` is the short, established suffix for the target language: `rs` for
Rust, `py` for Python, `wasm` for WebAssembly, `rb` for Ruby, `go` for Go,
`java` / `kt` / `swift` for the JVM/mobile family.

| Language | Internal Cargo crate | Published artifact | Registry |
|---|---|---|---|
| Rust | `datalogic-rs` | `datalogic-rs` | crates.io |
| WebAssembly | `datalogic-wasm` | **`@goplasmatic/datalogic-wasm`** | npm |
| Node native | `datalogic-node` | `@goplasmatic/datalogic-node` (recommended for Node; the WASM `@goplasmatic/datalogic-wasm` ships alongside for browsers / Deno / Bun / Workers) | npm |
| Python | `datalogic-py` | `datalogic-py` (PyPI) → `import datalogic_py` | PyPI |
| C ABI | `datalogic-c` | shared `cdylib`/`staticlib` + header (consumed by Go/JVM/.NET/PHP in-tree, not separately published) | none |
| Go | `datalogic-go` | `github.com/GoPlasmatic/datalogic-rs/bindings/go/v5` (in-tree module; `/v5` major-version suffix required by Go modules) | Go modules |
| JVM | (no Cargo crate; Maven module) | `io.github.goplasmatic:datalogic` | Maven Central |
| .NET | (no Cargo crate; .NET project) | `Goplasmatic.Datalogic` | NuGet |
| PHP | (no Cargo crate; Composer package) | `goplasmatic/datalogic` | Packagist |
| _future_ Ruby | `datalogic-rb` | `datalogic-rb` | RubyGems |

For Python the PyPI distribution name is `datalogic-py` but the Python
**module** name is `datalogic_py`: Python doesn't allow hyphens in
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
| Cargo | `crate-type = ["cdylib", "rlib"]`: `cdylib` for the FFI artifact, `rlib` so Rust consumers can also link it. `bindings/c` additionally emits `staticlib`, because the Go cgo consumer links `libdatalogic_c.a` statically; the Go build makes it on its own with the `go-release` profile (no debuginfo, fat LTO) to keep the module tag small |
| Dep on core | `datalogic-rs = { path = "../../crates/datalogic-rs", version = "...", features = [...] }`: the path builds against the tree; `version` names the oldest core release the binding works with, so raise it when the binding starts calling newer core API |
| Core features | The binding turns on core's `all-operators` umbrella, which enables every operator family, plus the cross-cutting features it needs (`serde_json`, `trace`, `templating`, `budget`; WASM adds `wasm-clock`). A new operator family joins `all-operators` in `crates/datalogic-rs/Cargo.toml` and reaches every binding with no manifest edit |
| Tests | The binding's native layout and runner: `tests/` + pytest (Python), `__test__/` + `node --test` (Node), root-level `*_test.go` + `go test` (Go), `src/test/` + JUnit (JVM), xunit (.NET), PHPUnit (PHP), `wasm-pack test` (WASM) |
| CI | A reusable `.github/workflows/release-build-<lang>.yml` that builds, tests and packages the binding (the C-ABI hosts JVM, .NET and PHP share `release-build-c-cdylib.yml` for the native library). `release.yml` calls it as `build-<lang>` and then runs `publish-<lang>`, which `needs: publish-crate` so a binding never ships ahead of core. `ci.yml` also calls `release-build-go.yml`, with uploads off, for its cross-platform matrix |
| Release tags | `v*` (e.g. `v5.1.0`), a single unified trigger. One tag push runs validate + tests, publishes core, then fans out every binding in parallel. |
| Versioning | Bindings track the core version exactly (5.3.0 → 5.3.0). `validate` fails if any binding's `Cargo.toml` / `pyproject.toml` / `package.json` / `Datalogic.csproj` / `pom.xml` drifts from core (`composer.json` carries no version: Packagist resolves it from the tag, and the Go module version lives in the `bindings/go/vX.Y.Z` tag). |

## Why these conventions

- **Excluded from root workspace.** Bindings pull in language-specific
  build deps (pyo3, napi, wasm-bindgen, cbindgen, …) that bloat the default `cargo test
  --workspace --all-features` and would force contributors who only
  want to run Rust tests to install language toolchains (Python
  interpreter, Node.js, JDK). Excluding keeps the core's dev loop fast.

- **`cdylib` + `rlib`.** `cdylib` is the importable artifact every
  binding needs (`.so`/`.pyd`/`.dll`). `rlib` lets a downstream Rust
  crate (e.g. integration tests, the binding's own benchmarks) link it
  as a normal dep without duplicating source.

- **One umbrella for the operator families.** Every binding exposes
  every operator family, so each depends on core's `all-operators`
  feature rather than listing the families. Adding a family is one entry
  in `all-operators` (core's `operator_names_test` checks that the list
  names every family in the operator table); listing them per binding
  meant a multi-file edit, and a binding that missed one quietly lacked
  its operators. The cross-cutting features (`serde_json`, `trace`,
  `templating`, `budget`) stay explicit, since not every binding wants
  each.

- **One release run, one reusable workflow per binding.** A `v*` tag
  starts `release.yml`: validate + tests → publish core → fan out the
  bindings in parallel. Each binding's build lives in its own
  `release-build-<lang>.yml`, which `release.yml` calls and `ci.yml` can
  reuse; the publish jobs stay in `release.yml` next to the core
  publish they depend on. A single tag push produces one workflow run
  with one set of status checks. The trade-off is that a Python
  wheel-build failure shows up in the same run as core/wasm. The
  bindings are independent jobs, though, so a failure in one doesn't
  roll back the others.

## Existing bindings

| Binding | Path | Tech | Publishes to |
|---|---|---|---|
| WebAssembly | `bindings/wasm/` | wasm-bindgen + wasm-pack | npm: `@goplasmatic/datalogic-wasm` |
| Node native | `bindings/node/` | napi-rs + napi-cli (per-platform `.node` prebuilds with `optionalDependencies`) | npm: `@goplasmatic/datalogic-node` |
| Python | `bindings/python/` | pyo3 + maturin (abi3-py310) | PyPI: `datalogic-py` |
| C ABI | `bindings/c/` | `extern "C"` + cbindgen-generated header | (not separately published; consumed in-tree by Go/JVM/.NET/PHP) |
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
at evaluation time: the built-in dispatches first.

### Two npm packages, one engine

The JS side ships as two packages that share the Rust core:

- **`@goplasmatic/datalogic-node`** is the native Node target.
  napi-rs gives the binding direct access to V8 types and per-platform
  native code: the same Rust engine behind a thin FFI layer.
  Node services should pick this by default.
- **`@goplasmatic/datalogic-wasm`** is the WebAssembly build. Run it in
  browsers, Deno, Bun, Cloudflare Workers, or any other runtime where a
  single artifact across platforms beats per-platform native prebuilds.
  Node consumers who want one artifact shared with a browser frontend
  can still use it, but the native package is faster.

Both packages track the same version and ship from the same release
workflow; pick the one that matches the runtime, not the language.

## Shared C ABI (`bindings/c/`)

The C ABI is **not a publishable binding by itself**: it is the canonical
FFI boundary that lower-level language packages consume. Languages whose
Rust binding tools (pyo3, napi-rs, magnus, wasm-bindgen) provide a more
ergonomic surface skip the C ABI and target their runtime directly.
Languages without a mature Rust binding tool (Go, PHP, JVM via FFI)
consume the C ABI's cdylib + generated header.

| Binding route | Goes through `bindings/c/`? | Why |
|---|---|---|
| Python (pyo3) | No | pyo3 gives ergonomic dict/list marshalling; better than JSON-string FFI |
| WASM (wasm-bindgen) | No | The browser doesn't have a C ABI; wasm-bindgen is the only path |
| Node native (napi-rs) | No | napi-rs exposes V8 types directly; cheaper than JSON-roundtrip |
| Ruby (magnus) | No | magnus mirrors pyo3: direct Ruby type marshalling |
| Go (cgo) | **Yes** | No mature Rust↔Go binding tool |
| JVM (FFM) | **Yes** | Avoids hand-writing JNI per platform |
| .NET (P/Invoke / `LibraryImport`) | **Yes** | NativeAOT-ready source-gen P/Invoke over the cdylib |
| PHP (FFI) | **Yes** | PHP's FFI extension consumes any cdylib + curated header |

The C ABI's surface is JSON-in/JSON-out throughout, with no struct
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

Supported (os, arch) matrix. Both workflows share the same runners and
the same Linux/macOS targets; they differ on Windows, where the Go
staticlib must match cgo's mingw ABI while the cdylib consumers
(.NET, JVM, PHP) want the idiomatic MSVC ABI:

| OS | Arch | Runner | Go staticlib target (`release-build-go.yml`) | cdylib target (`release-build-c-cdylib.yml`) |
|---|---|---|---|---|
| Linux | amd64 | `ubuntu-latest` | `x86_64-unknown-linux-gnu` | same |
| Linux | arm64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | same |
| macOS | amd64 | `macos-15` (cross from arm64 host) | `x86_64-apple-darwin` | same |
| macOS | arm64 | `macos-15` (native) | `aarch64-apple-darwin` | same |
| Windows | amd64 | `windows-latest` | `x86_64-pc-windows-gnu` (mingw-w64) | `x86_64-pc-windows-msvc` |
| Windows | arm64 | `windows-11-arm` | `aarch64-pc-windows-gnullvm` (llvm-mingw, installed in-job; no native mingw-w64 ARM64 port exists) | `aarch64-pc-windows-msvc` |

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

## Shared wire formats (`crates/datalogic-bind`)

The four Rust-level bindings (`wasm`, `node`, `python`, `c`) depend on
`crates/datalogic-bind`, which defines every JSON document a binding hands
to its host: the traced-run envelope, the operator catalogue (the schema
of `docs/src/operators/operators.json`), rule facts and check diagnostics,
plus the custom-operator argument/result bridge, the typed-result type
names and the JS budget validation. A format changes in one place and
every host sees the same bytes; the Go, JVM, .NET and PHP wrappers reach
the same formats through the C ABI. The crate is not published: it is
built from the tree like core.

## API names across bindings

One concept, one name, cased for the language. Where a language's own
type names would make the shared name misleading, the binding keeps the
idiomatic spelling (a Java or C# `Int` is 32-bit, so those bindings say
`Long` / `Int64`).

| Concept | Rust | Python | Node / WASM | Go | JVM | .NET | PHP | C ABI |
|---|---|---|---|---|---|---|---|---|
| Compile | `compile` | `compile` | `compile` | `Compile` | `compile` | `Compile` | `compile` | `datalogic_engine_compile` |
| Template / strict compile | `compile_template` / `compile_strict` | same | `compileTemplate` / `compileStrict` | `CompileTemplate` / `CompileStrict` | `compileTemplate` / `compileStrict` | `CompileTemplate` / `CompileStrict` | `compileTemplate` / `compileStrict` | `datalogic_engine_compile_mode` |
| Checked compile | `compile_checked` | `compile_checked` | `compileChecked` | `CompileChecked` | `compileChecked` | `CompileChecked` | `compileChecked` | `datalogic_engine_compile_checked` |
| Diagnostics | `check` | `check` | `check` | `Check` | `check` | `Check` | `check` | `datalogic_engine_check` |
| Operator catalogue | `operators` | `operators` | `operators` | `Operators` | `operators` | `Operators` | `operators` | `datalogic_engine_operators` |
| Rule facts | `Logic::facts` | `Rule.facts` | `Rule.facts` | `Rule.Facts` | `Rule.facts` | `Rule.Facts` | `Rule::facts` | `datalogic_rule_facts` |
| Truthiness | `truthy_of` | `truthy` | `truthy` | `Truthy` | `truthy` | `Truthy` | `truthy` | `datalogic_engine_truthy` |
| Typed results | n/a | `evaluate_bool` / `_int` / `_float` / `_truthy` | `evaluateBool` / `Int` / `Float` / `Truthy` | `EvaluateBool` / `Int64` / `Float64` / `Truthy` | `evaluateBool` / `Long` / `Double` / `Truthy` | `EvaluateBool` / `Int64` / `Double` / `Truthy` | `evaluateBool` / `Int` / `Float` / `Truthy` | `datalogic_session_evaluate_bool` / `_i64` / `_f64` / `_truthy` |
| Trace in a mode | `TracedSession::with_mode` | `evaluate_with_trace(.., mode)` | `evaluateWithTrace(.., mode)` | `TracedSession.EvaluateMode` | `TracedSession.evaluate(.., mode)` | `TracedSession.Evaluate(.., mode)` | `TracedSession::evaluate(.., $mode)` | `datalogic_traced_session_evaluate_mode` |
| Metered | `evaluate_metered` | `evaluate_metered` | `evaluateMetered` | `EvaluateMetered` | `evaluateMetered` | `EvaluateMetered` | `evaluateMetered` | `datalogic_session_evaluate_metered` |
| Refuse a built-in name | `try_add_operator` | `strict_operator_names=True` | `strictOperatorNames` | `StrictOperatorNames` | `withStrictOperatorNames` | `WithStrictOperatorNames` | `withStrictOperatorNames` | `datalogic_engine_builder_set_strict_operator_names` |
| Operator families | `with_families` | `families=[...]` | `families` | `Families` | `withFamilies` | `WithFamilies` | `withFamilies` | `datalogic_engine_builder_set_families` |
| Error type | `Error::code` | `.error_type` | `.errorType` | `.Type` | `errorType()` | `.ErrorType` | `->errorType` | `datalogic_error_tag` |

`truthy` reads a string as JSON text in every binding, as data is read:
`truthy("[]")` asks about an empty array. A metered call's budget is at
least 1 where the host can leave it out (Python `None`, JavaScript
`undefined`); the C-ABI bindings, which cannot, spell the engine's own
budget `0`. No binding reads `0` as a budget nothing fits in.

Deprecated in 5.8 and removed in 6.0: the WASM `CompiledRule` class and
the free `evaluate(logic, data, templating)` / `evaluateWithTrace(...,
templating)` functions (use an `Engine`), and `evaluateNumber` in Node and
WASM (use `evaluateFloat`).

## Scenarios (`bindings/scenarios/api.json`)

One file of API-level cases that every binding runs through its own
public API: compile modes, checked compiles, diagnostics, truthiness,
facts, metering and engine options, each with its expected result or
error type. The runners are `python/tests/test_scenarios.py`,
`node/__test__/scenarios.test.mjs`, `wasm/tests/scenarios.rs`,
`c/tests/scenarios.rs`, `go/scenarios_test.go`,
`jvm/.../ScenariosTest.java`, `dotnet/.../ScenarioTests.cs` and
`php/tests/ScenariosTest.php`. A new API item is not done until it has a
scenario and all eight runners pass it.

`bindings/c/tests/header_sync.rs` keeps the hand-written declarations in
step with the generated `include/datalogic.h`: PHP's FFI header (names and
parameter counts), the JVM's downcall handles and .NET's imports.

## Open candidates

Bindings that haven't landed yet:

- **Ruby**: `bindings/ruby/` via `magnus` (PR registry: RubyGems)
- **Swift**: `swift-bridge` or UniFFI (also covers Kotlin natively)
- **Elixir**: `rustler` NIFs

The core engine is `Send + Sync` and exposes a compile-once /
evaluate-many surface (`Engine`, `Logic`, `Session`), so the same
binding shape transfers to any language with a Rust FFI story.

## Binding README template

Every binding README is a registry landing page first (npm, PyPI,
pkg.go.dev, Maven Central, NuGet, Packagist) and a repo document second.
New bindings follow this section order:

1. H1: the published package name, no links
2. Badge row (registry version, CI, license) plus the line
   `Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs): one engine, every runtime.`
3. Three-sentence pitch ending with the conformance stat: every binding
   runs the same core and passes the same 1,974-case conformance
   battery (65 suites)
4. At most one version blockquote (v4 rename / "new in v5" steering)
5. Install
6. Quick start
7. Compile-once / evaluate-many
8. Sessions (hot-loop arena reuse), then the ABI v2 tiers: data
   handles (parse once), typed results, batch evaluation
9. API surface table (one row per tier, including Data handle, Typed,
   Batch and Traced)
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
