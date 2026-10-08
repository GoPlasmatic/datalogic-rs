# Development

A walkthrough of working on each package in this monorepo. For the big
picture (what depends on what, why the layout is shaped this way), see
[ARCHITECTURE.md](./ARCHITECTURE.md).

## Prerequisites

| Tool        | Version | Why                                                            |
|-------------|---------|----------------------------------------------------------------|
| Rust        | 1.98+   | `rust-version` in `crates/datalogic-rs/Cargo.toml` (inherited from `datavalue-rs` 0.3) |
| `wasm-pack` | latest  | Builds `bindings/wasm` (only for WASM/UI changes)              |
| Node.js     | 20+     | Builds and runs `ui`, `bindings/wasm`, and `bindings/node`     |
| Python      | 3.10+   | Builds `bindings/python` via `maturin`                         |
| Go          | 1.25+   | Builds `bindings/go` (`go.mod` declares `go 1.25`; also needs a C compiler for cgo) |
| Java JDK    | 22+     | FFM (`java.lang.foreign`) downcalls; `--enable-native-access=ALL-UNNAMED` on 24+ |
| Maven       | 3.8+    | Builds `bindings/jvm` (only for JVM changes)                   |
| .NET SDK    | 8.0+    | Builds and tests `bindings/dotnet` (only for .NET changes); SDK 10 also builds the `net10.0` target |
| PHP         | 8.4+    | Runs PHP tests (requires `ext-ffi` enabled in `php.ini`)       |
| Composer    | 2.0+    | Manages dependencies for `bindings/php` (only for PHP changes)  |
| `mdbook`    | latest  | Builds the docs site under `docs/`                             |

```bash
rustup update stable
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
# Install Node, Go, Java, .NET, and PHP via your package manager of choice
pip install maturin    # only if you are editing bindings/python/
cargo install mdbook   # only if you are editing docs/
```

## Repo-wide commands

The repo holds six Cargo workspaces. The root one has three members
(`crates/datalogic-rs`, `crates/datalogic-bind`, `tools/benchmark`). The
four bindings and the fuzz crate are `exclude`d from it and each declares
its own `[workspace]` table (the `exclude` comment in the root
`Cargo.toml` explains why per crate), so root-level `cargo fmt --all`,
`cargo clippy --workspace` and `cargo clean` **skip them without a
warning**.

The root `Makefile` fans those commands out over every manifest:

```bash
make lint        # fmt-check + clippy, all six manifests; run before a PR
make fmt         # format everything
make fmt-check   # check formatting without writing (what CI gates on)
make clippy      # clippy everything, every crate's failures in one pass
make test        # every Rust test suite: root workspace, C ABI, WASM
make doc         # core rustdoc with warnings denied (all + default features)
make deny        # cargo-deny every shipping workspace against deny.toml
make stats       # quoted stats + the tests' required-features (CI gates on both)
make stats-write # rewrite every conformance-count quote to the current count
make semver      # cargo-semver-checks the core crate against its last release
make clean       # cargo clean every manifest (~3 GB in a warm tree)
make clean-all   # clean + node_modules, venv, vendor, pkg/, dotnet bin+obj, ...
make help        # list all targets
```

`make deny` and `make semver` need `cargo install cargo-deny
cargo-semver-checks --locked`. Some targets need more than the stable
host toolchain:

