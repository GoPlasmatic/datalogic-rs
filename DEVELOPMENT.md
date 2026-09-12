# Development

A walkthrough of working on each package in this monorepo. For the big
picture (what depends on what, why the layout is shaped this way), see
[ARCHITECTURE.md](./ARCHITECTURE.md).

## Prerequisites

| Tool        | Version | Why                                                            |
|-------------|---------|----------------------------------------------------------------|
| Rust        | 1.85+   | The core crate uses `edition = "2024"`                         |
| `wasm-pack` | latest  | Builds `bindings/wasm` (only for WASM/UI changes)              |
| Node.js     | 20+     | Builds and runs `ui`, `bindings/wasm`, and `bindings/node`     |
| Python      | 3.10+   | Builds `bindings/python` via `maturin`                         |
| Go          | 1.25+   | Builds `bindings/go` (`go.mod` declares `go 1.25`; also needs a C compiler for cgo) |
| Java JDK    | 22+     | FFM (`java.lang.foreign`) downcalls; `--enable-native-access=ALL-UNNAMED` on 24+ |
| Maven       | 3.8+    | Builds `bindings/jvm` (only for JVM changes)                   |
| .NET SDK    | 8.0+    | Builds and tests `bindings/dotnet` (only for .NET changes)     |
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

The repo holds six Cargo manifests, but the root workspace has only two
members. The four bindings and the fuzz crate are `exclude`d from it (each
declares its own `[workspace]` table; the `exclude` comment in the root
`Cargo.toml` explains why per crate), so root-level `cargo fmt --all`,
`cargo clippy --workspace` and `cargo clean` **silently skip them**.

The root `Makefile` fans those commands out over every manifest:

```bash
make lint        # fmt-check + clippy, all six manifests — run before a PR
make fmt         # format everything
make fmt-check   # check formatting without writing (what CI gates on)
make clippy      # clippy everything, every crate's failures in one pass
make clean       # cargo clean every manifest (~3 GB in a warm tree)
make clean-all   # clean + node_modules, venv, vendor, pkg/, dotnet bin+obj, ...
make help        # list all targets
```

Two crates need more than the stable host toolchain, and `make` degrades
rather than failing if you don't have it:

- **`bindings/wasm`** — `rustup target add wasm32-unknown-unknown`. Its
  `tests/web.rs` is `#![cfg(target_arch = "wasm32")]`, so a host-target lint
  compiles it to an empty file and checks nothing. Without the target, `make
  clippy` warns and falls back to a host lint (in CI, where `CI` is set, it
  fails instead so lost coverage can't hide).
- **`crates/datalogic-rs/fuzz`** — `rustup toolchain install nightly`.
  `#![no_main]` + `libfuzzer_sys` don't build on stable. Without nightly,
  `make clippy` prints a SKIP; `make fmt` covers it either way.

Individual crates are reachable as `make clippy-c`, `make clippy-wasm`, etc.

## The build pipeline

The packages have a strict build order. From a fresh clone:

```bash
# 1. Rust workspace — runs core unit/integration tests and the bench crate's checks.
# Most integration tests are gated behind feature = "serde_json"; the JSONLogic
# runner additionally needs feature = "templating". --all-features unlocks both.
cargo test --workspace --all-features

# 2. WASM bindings — produces bindings/wasm/pkg/{web,bundler,nodejs}.
cd bindings/wasm && ./build.sh && cd ../..

# 3. Node native binding — produces bindings/node/datalogic-node.<triple>.node
#    plus the index.js/index.d.ts loaders. Skip if you're only touching the
#    WASM or browser side.
cd bindings/node && npm install && npx napi build --platform --release && cd ../..

# 4. UI: picks up the WASM built in step 2 automatically.
cd ui && npm install
npm run dev   # or: npm run build:lib for the publishable bundle
```

The UI does not resolve `@goplasmatic/datalogic-wasm` from the registry:
its Vite and TypeScript configs alias the package to `ui/vendor/datalogic`,
and the `predev` / `prebuild*` lifecycle hooks copy `../bindings/wasm/pkg`
there (`npm run sync-wasm`). Rebuild WASM first, then start the UI, and the
fresh build is what you are testing; see [`ui` below](#ui--react-component).

## `crates/datalogic-rs` — Rust library

```bash
cargo check -p datalogic-rs
cargo test  -p datalogic-rs                        # default features
cargo test  -p datalogic-rs --all-features         # everything
make lint      # fmt + clippy, every manifest — what CI gates on
```

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

## `bindings/wasm` — WebAssembly bindings (browser / Deno / Bun / Workers)

```bash
cd bindings/wasm
./build.sh               # builds web, bundler, and nodejs targets
```

The crate is its own Cargo workspace (see ARCHITECTURE.md for why), so
`cargo` commands inside `bindings/wasm/` operate on it standalone. Run
`cargo test` from inside that directory if you need to test the FFI.
End-user API and install instructions: [bindings/wasm/README.md](./bindings/wasm/README.md).

The WASM build still ships a `nodejs` target — it's the right pick when
a consumer wants one artifact across Node + browser. **For production
Node workloads, prefer the native binding below**; it's noticeably
faster.

## `bindings/node` — Node native binding (napi-rs)

```bash
cd bindings/node
npm install                                   # one-time; pulls @napi-rs/cli
npx napi build --platform --release           # emits datalogic-node.<triple>.node + index.js + index.d.ts
npm test                                      # node --test '__test__/*.test.mjs'
```

This is the **first-class Node target** — published as
`@goplasmatic/datalogic-node` with per-platform `.node` prebuilds
distributed as npm `optionalDependencies`. The `.node` artifact,
`index.js`, and `index.d.ts` are generated by `napi build` and
gitignored; rerun the build after any Rust-side change.

The crate is its own Cargo workspace (matches the wasm/python/c
pattern) — `cargo` commands inside `bindings/node/` don't touch the
root workspace. End-user API and install instructions:
[bindings/node/README.md](./bindings/node/README.md).

## `bindings/python` — Python bindings (pyo3)

```bash
cd bindings/python
maturin develop --release         # build + install into the current venv
pytest                            # run the Python test suite
maturin build --release           # produce a wheel under target/wheels/
```

Like `bindings/wasm`, this crate is its own Cargo workspace (keeps the
pyo3 build deps out of the core `cargo test --workspace` path). End-user
API and install instructions: [bindings/python/README.md](./bindings/python/README.md).

## `bindings/c` — shared C ABI (cbindgen)

```bash
cd bindings/c
cargo build --release             # produces libdatalogic_c.{so,dylib,a}
cargo test                        # smoke-tests the extern "C" surface
```

The C header `include/datalogic.h` is regenerated by cbindgen on every
build. Don't edit by hand — edit `src/` and rebuild. Consumers can set
`DATALOGIC_C_SKIP_CBINDGEN=1` to suppress regeneration.

This crate is **not user-facing**; it's the FFI boundary the Go, JVM,
.NET, and PHP bindings consume. See
[bindings/c/README.md](./bindings/c/README.md) for the API surface and
memory / threading rules.

## `bindings/go` — Go binding (cgo over C ABI)

```bash
cd bindings/go
make build                        # cargo-builds bindings/c, stages lib/<host>/
make test                         # runs `go test -v ./...`
make print-platform               # prints the host's lib/ subdirectory name
```

The Makefile auto-detects host OS/arch and stages into
`lib/<host_os>_<host_arch>/` — only the matching `cgo_*_*.go` file
needs that subdirectory populated locally. Re-run `make build` after
any change to the C ABI's Rust source. End-user API, install
instructions, and prebuilt-library platform matrix:
[bindings/go/README.md](./bindings/go/README.md).

## `bindings/dotnet` — .NET binding (P/Invoke over C ABI)

```bash
cd ../c && cargo build --release   # produce libdatalogic_c.{so,dylib,dll}
cd ../dotnet
dotnet build -c Release
dotnet test
```

P/Invoke stubs are hand-written with `LibraryImport` (source-generated,
NativeAOT-ready). The native library is resolved at runtime via a
`DllImportResolver` that falls through:
`DATALOGIC_NATIVE_LIB` env → NuGet's `runtimes/<rid>/native/` →
`bindings/c/target/release/`. Publish target: NuGet `Goplasmatic.Datalogic`.
End-user API: [bindings/dotnet/README.md](./bindings/dotnet/README.md).

## `bindings/jvm` — JVM binding (FFM over C ABI)

```bash
cd ../c && cargo build --release
cd ../jvm
mvn test                           # JUnit 5
mvn package                        # produces target/datalogic-*.jar + sources + javadoc
```

The binding calls the C ABI directly through the `java.lang.foreign`
(FFM) API (JDK 22+); `internal/DatalogicNative` mirrors
`bindings/c/include/datalogic.h` as `MethodHandle` downcalls. The
Surefire plugin sets the `datalogic.library.path` system property to
`../c/target/release` (and passes `--enable-native-access=ALL-UNNAMED`)
so local tests pick up the in-tree cdylib. Publishable JARs ship the
native libs at the classpath root under `<os-arch>/`, which the loader
extracts and links at runtime. Target: Maven
Central as `io.github.goplasmatic:datalogic`. End-user API:
[bindings/jvm/README.md](./bindings/jvm/README.md).

## `bindings/php` — PHP binding (PHP FFI over C ABI)

```bash
cd ../c && cargo build --release
cd ../php
composer install                    # one-time
vendor/bin/phpunit                  # PHPUnit
```

Loads `libdatalogic_c.{so,dylib,dll}` at runtime via
`FFI::cdef(<curated header>, <lib path>)`. The Native loader searches
`DATALOGIC_NATIVE_LIB` → `bindings/php/lib/<os>-<arch>/` → in-tree
`bindings/c/target/release/`. PHP 8.4+ with `ext-ffi` required (tracks
`composer.json`; PHPUnit 13 in the test suite requires PHP 8.4+). Publish
target: Packagist `goplasmatic/datalogic`. End-user API:
[bindings/php/README.md](./bindings/php/README.md).

## `ui` — React component

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
| Registry and help | `src/components/logic-editor/config/__tests__/` | Registry matches `builtinOperatorNames()`; every help example evaluates to its documented result |
| Round trips | `src/components/logic-editor/utils/__tests__/` | A corpus covering every operator plus the shipped samples survives `jsonLogicToNodes` → `nodesToJsonLogic` and evaluates identically; edge ids stay unique |
| Editing | `src/components/logic-editor/services/__tests__/` | Argument add/remove, deletion reindexing, duplicate/paste/wrap, inline edits |
| Trace | `src/components/logic-editor/utils/trace/__tests__/` | Real `evaluateWithTrace` envelopes map onto the diagram with no synthetic nodes |
| App surface | `ui/tests/` | Samples evaluate to their stored results, share URLs round trip, every operator is reachable from the menus, evaluator/config helpers |

Adding an operator, a help example, or a sample means giving it the result the
engine actually produces: the suites compare against a live evaluation.

Three Vite configs power the three build modes:

- `vite.config.ts` — playground SPA
- `vite.lib.config.ts` — `@goplasmatic/datalogic-ui` library bundle
- `vite.embed.config.ts` — embeddable widget for docs

The WASM dep is vendored under `ui/vendor/datalogic/` (gitignored),
synced from `bindings/wasm/pkg/` by `sync-wasm`. The `predev` and `prebuild*`
hooks run it automatically, so the typical loop is just:

```bash
cd bindings/wasm && ./build.sh    # rebuild after Rust changes
cd ../../ui && npm run dev        # predev re-vendors the fresh pkg/
```

`@goplasmatic/datalogic-wasm` is listed as a **devDependency** pinned to the
last published release. Nothing in the build resolves it: `vite.config.ts`,
`vite.lib.config.ts`, `vite.embed.config.ts`, `tsconfig.app.json` and
`tsconfig.lib.json` all alias the package to `vendor/datalogic`. The pin
exists so the package name resolves for editors and for a plain `npm install`;
`release-build-ui.yml` rewrites it to the version being published. The library
bundle embeds the WASM engine, so consumers install neither.

## Releases

All publishing flows through `.github/workflows/release.yml`, triggered by
pushing a `v*` tag whose version matches `crates/datalogic-rs/Cargo.toml`. The
workflow validates → publishes the crate to crates.io → builds and publishes
every binding (`@goplasmatic/datalogic-wasm` WASM and
`@goplasmatic/datalogic-node` napi-rs prebuilds to npm, `datalogic-py` to
PyPI, `io.github.goplasmatic:datalogic` to Maven Central, `Goplasmatic.Datalogic`
to NuGet, `goplasmatic/datalogic` to Packagist, the Go module tag, and
`@goplasmatic/datalogic-ui`) → cuts the GitHub Release. There are no local
publish scripts; do not run `npm publish` or `cargo publish` by hand.
Use `scripts/bump-version.sh <x.y.z>` to update every versioned file the
validate job checks.

### Open release-ops items

The one-time watch list for the first 5.0.1 release legs (added
2026-07-02) was retired after that release brought all nine registries
up. Still open:

- **JVM natives on a clean machine:** `publish-jvm`'s Maven Central
  deploy first ran with the classpath-root layout on 2026-07-07; verify
  once that the published JAR loads its bundled natives on a machine
  with no repo checkout and `datalogic.library.path` unset.
- **NuGet signing** remains unimplemented: needs org certificates and a
  signing decision (README embedding, SourceLink, and snupkg already ship).

### One-time registry / marketing ops (added 2026-07-03; maintainer-only)

Registry state is a living figure; the release workflow run for the
latest `v*` tag is the source of truth, not this paragraph. Last
recorded check (2026-08-19, the 5.2.0 release): eight of the nine
registries served the tag (crates.io, npm ×3, PyPI, NuGet, the Go proxy,
and Maven Central, first published 2026-07-07); Packagist (registered
2026-07-03) lagged because the PHP dist push token had expired, so the
PHP leg needs `PHP_DIST_PUSH_TOKEN` rotated and `release.yml` rerun on
the tag. Done on 2026-07-03: Packagist
registration + webhook, GitHub Discussions enabled, wiki disabled. Done
on 2026-07-07: first Maven Central publish (`io.github.goplasmatic:datalogic`);
the root README's Maven row now carries the shields.io maven-central
badge. Done: Discussions categories created (Announcements, Q&A, Ideas,
Show and tell). Done on 2026-07-15: the stale v4 npm package
`@goplasmatic/datalogic` was deprecated and removed from the registry
(`npm view` now 404s); do **not** re-register or republish that name —
any new publish would resurrect its search-rank signal and split the
lineup three ways again. Still open:

- **Pin a "Who's using datalogic-rs? Add your project" thread** in the
  Show and tell Discussions category (the categories themselves exist;
  `.github/ISSUE_TEMPLATE/config.yml` already links to Q&A).
- **FUNDING.yml is intentionally absent**: add it only after enrolling
  the org (or a maintainer account) in GitHub Sponsors — a Sponsor
  button that 404s is worse than none.

Promotion sequencing, launch checklists, and adoption metrics live in
[.github/LAUNCH-PLAYBOOK.md](./.github/LAUNCH-PLAYBOOK.md).

## `tools/benchmark` — performance harness

Dev-only, never published. Four binaries share `src/lib.rs`: `self`
(regression baseline), `compare` (cross-library matrix), `boundary_core`
(the rust-core runner for the per-binding boundary benchmark under
`tools/benchmark/boundary/`), and `profile_macro` (hammers one macro
suite in a hot loop as a feeder for samply / Instruments):

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
```

Reports land in `tools/benchmark/output/` (gitignored). To add another
JSONLogic implementation as a comparison subject, see
[tools/benchmark/README.md](./tools/benchmark/README.md).

## Adding a built-in operator

A built-in operator is wired through several places in the tree. The
module doc at the top of `crates/datalogic-rs/src/opcode.rs` carries the
short form of this list; this is the full one.

1. **OpCode.** In `crates/datalogic-rs/src/opcode.rs`, add a variant to
   `OpCode`, an entry to `OPCODE_NAMES` (canonical name first, then any
   aliases; `FromStr` is a scan over this table, so there is no separate
   parse arm to write), and an `as_str()` arm. The opcode unit tests
   enforce that every name round-trips. `Engine::builtin_operator_names()`
   is derived from the same table, so the new name is reported
   automatically.
2. **Implementation.** Add `evaluate_<op>` under
   `crates/datalogic-rs/src/operators/<category>/` following the
   established signature
   (`args: &'a [CompiledNode], ctx: &mut ContextStack<'a>, engine: &Engine, arena: &'a Bump`)
   returning `Result<&'a DataValue<'a>>`.
3. **Dispatch.** Add an arm to the `dispatch_node_inner` match in
   `crates/datalogic-rs/src/engine/dispatch.rs`.
4. **Optimizer classification.** The compiler treats an operator as a
   pure function of its arguments unless told otherwise. If the new
   operator reads the data context, runs a callback per element, has
   side effects, or depends on runtime state, add it to the dynamic
   arms of `opcode_is_static` in `crates/datalogic-rs/src/node/logic.rs`
   (so it is never constant-folded) and to `opcode_is_cse_pure` or
   `is_iterator_opcode` in `crates/datalogic-rs/src/compile/optimize/cse.rs`
   (so the CSE pass neither memoizes it nor caches inside its per-item
   bodies). If it pushes a context frame around one of its arguments,
   register that position in `frames_pushed_for_child` in
   `crates/datalogic-rs/src/compile/scope.rs`, which both the scope pass
   and CSE consult; the debug oracle in `operators/variable` fails the
   first test that exercises an unregistered frame. `group_by` and keyed
   `distinct` (5.2.0) are the worked example; a pure operator needs
   nothing here.
5. **Suite.** Add a JSON suite under
   `crates/datalogic-rs/tests/suites/<category>/` covering the happy path
   and at least one error case, and register its path in
   `crates/datalogic-rs/tests/suites/index.json`; the runner only
   discovers files listed there. See
   [crates/datalogic-rs/tests/README.md](./crates/datalogic-rs/tests/README.md)
   for the suite format.
6. **Feature gating (new family only).** If the operator starts a new
   feature family, declare the feature in `crates/datalogic-rs/Cargo.toml`,
   `#[cfg]`-gate the variant, dispatch arm, and implementation, and add
   the feature to every consumer that enables families explicitly:
   `bindings/wasm/Cargo.toml`, `bindings/node/Cargo.toml`,
   `bindings/python/Cargo.toml`, `bindings/c/Cargo.toml` (Go, JVM, .NET,
   and PHP inherit from it), `tools/benchmark/Cargo.toml` (otherwise its
   suites show `ERR` in the matrix), and the `feature-matrix` job in
   `.github/workflows/ci.yml`. An operator joining an existing family
   reuses that family's gate.
7. **Editor and docs.** Add the operator's picker entry under
   `ui/src/components/logic-editor/config/operators/` (one file per
   category) so the React editor offers it, and document it on the
   matching page under `docs/src/operators/`. The WASM and Node bindings
   need no change: they expose the engine as-is, so the operator is live
   once you rebuild them.

## Adding a custom operator (your own application)

Custom operators (extending the engine from your application code) are
covered in the
[Custom Operators guide](https://goplasmatic.github.io/datalogic-rs/advanced/custom-operators.html)
on the docs site, with a runnable
[`custom_operator` example](./crates/datalogic-rs/examples/custom_operator.rs)
in the core crate.

## Documentation site (`docs/`)

```bash
mdbook serve docs       # live preview at http://localhost:3000
mdbook build docs       # produces docs/book/
```

The published site at https://goplasmatic.github.io/datalogic-rs/ is built
by `.github/workflows/docs.yml` on every push to `main` that touches docs,
WASM, or UI. The workflow also bundles the UI playground and the embed
widget into the rendered book.