- **`bindings/wasm`**: `rustup target add wasm32-unknown-unknown`. Its
  test files open with `#![cfg(target_arch = "wasm32")]`, so a host-target
  lint compiles them to empty files and checks nothing. Without the target,
  `make clippy` warns and falls back to a host lint (in CI, where `CI` is
  set, it fails instead so lost coverage can't hide). `make test` runs the
  tests under `wasm-pack test --node`, and skips them without `wasm-pack`
  (again, except in CI).
- **`crates/datalogic-rs/fuzz`**: lints on stable (its build script
  compiles libFuzzer's C++, so it needs a C++ compiler); only
  `cargo fuzz run` needs nightly.

Individual crates are reachable as `make clippy-c`, `make clippy-wasm`, etc.

## The build pipeline

The packages have a strict build order. From a fresh clone:

```bash
# 1. Rust workspace: runs core unit/integration tests and the bench crate's checks.
# Most integration tests are gated behind feature = "serde_json"; the JSONLogic
# runner also needs feature = "templating". --all-features unlocks both.
cargo test --workspace --all-features

# 2. WASM bindings: produces bindings/wasm/pkg/{web,bundler,nodejs}.
cd bindings/wasm && ./build.sh && cd ../..

# 3. Node native binding: produces bindings/node/datalogic-node.<triple>.node
#    plus the index.js/index.d.ts loaders. Skip if you're only touching the
#    WASM or browser side.
cd bindings/node && npm install && npx napi build --platform --release && cd ../..

# 4. UI: its predev / prebuild hooks vendor the WASM built in step 2.
cd ui && npm install
npm run dev   # or: npm run build:lib for the publishable bundle
```

The UI does not resolve `@goplasmatic/datalogic-wasm` from the registry:
its Vite and TypeScript configs alias the package to `ui/vendor/datalogic`,
and the `predev` / `prebuild*` lifecycle hooks copy `../bindings/wasm/pkg`
there (`npm run sync-wasm`). Rebuild WASM first, then start the UI, and you
test the fresh build; see [`ui` below](#ui-react-component).

## `crates/datalogic-rs`: Rust library

```bash
cargo check -p datalogic-rs
cargo test  -p datalogic-rs --all-features         # everything; the run CI gates on
make lint      # fmt + clippy, every manifest (what CI gates on)
```

Always pass `--all-features` (or the features a test needs). Most
integration tests need `serde_json`, and several need more; each declares
its features as `[[test]] required-features` in
`crates/datalogic-rs/Cargo.toml`, so a plain `cargo test -p datalogic-rs`
skips them and runs little beyond the unit tests. Naming one without its
features (`cargo test -p datalogic-rs --test basic_test`) fails with the
features to pass. `scripts/check-test-features.sh` keeps those entries in
step with each file's `#![cfg(...)]` header.

Run a single JSONLogic suite (the `test_jsonlogic` harness picks the file
from an env var). The path is relative to `crates/datalogic-rs/` because that's
the test binary's cwd; the harness needs both `serde_json` and `templating`
(both included in `--all-features`):

```bash
JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
  cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
```

Run a feature-gated example:

```bash
cargo run -p datalogic-rs --example getting_started   --features templating
cargo run -p datalogic-rs --example structured_objects --features templating
cargo run -p datalogic-rs --example tracing           --features trace
cargo run -p datalogic-rs --example datetime_ops      --features datetime
cargo run -p datalogic-rs --example error_handling    --features error-handling
cargo run -p datalogic-rs --example zero_copy_input   --features serde_json
```

See [crates/datalogic-rs/examples/README.md](./crates/datalogic-rs/examples/README.md)
for the full table.

### Fuzzing (optional, nightly only)

`crates/datalogic-rs/fuzz/` holds a cargo-fuzz target that feeds arbitrary
(rule, data) strings into `Engine::eval_str` (plain and templating engines);
errors are expected, panics/aborts are findings. It complements the bounded
proptest generator in `tests/property_test.rs` with coverage-guided byte
mutation. The fuzz crate is excluded from the workspace and needs a nightly
toolchain plus `cargo install cargo-fuzz`:

```bash
cd crates/datalogic-rs
cargo +nightly fuzz run eval_str                    # until interrupted
cargo +nightly fuzz run eval_str -- -max_total_time=300   # bounded run
```

Crashing inputs land in `fuzz/artifacts/`; minimize with
`cargo +nightly fuzz tmin eval_str <artifact>` and turn the minimized case
into a regression test before fixing.

The `fuzz` workflow (`.github/workflows/fuzz.yml`) runs the target nightly
for a bounded time, seeded with every rule and data value in the
conformance suites, and uploads any crash as an artifact. The fuzz crate
depends on `all-operators`, so a new operator family is fuzzed without an
edit there.

## `crates/datalogic-bind`: what the bindings share

```bash
cargo test -p datalogic-bind      # also part of `cargo test --workspace --all-features`
```

Not published; the WASM, Node, Python and C bindings build it from the
tree. It defines the JSON documents every binding emits (the traced-run
envelope, the operator catalogue, rule facts, check diagnostics, batch
item errors) and the custom-operator bridge. Its tests validate real
output against `schemas/*.v1.json` and reject a property a schema does
not list, and check the Node TypeScript return types and the Python stub
(`bindings/python/datalogic_py.pyi`) against the same schemas. A format
change is a schema change; see [schemas/README.md](./schemas/README.md).

Every binding also runs `bindings/scenarios/api.json` through its own
API: one list of scenarios (templating modes, families, `check`, facts,
budgets, traced pointers, ...), each with one expectation all eight
bindings must meet. Add a scenario there when you add binding surface;
the "Scenarios" section of [bindings/BINDINGS.md](./bindings/BINDINGS.md)
has the conventions.

## `bindings/wasm`: WebAssembly bindings (browser / Deno / Bun / Workers)

```bash
cd bindings/wasm
./build.sh               # builds web, bundler, and nodejs targets
```

The crate is its own Cargo workspace (see ARCHITECTURE.md for why), so
`cargo` commands inside `bindings/wasm/` operate on it standalone. Its
tests (`tests/web.rs`, `scenarios.rs`, `conformance.rs`,
`invalid_arguments.rs`) compile only for wasm32: run them with
`wasm-pack test --node` from that directory, or `make test-wasm`.
End-user API and install instructions: [bindings/wasm/README.md](./bindings/wasm/README.md).

The WASM build ships a `nodejs` target too, which suits a consumer
that wants one artifact across Node + browser. **For production Node
workloads, prefer the native binding below**; it's faster.

## `bindings/node`: Node native binding (napi-rs)

```bash
cd bindings/node
npm install                                   # one-time; pulls @napi-rs/cli
npx napi build --platform --release           # emits datalogic-node.<triple>.node + index.js + index.d.ts
npm test                                      # node --test __test__/*.test.mjs
```

This is the **primary Node target**, published as
`@goplasmatic/datalogic-node` with per-platform `.node` prebuilds
distributed as npm `optionalDependencies`. `napi build` generates the
`.node` artifact, `index.js`, and `index.d.ts`, all gitignored; rerun
the build after any Rust-side change.

The crate is its own Cargo workspace (matches the wasm/python/c
pattern), so `cargo` commands inside `bindings/node/` don't touch the
root workspace. End-user API and install instructions:
[bindings/node/README.md](./bindings/node/README.md).

## `bindings/python`: Python bindings (pyo3)

```bash
cd bindings/python
maturin develop --release         # build + install into the current venv
pytest                            # run the Python test suite
maturin build --release           # produce a wheel under target/wheels/
```

Like `bindings/wasm`, this crate is its own Cargo workspace (keeps the
pyo3 build deps out of the core `cargo test --workspace` path). End-user
API and install instructions: [bindings/python/README.md](./bindings/python/README.md).

## `bindings/c`: shared C ABI (cbindgen)

```bash
cd bindings/c
cargo build --release             # produces libdatalogic_c.{so,dylib,a}
cargo test                        # smoke, scenarios, conformance, header sync
```

cbindgen regenerates the C header `include/datalogic.h` on every
build. Don't edit it by hand; edit `src/` and rebuild. Consumers can set
`DATALOGIC_C_SKIP_CBINDGEN=1` to suppress regeneration. The
`header_sync` test fails when PHP's FFI header (`bindings/php/src/datalogic-ffi.h`)
or the JVM and .NET native declarations drift from the generated header.

This crate is **not user-facing**; it's the FFI boundary the Go, JVM,
.NET, and PHP bindings consume. See
[bindings/c/README.md](./bindings/c/README.md) for the API surface and
memory / threading rules.

## `bindings/go`: Go binding (cgo over C ABI)

```bash
cd bindings/go
make build                        # cargo-builds bindings/c, stages lib/<host>/
make test                         # runs `go test -v ./...`
make print-platform               # prints the host's lib/ subdirectory name
```

The Makefile auto-detects host OS/arch and stages into
`lib/<host_os>_<host_arch>/`; only the matching `cgo_*_*.go` file
needs that subdirectory populated locally. Re-run `make build` after
any change to the C ABI's Rust source. End-user API, install
instructions, and prebuilt-library platform matrix:
[bindings/go/README.md](./bindings/go/README.md).

## `bindings/dotnet`: .NET binding (P/Invoke over C ABI)

```bash
cd ../c && cargo build --release   # produce libdatalogic_c.{so,dylib,dll}
cd ../dotnet
dotnet build -c Release
dotnet test
```

P/Invoke stubs are hand-written with `LibraryImport` (source-generated,
NativeAOT-ready). The package targets `net8.0`, plus `net10.0` when the
SDK is 10 or newer. A `DllImportResolver` resolves the native library at
runtime, falling through:
`DATALOGIC_NATIVE_LIB` env → NuGet's `runtimes/<rid>/native/` →
`bindings/c/target/release/`. Publish target: NuGet `Goplasmatic.Datalogic`.
End-user API: [bindings/dotnet/README.md](./bindings/dotnet/README.md).

## `bindings/jvm`: JVM binding (FFM over C ABI)

```bash
cd ../c && cargo build --release
cd ../jvm
mvn test                           # JUnit 5
mvn package                        # produces target/datalogic-*.jar + sources + javadoc
```

The binding calls the C ABI through the `java.lang.foreign`
(FFM) API (JDK 22+); `internal/DatalogicNative` mirrors
`bindings/c/include/datalogic.h` as `MethodHandle` downcalls. The
Surefire plugin sets the `datalogic.library.path` system property to
`../c/target/release` (and passes `--enable-native-access=ALL-UNNAMED`)
so local tests pick up the in-tree cdylib. Publishable JARs ship the
native libs at the classpath root under `<os-arch>/`; the loader extracts
the matching one once into a per-user cache named by its SHA-256
(`~/.cache/datalogic/native/` or the platform equivalent, falling back to
a temp directory) and links it. Target: Maven
Central as `io.github.goplasmatic:datalogic`. End-user API:
[bindings/jvm/README.md](./bindings/jvm/README.md).

## `bindings/php`: PHP binding (PHP FFI over C ABI)

```bash
cd ../c && cargo build --release
cd ../php
composer install                    # one-time
vendor/bin/phpunit                  # PHPUnit
```

Loads `libdatalogic_c.{so,dylib,dll}` at runtime via
`FFI::cdef(<curated header>, <lib path>)`, with the curated header in
`src/datalogic-ffi.h`. The Native loader searches
`DATALOGIC_NATIVE_LIB` → `bindings/php/lib/<os>-<arch>/` → in-tree
`bindings/c/target/release/` → the OS library path. PHP 8.4+ with `ext-ffi` required (tracks
`composer.json`; PHPUnit 13 in the test suite requires PHP 8.4+). Publish
target: Packagist `goplasmatic/datalogic`. End-user API:
[bindings/php/README.md](./bindings/php/README.md).

## `ui`: React component

```bash
cd ui
npm install
npm run dev              # local playground, hot reload (auto-syncs WASM)
npm test                 # vitest: registry, round trips, trace, samples
npm run build            # standalone playground (dist/)
npm run build:lib        # publishable component (dist/)
npm run build:embed      # embeddable widget for the docs site (dist-embed/)
npm run lint
npm run sync-wasm        # manually re-copy ../bindings/wasm/pkg/ → vendor/datalogic/
```

`npm test` runs against the vendored engine rather than fixtures, so it fails
when the UI's picture of the engine drifts:

| Suite | Location | Guards |
|-------|----------|--------|
| Registry and help | `src/components/logic-editor/config/__tests__/` | Registry matches `builtinOperatorNames()`; every picker entry (aliases included) has an argument count the catalogue in `docs/src/operators/operators.json` allows; every help example evaluates to its documented result |
| Round trips | `src/components/logic-editor/utils/__tests__/` | A corpus covering every operator plus the shipped samples survives `jsonLogicToNodes` → `nodesToJsonLogic` and evaluates identically; edge ids stay unique |
| Editing | `src/components/logic-editor/services/__tests__/` | Argument add/remove, deletion reindexing, duplicate/paste/wrap, inline edits |
| Trace | `src/components/logic-editor/utils/trace/__tests__/` | Real `evaluateWithTrace` envelopes map onto the diagram through the engine's node pointers, with no synthetic nodes |
| App surface | `ui/tests/` | Samples evaluate to their stored results, share URLs round trip, every operator is reachable from the menus, evaluator/config helpers, the CommonJS build starts the engine (after `npm run build:lib`) |
| Editor (jsdom) | `ui/tests/dom/` | The rendered editor: modes, editing round trips, undo history, keyboard shortcuts, properties panel |

Adding an operator, a help example, or a sample means giving it the result the
engine produces: the suites compare against a live evaluation.

Three Vite configs power the three build modes:

- `vite.config.ts`: playground SPA
- `vite.lib.config.ts`: `@goplasmatic/datalogic-ui` library bundle
- `vite.embed.config.ts`: embeddable widget for docs

The WASM dep is vendored under `ui/vendor/datalogic/` (gitignored),
synced from `bindings/wasm/pkg/` by `sync-wasm`. The `predev` and `prebuild*`
hooks run it, so the typical loop is:

```bash
cd bindings/wasm && ./build.sh    # rebuild after Rust changes
cd ../../ui && npm run dev        # predev re-vendors the fresh pkg/
```

`ui/package.json` does not depend on `@goplasmatic/datalogic-wasm`. Every
import of it resolves to `vendor/datalogic`: through `vite.aliases.ts` for
the three Vite configs and `vitest.config.ts`, and through `paths` in
`tsconfig.app.json` and `tsconfig.lib.json`. `sync-wasm` refuses a `pkg/`
whose version differs from the UI's (set
`DATALOGIC_ALLOW_WASM_VERSION_MISMATCH=1` to accept one). After building
the library, `release-build-ui.yml` adds the package as a devDependency
pinned to the version being published, for provenance. The library bundle
embeds the WASM engine, so consumers install neither.

## Releases

All publishing flows through `.github/workflows/release.yml`, triggered by
pushing a `v*` tag whose version matches `crates/datalogic-rs/Cargo.toml`.
There are no local publish scripts; do not run `npm publish` or
`cargo publish` by hand.

### Cutting a release

1. `scripts/bump-version.sh <x.y.z>` updates every versioned file the
   validate job checks, then refreshes the five `Cargo.lock` files (CI and
   the release build with `--locked`).
2. `scripts/check-stats.sh --write` updates every quoted conformance count.
   The release runs the script with `--strict`, where a stale count fails.
3. Give the root `CHANGELOG.md` a dated `## [x.y.z] - YYYY-MM-DD` section.
   The crates.io `crates/datalogic-rs/CHANGELOG.md` only links to it; leave
   it alone.
4. Merge, wait for CI on `main`, then tag that commit and push the tag.

### What the workflow does

1. **`validate`**: the tag is a tag and matches every package version; the
   CHANGELOG section exists and is dated; `check-stats.sh --strict`; and
   `cargo semver-checks` against the last crates.io release, version-aware,
   so a patch tag that adds API fails.
2. **`ci`, and every `build-*` job, in parallel**: `ci` is the whole of
   `ci.yml` (lint, tests, feature matrix, MSRV, minimal versions, docs,
   examples, cargo-deny, every binding's tests) run against the tag. Each
   `build-*` job is a `release-build-*.yml` reusable workflow that builds
   one binding's publishable artifact with the pinned release toolchain
   and the committed lockfile: the WASM package and the UI bundle, Python
   wheels and sdist, Node prebuilds plus the generated `index.js` /
   `index.d.ts` loader (the `node-js-loader` artifact), the C cdylib matrix, the Go
   staticlib matrix, and from the cdylibs the NuGet package, the JAR and
   the PHP dist (staged by `scripts/stage-natives.sh`, which fails if a
   platform is missing). `smoke-hosts` then installs the JAR, .nupkg and
   PHP dist on macOS and Windows and runs an evaluation and a custom
   operator through each; it reports but is not a gate.
3. **`publish-crate`**: runs only when `validate`, `ci` and every build
   passed. Once it succeeds the version is on crates.io for good.
4. **Publish phase**, each job downloading its artifact and pushing it
   (no rebuild): npm (WASM, Node), PyPI, NuGet, Maven Central, Packagist
   (through the `GoPlasmatic/datalogic-php` split), and the
   `bindings/go/vX.Y.Z` module tag. `publish-node` refuses to publish the
   umbrella `@goplasmatic/datalogic-node` package unless `npm pack` lists
   its `index.js` and `index.d.ts`. `publish-ui` waits for `publish-wasm`.
5. **`github-release`**: runs alongside `publish-crate`, gated on `ci` and
   the builds only, so a registry outage cannot leave the release page
   without its assets. It takes the notes from the version's CHANGELOG
   section, opens an Announcements discussion, and attaches the WASM and
   UI npm tarballs, Python wheels and sdist, Node prebuilds, Go staticlib
   and C cdylib tarballs (each with `datalogic.h`), the .nupkg, the JARs
   and the PHP zip.

A failed registry leg is re-run with `gh workflow run release.yml --ref
vX.Y.Z`; every publish step skips a version the registry already has.
Registry status, open release-ops items and one-time maintainer chores live
in [.github/LAUNCH-PLAYBOOK.md](./.github/LAUNCH-PLAYBOOK.md#registry-and-release-ops).

## `tools/benchmark`: performance harness

Dev-only, never published. Five binaries share `src/lib.rs`: `self`
(regression baseline), `compare` (cross-library matrix), `boundary_core`
(the rust-core runner for the per-binding boundary benchmark under
`tools/benchmark/boundary/`), `profile_macro` (hammers one macro
suite in a hot loop as a feeder for samply / Instruments), and
`projection` (evaluation over a large context, with and without read
projection). Suites are read through the core tests' shared loader
(`crates/datalogic-rs/tests/common/suite.rs`), so each case compiles on
the engine its `templating` / `template_key_escape` fields ask for, and
a suite that fails to parse stops the run:

```bash
# datalogic-rs alone, fast arena path
cargo run --release -p datalogic-bench --bin self
cargo run --release -p datalogic-bench --bin self -- --all   # every suite + JSON report

# Cross-library comparison (only datalogic-rs ships by default)
cargo run --release -p datalogic-bench --bin compare -- --all

# Per-binding boundary cost, rust-core column (other runtimes: boundary/run.sh).
# The workloads dir defaults to tools/benchmark/boundary/workloads/.
cargo run --release -p datalogic-bench --bin boundary_core

# Profiler feeder: <suite-substring> [seconds], one macro suite in a hot loop
cargo run --release -p datalogic-bench --bin profile_macro -- checkout 10

# Read projection over a large context
cargo run --release -p datalogic-bench --bin projection
```

Reports land in `tools/benchmark/output/` (gitignored), labelled with
the `datalogic-rs` version they measured (`report-self-v5.8.0-*.json`). To add another
JSONLogic implementation as a comparison subject, see
[tools/benchmark/README.md](./tools/benchmark/README.md).

## Adding a built-in operator

Every built-in operator is one row of the operator table in
`crates/datalogic-rs/src/operators/table.rs` plus one plain function the
row points at. The `OpCode` enum, name lookup, `as_str`, dispatch arms,
`Engine::builtin_operator_names()`, `Engine::operators()` and every
optimizer classification are generated from the row or derived from what
it declares. The module doc at the top of `table.rs` carries the row
grammar.

1. **Row.** Add a row inside the right `family` block. The family's `cfg`
   gate and its default metadata preset (`= STRING` here) apply to every
   row in it.

   ```rust
   family ExtString (feature = "ext-string") = STRING {
       Repeat ["repeat"] => eager(Str, Lenient<Int>) string::repeat;
       Length ["length"] => eager(Any) array::length { cost: Cost::Bytes, on_extra: Extra::InvalidArgs };
   }
   ```

   A row without braces takes the family's preset as is. Braces override
   fields, and a trailing `..BASE` starts from another preset instead:
   `{ on_empty_source: Some(singleton_empty_array), ..ITERATOR }`.

   The shape is `eager(Extractor, ...)` for an operator whose arguments
   can be evaluated in order (coerced for you; a variadic tail is
   `Rest<T>`, which evaluates each argument when the body asks, so
   `{"??": [...]}` still stops at the first non-null), `raw` for one that
   inspects its argument nodes or evaluates only some of them (`if`,
   `val`, `throw`), `each` for an
   iterator over `args[0]`, or `iter` for an iterator that resolves its
   source itself (it receives the cached `IterArgKind`). An `each` row
   declares `on_empty_source`, the result for a null, missing or empty
   array source; the generated arm returns it without calling the body,
   and the body receives the rest as `Items` (a non-empty array, an
   object, or a scalar) followed by `args`. Use `iter` only when that
   does not fit (`sort` answers null and `[]` differently, `reduce`
   evaluates its initial value first, `min` / `max` are variadic).
   Names are canonical first, then aliases; `[]` makes an internal opcode
   (give it a `display` name).

   A `raw`, `iter` or `each` row declares how many arguments it reads in
   brackets, `raw[2]`, `raw[2..]` or `iter[2..=3]`, and the generated arm
   applies the row's `on_missing` / `on_extra` before the body runs, so
   the body never checks the count itself (it may still branch on it).
   Leave the brackets off only when every count is meaningful (`+`, `cat`).

   Operators that share an implementation name their operation once with
   `@ Kind(payload)`: `raw[2..] comparison::ordered @ Ord(OrdOp::Gt)`.
   The row's `algebra` becomes `Algebra::Ord(OrdOp::Gt)` and the body
   receives `OrdOp::Gt` as its last argument, so the fast paths and the
   body read the same constant. A one-off bound argument goes in the path
   instead: `eager(StrictNum, Rest<StrictNum>) arithmetic::unary_math(UnaryMathOp::Abs)`.
2. **Body.** Write the function the row names, under
   `crates/datalogic-rs/src/operators/<category>/`. An `eager` body takes
   the extracted values and returns any `IntoValue` type:

   ```rust
   pub(crate) fn repeat<'a>(cx: &mut Cx<'_, 'a>, text: &'a str, n: Option<i64>) -> Result<&'a str> {
       let n = n.unwrap_or(1).max(0) as usize;
       cx.charge_bytes(text.len().saturating_mul(n))?;
       Ok(cx.arena.alloc_str(&text.repeat(n)))
   }
   ```

   `raw` and `iter` bodies keep the four-parameter signature
   (`args, ctx, engine, arena`, plus `iter_arg_kind` after `args` for
   `iter`, plus the `@` payload last); an `each` body takes
   `items: Items` before `args`. Arity for an `eager` row comes from
   its extractors: a missing
   argument raises `InvalidArguments` and extras are ignored unless the
   row's `on_missing` / `on_extra` say otherwise. The extractor set
   (`Any`, `Str`, `Int`, `StrictNum`, `Truthy`, `Obj`, `Nullable<T>`,
   `Opt<T>`, `Lenient<T>`, `Lazy`, `Rest<T>`) lives in
   `operators/extract.rs`, with a table of what each one accepts;
   family-specific ones (the tensor family's) live with the family.
3. **Declared facts.** The row's metadata (an `OpMeta`, see
   `operators/meta.rs`) says
   whether the operator reads the data context (`reads_context`), has an
   effect (`Clock`, `Throws`, `Catches`), runs an argument under a pushed
   frame (`frames`), reads an argument position as written
   (`literal_args`: an expression there keeps its own node when the rest
   folds), opts out of folding or CSE, and what its work is
   proportional to (`cost`). Constant folding, CSE and scope resolution
   derive their classification from these facts, so a pure operator
   declares nothing. If it pushes a frame and does not say so, variable
   references beneath it resolve against the wrong frame: the debug scope
   oracle fails the first suite case that exercises it.
4. **Suite.** Add a JSON suite under
   `crates/datalogic-rs/tests/suites/<category>/` covering the happy path
   and at least one error case, then add it to
   `crates/datalogic-rs/tests/suites/index.json` (which fixes run order)
   with `UPDATE_SUITE_INDEX=1 cargo test -p datalogic-rs --all-features --test test_jsonlogic suite_index`.
   `suite_index_lists_every_suite` fails on an unlisted file,
   `every_operator_has_suite_cases` on an operator no rule calls, and
   `every_foldable_row_is_folded_by_a_suite_case` on a foldable operator
   with no all-literal case (it also checks that folding such a case
   changes nothing). See
   [crates/datalogic-rs/tests/README.md](./crates/datalogic-rs/tests/README.md)
   for the suite format.
5. **Snapshot.** Regenerate `docs/src/operators/operators.json` and the
   feature table in `docs/src/operators/overview.md`:
   `UPDATE_OPERATORS_JSON=1 cargo test -p datalogic-rs --all-features --test operators_json_test`.
   The same test checks that the overview's category table names the
   operator and that its opening counts add up.
   A row with a `cost` other than `Node` also needs a size-parameterised
   probe in `tests/budget_audit_test.rs`, which fails until it has one.
6. **Feature gating (new family only).** Declare the feature in
   `crates/datalogic-rs/Cargo.toml`, add it to the `all-operators` list
   there (the bindings and the benchmark depend on that one feature), add
   a `family` block with its gate to the table (the public `Family` enum
   is generated from it), and add the feature to the `feature-matrix` job in
   `.github/workflows/ci.yml` (and a `feature-combos` leg there, so its
   suites run on a reduced build).
   `catalogue_gates_match_cargo_features` fails until the feature and the
   `all-operators` list agree with the table.
7. **Editor and docs.** Add the operator's picker entry under
   `ui/src/components/logic-editor/config/operators/` (one file per
   category) so the React editor offers it, and document it on the
   matching page under `docs/src/operators/`. The UI's
   `config/__tests__/catalogue.test.ts` reads `operators.json` and fails
   until the picker has the operator (and its aliases), with an argument
   count the engine accepts. No binding needs a change: each depends on
   `all-operators` and exposes the engine as-is, so the operator is live
   once you rebuild it.

Generated guardrails cover every row without edits: name round-trips, the
catalogue against the build and against `Cargo.toml`, the declared arity
policy of every `eager` row (messages included, since an
`InvalidArguments` message is the serialised error `type`), scoped
arguments under the frame oracle, and the budget audit.

## Adding a custom operator (your own application)

The
[Custom Operators guide](https://goplasmatic.github.io/datalogic-rs/advanced/custom-operators.html)
on the docs site covers custom operators (extending the engine from your
application code), with a runnable
[`custom_operator` example](./crates/datalogic-rs/examples/custom_operator.rs)
in the core crate.

## Documentation site (`docs/`)

```bash
mdbook serve docs       # live preview at http://localhost:3000
mdbook build docs       # produces docs/book/
```

`.github/workflows/docs.yml` builds the published site at
https://goplasmatic.github.io/datalogic-rs/ on every push to `main` that
touches the docs, the UI, the WASM binding, `crates/datalogic-bind` or the
core crate's source (the playground embeds the engine). The workflow also
bundles the UI playground and the embed widget into the rendered book.

`.github/workflows/docs-check.yml` runs on every PR, docs-only ones
included: `scripts/check-stats.sh`, an mdBook build, and
`scripts/check-book-links.py` over the built book. Run the same before
you push a docs change:

```bash
bash scripts/check-stats.sh
mdbook build docs && python3 scripts/check-book-links.py docs/book
```
